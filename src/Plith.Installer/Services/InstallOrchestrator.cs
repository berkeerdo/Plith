using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
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
        foreach (var proc in Process.GetProcessesByName("Plith"))
        {
            // 5 s WaitForExit (up from 2 s) — UIAccess-signed Plith running from
            // Program Files takes longer to unwind than a plain user-mode process:
            // the elevated tray plus the WinRT SMTC finalizer plus the WH_KEYBOARD_LL
            // hook each need their own shutdown steps, and a 2 s ceiling would race
            // through them under Norton contention.
            try { proc.Kill(entireProcessTree: true); proc.WaitForExit(5000); }
#pragma warning disable CA1031 // Do not catch general exception types
            catch { }
#pragma warning restore CA1031
        }
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

    // File.Copy with exponential backoff for the common "target was locked a moment ago"
    // case. Overwriting a DLL right after the process that had it loaded exits routinely
    // fails with UnauthorizedAccessException / IOException on Windows because the OS
    // hasn't yet released its memory-mapped view; AV scanners produce the same transient
    // lock. If short waits don't clear the lock, fall through to the sideline dance
    // which handles a persistently-held target (Norton scanning our binary, Explorer
    // extracting an icon, etc.). If even THAT fails, the caller gets an actionable
    // message so the UI can surface a fix path instead of a raw stack.
    //
    // Attempt cadence: 250 / 500 / 1000 / 2000 / 2000 / 2000 / 2000 / 2000 = ~12 s of
    // waits before falling through — up from the previous 3.75 s. Observed Norton-plus-
    // running-Plith holds tend to clear inside 5-10 s; the extra headroom converts most
    // "installer says access denied" cases into "installer paused for a few seconds
    // then succeeded".
    internal static void CopyWithRetry(string source, string target)
    {
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
                // Swallow every attempt: on the last one we still want to fall through
                // to CopyOverLockedFile below rather than rethrow. Previously the
                // filter had 'attempt < maxAttempts' and the final throw skipped the
                // sideline dance entirely, defeating the whole point of the fallback.
                if (attempt == maxAttempts) break;
                System.Threading.Thread.Sleep(delayMs);
                delayMs = Math.Min(delayMs * 2, maxDelayMs);
            }
        }
        try
        {
            CopyOverLockedFile(source, target);
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            throw new InstallLockedFileException(target, lastError ?? ex);
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
