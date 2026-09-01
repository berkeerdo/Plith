using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Services;

public sealed class InstallOrchestrator
{
    public const string InstallDir = @"C:\Program Files\Plith";
    public static readonly string InstalledExe = Path.Combine(InstallDir, "Plith.exe");
    public static readonly string UninstallerDir = Path.Combine(InstallDir, "Setup");
    public static readonly string UninstallerExe = Path.Combine(UninstallerDir, "Plith-Uninstaller.exe");
    private static readonly string StageDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Plith", "Installer", "stage");

    private readonly LogService _log;
    private readonly CertService _cert;
    private readonly ShortcutService _shortcut;
    private readonly RegistryService _registry;
    private readonly InstallerViewModel _vm;

    public InstallOrchestrator(LogService log, CertService cert,
        ShortcutService shortcut, RegistryService registry, InstallerViewModel vm)
    {
        _log = log;
        _cert = cert;
        _shortcut = shortcut;
        _registry = registry;
        _vm = vm;
    }

    public void PrepareSteps()
    {
        _vm.Steps.Clear();
        _vm.Steps.Add(new InstallStepViewModel { Title = "Registering trust" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Extracting Plith files" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Copying to Program Files" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Registering Plith" });
        _vm.Progress = 0;
    }

    public async Task RunInstallAsync()
    {
        _log.Info("Install: starting");
        try
        {
            string thumbprint = await RunStep(0, () => _cert.InstallTrust());
            _log.Info($"Install: registered trust for cert thumbprint {thumbprint}");
            await RunStep(1, () => ExtractBundle());
            await RunStep(2, () => CopyToProgramFiles());
            await RunStep(3, () => RegisterPlith());
            _log.Info("Install: done");
        }
        catch (Exception ex)
        {
            _log.Error("Install pipeline", ex);
            throw;
        }
    }

    private async Task<T> RunStep<T>(int stepIndex, Func<T> action)
    {
        var step = _vm.Steps[stepIndex];
        step.Status = InstallStepStatus.Running;
        try
        {
            var result = await Task.Run(action);
            step.Status = InstallStepStatus.Done;
            _vm.Progress = (stepIndex + 1) / (double)_vm.Steps.Count;
            return result;
        }
        catch (Exception ex)
        {
            step.Status = InstallStepStatus.Failed;
            step.FailureMessage = ex.Message;
            _vm.FailedStepTitle = step.Title;
            _vm.ErrorMessage = ex.Message;
            throw;
        }
    }

    private async Task RunStep(int stepIndex, Action action)
        => await RunStep(stepIndex, () => { action(); return true; });

    private void ExtractBundle()
    {
        if (Directory.Exists(StageDir)) Directory.Delete(StageDir, recursive: true);
        Directory.CreateDirectory(StageDir);
        var extractor = EmbeddedExtractor.FromEmbeddedResource();
        extractor.ExtractTo(StageDir);
    }

    private void CopyToProgramFiles()
    {
        // Make sure every Plith process is really dead BEFORE touching a single file.
        // Historically the kill happened here with a catch{} that swallowed every
        // failure, and the copy loop then ran into the same locked files. If Plith
        // survives our attempts, throw with a message that names the fix path instead
        // of continuing into a guaranteed UnauthorizedAccessException down the line.
        EnsurePlithIsClosed();

        // Ask Windows Restart Manager to shut down any NON-critical process still
        // holding the existing install-dir files. RM sends WM_CLOSE first, then
        // TerminateProcess; it skips services and critical processes (Norton,
        // Windows Defender, System). That's exactly what we need — Plith stragglers
        // and Explorer preview handlers get closed cleanly, AV keeps running but at
        // least stops being the mysterious "unknown holder".
        AskRestartManagerToReleaseInstallDir();

        // Windows keeps a memory-mapped view of a .NET process's DLLs alive briefly
        // after the process exits; overwriting them immediately trips
        // UnauthorizedAccessException even though the .exe is gone. AV scanning on
        // a fresh EXE compounds the delay. 5 s (up from 1.5 s) covers the observed
        // worst case where the old build cached under Norton scan blocks the write.
        System.Threading.Thread.Sleep(5000);

        Directory.CreateDirectory(InstallDir);
        Directory.CreateDirectory(UninstallerDir);

        MirrorCopy(StageDir, InstallDir);
        CopyWithRetry(Environment.ProcessPath!, UninstallerExe);

        if (_vm.AutoStartEnabled) _registry.WriteAutoStart(InstalledExe);
        else _registry.RemoveAutoStart();
    }

    private void AskRestartManagerToReleaseInstallDir()
    {
        if (!Directory.Exists(InstallDir)) return;
        try
        {
            // Cap at 60 files — Restart Manager has a fixed cap on registered
            // resources per session and the full Plith install has ~30 files anyway.
            var existing = new List<string>();
            foreach (var file in Directory.EnumerateFiles(InstallDir, "*", SearchOption.AllDirectories))
            {
                existing.Add(file);
                if (existing.Count >= 60) break;
            }
            if (existing.Count == 0) return;

            var holders = RestartManagerService.CloseHolders(existing);
            if (holders.Count > 0)
                _log.Info($"Install: Restart Manager identified holders: {string.Join(", ", holders)}");
            else
                _log.Info("Install: Restart Manager: no processes held install files");
        }
        catch (Exception ex)
        {
            // RM is a best-effort optimisation. If the API itself fails (missing
            // dll, permission fluke) we still fall through to the retry loop.
            _log.Warn($"Install: Restart Manager query failed: {ex.Message}");
        }
    }

    // Multi-round kill with verification, up to ~15 s total. UIAccess-signed Plith
    // running from Program Files takes longer to unwind than a plain user-mode
    // process — the tray plus WinRT SMTC finalizer plus the WH_KEYBOARD_LL hook
    // each need their own shutdown steps, and a Norton scan running concurrently
    // can extend WaitForExit past a naive 5 s cap. If a process refuses to die
    // (permissions, protected handle) or a new one spawns in between (the tray
    // auto-restart edge case), we surface a clear error that names Plith and points
    // the user at the tray-Exit action.
    private void EnsurePlithIsClosed()
    {
        const int rounds = 3;
        int survivingCount = 0;
        for (int round = 1; round <= rounds; round++)
        {
            var procs = Process.GetProcessesByName("Plith");
            if (procs.Length == 0) return;

            _log.Info($"Install: killing {procs.Length} Plith process(es), round {round}/{rounds}");
            foreach (var proc in procs)
            {
                try
                {
                    proc.Kill(entireProcessTree: true);
                    proc.WaitForExit(5000);
                }
                catch (Exception ex)
                {
                    // Log but keep trying the remaining processes — a partial kill
                    // is still better than skipping every subsequent one.
                    _log.Warn($"Install: kill failed for pid {proc.Id}: {ex.Message}");
                }
                finally
                {
                    proc.Dispose();
                }
            }

            // Give Windows a moment to notify the parent about the exit + let a
            // stubborn tray try to restart itself. Longer wait between rounds
            // improves odds a lingering process actually stays dead.
            System.Threading.Thread.Sleep(750);

            survivingCount = Process.GetProcessesByName("Plith").Length;
            if (survivingCount == 0) return;
        }

        throw new PlithStillRunningException(survivingCount);
    }

    private void RegisterPlith()
    {
        _shortcut.CreateStartMenuShortcut(InstalledExe,
            "Modern Windows audio OSD with Voicemeeter-first design and media controls.");

        var versionInfo = FileVersionInfo.GetVersionInfo(InstalledExe);
        var version = versionInfo.ProductVersion ?? versionInfo.FileVersion ?? "0.0.0";

        _registry.WriteUninstallEntry(InstallDir, InstalledExe, version, UninstallerExe);
    }

    private static void MirrorCopy(string source, string target)
    {
        Directory.CreateDirectory(target);

        foreach (var sourceFile in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
        {
            var relative = Path.GetRelativePath(source, sourceFile);
            var targetFile = Path.Combine(target, relative);
            Directory.CreateDirectory(Path.GetDirectoryName(targetFile)!);
            CopyWithRetry(sourceFile, targetFile);
        }

        // Mirror semantics: delete target-only files (except the Setup\ subdir which holds
        // the uninstaller we copy in after this method).
        foreach (var targetFile in Directory.EnumerateFiles(target, "*", SearchOption.AllDirectories))
        {
            if (targetFile.StartsWith(Path.Combine(target, "Setup"), StringComparison.OrdinalIgnoreCase))
                continue;
            var relative = Path.GetRelativePath(target, targetFile);
            var sourceFile = Path.Combine(source, relative);
            if (!File.Exists(sourceFile)) File.Delete(targetFile);
        }
    }

    public void PrepareUninstallSteps()
    {
        _vm.Steps.Clear();
        _vm.Steps.Add(new InstallStepViewModel { Title = "Stopping Plith" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Cleaning up registry" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Removing files" });
        _vm.Progress = 0;
    }

    public async Task RunUninstallAsync()
    {
        _log.Info("Uninstall: starting");
        try
        {
            await RunStep(0, () =>
            {
                foreach (var proc in Process.GetProcessesByName("Plith"))
                {
                    try { proc.Kill(entireProcessTree: true); proc.WaitForExit(2000); }
#pragma warning disable CA1031 // Do not catch general exception types
                    catch { }
#pragma warning restore CA1031
                }
            });

            // Registry cleanup BEFORE self-delete spawn — if a transient registry failure
            // throws after we've spawned the delete, the user's autostart entry would
            // dangle pointing at a deleted exe. By spawning self-delete last (which
            // commits the no-return), every observable failure mode keeps state coherent.
            await RunStep(1, () =>
            {
                _registry.RemoveAutoStart();
                _registry.RemoveUninstallEntry();
            });

            await RunStep(2, () =>
            {
                _shortcut.RemoveStartMenuShortcut();
                // Spawn the self-delete child process for InstallDir — runs AFTER this process exits.
                // The child waits 8 s then removes Program Files\Plith\ including this uninstaller binary.
                SpawnSelfDelete();
            });

            _log.Info("Uninstall: done");
        }
        catch (Exception ex)
        {
            _log.Error("Uninstall pipeline", ex);
            throw;
        }
    }

    // Multi-strategy overwrite. Failures we've seen in the wild fall into three
    // separate buckets and one strategy doesn't cover all three:
    //
    //   1. Sharing violation — a process (or the OS kernel via memory-mapped section)
    //      has the file open without FILE_SHARE_WRITE. File.Copy overwrite fails.
    //      Handled by the retry loop and the sideline rename dance.
    //
    //   2. ReadOnly attribute — AV quarantine or a previous install artifact set the
    //      ReadOnly bit. File.Copy overwrite fails with UnauthorizedAccessException
    //      even though nobody is holding the file. Handled by clearing the attribute
    //      before each attempt.
    //
    //   3. Restrictive ACL — a previous install / quarantine changed ownership or
    //      denied write to Administrators. File.Copy fails, so does File.Move.
    //      MoveFileEx REPLACE_EXISTING takes a different kernel path and sometimes
    //      succeeds where File.Copy doesn't — worth trying before giving up.
    //
    // The order below is: quickest safe hop first, most invasive last.
    internal static void CopyWithRetry(string source, string target)
    {
        // Short-circuit: identical content means no work. Third-party DLLs that
        // don't change between releases skip the whole overwrite dance — and those
        // are the files most likely to still be memory-mapped from the prior install.
        if (IsIdenticalContent(source, target)) return;

        // Clear the ReadOnly attribute up front — cheap, side-effect-free if the bit
        // isn't set. AV quarantine occasionally leaves this flipped on binaries it
        // scanned, which turns File.Copy overwrite into UnauthorizedAccessException
        // even when no process holds the file.
        TryClearReadOnly(target);

        const int maxAttempts = 8;
        const int maxDelayMs = 2000;
        int delayMs = 250;
        Exception? lastError = null;
        for (int attempt = 1; attempt <= maxAttempts; attempt++)
        {
            try
            {
                File.Copy(source, target, overwrite: true);
                return;
            }
            catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
            {
                lastError = ex;
                if (attempt == maxAttempts) break;
                System.Threading.Thread.Sleep(delayMs);
                delayMs = Math.Min(delayMs * 2, maxDelayMs);
                TryClearReadOnly(target);
            }
        }

        // Retry loop exhausted. Try MoveFileEx REPLACE_EXISTING — atomic kernel-level
        // replacement that occasionally succeeds against ACL / attribute issues that
        // trip File.Copy. If it works we're done; otherwise fall through to the
        // sideline dance.
        if (TryReplaceViaMoveFileEx(source, target))
            return;

        try
        {
            CopyOverLockedFile(source, target);
            return;
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            lastError = ex;
        }

        // Every strategy exhausted. Enrich the error with Restart Manager's holder
        // list AND the raw Win32 error code — the code is what tells the user (and
        // us in support) whether this was a lock (32, sharing violation), a
        // permissions problem (5, access denied), a missing directory (2, 3), etc.
        IReadOnlyList<string> holders = Array.Empty<string>();
        try { holders = RestartManagerService.EnumerateHolders(new[] { target }); } catch { }
        throw new InstallLockedFileException(target, holders, DescribeWin32Error(lastError!), lastError!);
    }

    private static void TryClearReadOnly(string path)
    {
        try
        {
            if (!File.Exists(path)) return;
            var attrs = File.GetAttributes(path);
            if ((attrs & FileAttributes.ReadOnly) != 0)
                File.SetAttributes(path, attrs & ~FileAttributes.ReadOnly);
        }
        catch { /* best effort — if we can't even read attributes, the copy will fail with a real message */ }
    }

    private static bool TryReplaceViaMoveFileEx(string source, string target)
    {
        // MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED (cross-volume) |
        // MOVEFILE_WRITE_THROUGH (block until flush). The combination behaves like
        // Windows Explorer's "replace file" — succeeds against many ACL / attribute
        // edge cases File.Copy overwrite can't cross. We do NOT want to fall through
        // silently on failure: return true only when the move actually reports success.
        const uint REPLACE_EXISTING = 0x1;
        const uint COPY_ALLOWED = 0x2;
        const uint WRITE_THROUGH = 0x8;
        try
        {
            return MoveFileEx(source, target, REPLACE_EXISTING | COPY_ALLOWED | WRITE_THROUGH);
        }
        catch { return false; }
    }

    // Extracts the underlying Win32 error code from a .NET IO / UAE exception so
    // the terminal error message can surface it. .NET wraps the Win32 code in
    // Exception.HResult (0x8007xxxx); the low 16 bits are the actual code.
    private static string DescribeWin32Error(Exception ex)
    {
        int hresult = ex.HResult;
        int code = hresult & 0xFFFF;
        string name = code switch
        {
            2 => "ERROR_FILE_NOT_FOUND",
            3 => "ERROR_PATH_NOT_FOUND",
            5 => "ERROR_ACCESS_DENIED (permissions / ACL / read-only)",
            19 => "ERROR_WRITE_PROTECT",
            32 => "ERROR_SHARING_VIOLATION (another process holds it)",
            33 => "ERROR_LOCK_VIOLATION",
            80 => "ERROR_FILE_EXISTS",
            145 => "ERROR_DIR_NOT_EMPTY",
            206 => "ERROR_FILENAME_EXCED_RANGE",
            _ => $"Win32 error code {code}",
        };
        return name;
    }

    // Length + SHA-256 comparison. Fast-fails on any I/O or permission error and
    // returns false, so the caller falls through to the real copy. If Norton has the
    // target open share-deny-write we can still usually open it share-read, which is
    // enough for hashing — the whole point is to detect "no work needed" cheaply.
    private static bool IsIdenticalContent(string source, string target)
    {
        try
        {
            if (!File.Exists(target)) return false;
            var srcInfo = new FileInfo(source);
            var tgtInfo = new FileInfo(target);
            if (srcInfo.Length != tgtInfo.Length) return false;

            using var sha = SHA256.Create();
            byte[] srcHash;
            byte[] tgtHash;
            using (var s = new FileStream(source, FileMode.Open, FileAccess.Read, FileShare.Read))
                srcHash = sha.ComputeHash(s);
            using (var t = new FileStream(target, FileMode.Open, FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete))
                tgtHash = sha.ComputeHash(t);
            return srcHash.AsSpan().SequenceEqual(tgtHash);
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            return false;
        }
    }

    // Rename dance for a persistently-locked target. Windows lets us RENAME a file
    // whose handles were opened with FILE_SHARE_DELETE (the default for loaded .NET
    // assemblies and everything AV scanners open) even when File.Copy overwrite hits
    // the same handle with a sharing violation. Move the old copy aside, drop the
    // new bytes at the original path, and ask Windows to delete the sideline on next
    // reboot. The application starts using the new bytes immediately; the sideline
    // is cleaned up automatically when the machine restarts.
    private static void CopyOverLockedFile(string source, string target)
    {
        if (File.Exists(target))
        {
            var sideline = string.Concat(target, ".pending-delete-",
                Guid.NewGuid().ToString("N").AsSpan(0, 8));
            File.Move(target, sideline);
            _ = MoveFileEx(sideline, null, MOVEFILE_DELAY_UNTIL_REBOOT);
        }
        File.Copy(source, target, overwrite: false);
    }

    private const uint MOVEFILE_DELAY_UNTIL_REBOOT = 0x4;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MoveFileEx(string lpExistingFileName, string? lpNewFileName, uint dwFlags);

    private static void SpawnSelfDelete()
    {
        // Spawned cmd.exe outlives this process (Plith-Uninstaller.exe) and deletes the
        // install dir which contains this binary. Standard Windows uninstaller pattern.
        // 8 s timeout (not 3) — slow disks + AV scanning the deleted exe can outlast a
        // shorter wait, leaving Setup\Plith-Uninstaller.exe orphaned. Explicit CWD =
        // C:\Windows so cmd doesn't inherit the install dir and block its own rd.
        var psi = new ProcessStartInfo("cmd.exe")
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            WindowStyle = ProcessWindowStyle.Hidden,
            WorkingDirectory = Environment.GetFolderPath(Environment.SpecialFolder.Windows),
        };
        psi.ArgumentList.Add("/c");
        psi.ArgumentList.Add($"timeout /t 8 /nobreak >nul && rd /s /q \"{InstallDir}\"");
        Process.Start(psi);
    }
}
