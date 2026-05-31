using System.Diagnostics;
using System.IO;
using System.Linq;

namespace Plith.Installer.Services;

/// <summary>
/// Locates signtool.exe (PATH first, Windows SDK fallback) and invokes it to sign a binary
/// with a thumbprint-referenced cert. Throws actionable errors when the tool can't be found
/// or when the signing call fails.
/// </summary>
public sealed class SignToolWrapper
{
    private readonly LogService _log;

    public SignToolWrapper(LogService log)
    {
        _log = log;
    }

    /// <summary>Sign the given exe with the cert identified by SHA-1 thumbprint.
    /// Uses SHA-256 file digest + timestamps via digicert.com.</summary>
    public void Sign(string exePath, string certThumbprint)
    {
        var signtool = ResolveSignToolPath()
            ?? throw new InvalidOperationException(
                "signtool.exe not found. Install the Windows 10/11 SDK or VS Build Tools " +
                "(workload: 'Desktop development with C++') and re-run.");

        _log.Info($"signtool: using '{signtool}'");
        _log.Info($"signtool: signing '{exePath}' with thumbprint {certThumbprint}");

        var psi = new ProcessStartInfo(signtool)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        psi.ArgumentList.Add("sign");
        psi.ArgumentList.Add("/sha1");
        psi.ArgumentList.Add(certThumbprint);
        psi.ArgumentList.Add("/fd");
        psi.ArgumentList.Add("SHA256");
        psi.ArgumentList.Add("/tr");
        psi.ArgumentList.Add("http://timestamp.digicert.com");
        psi.ArgumentList.Add("/td");
        psi.ArgumentList.Add("SHA256");
        psi.ArgumentList.Add(exePath);

        using var proc = Process.Start(psi)
            ?? throw new InvalidOperationException("Failed to launch signtool.");
        string stdout = proc.StandardOutput.ReadToEnd();
        string stderr = proc.StandardError.ReadToEnd();
        proc.WaitForExit();

        if (!string.IsNullOrWhiteSpace(stdout)) _log.Info("signtool stdout: " + stdout.Trim());
        if (!string.IsNullOrWhiteSpace(stderr)) _log.Info("signtool stderr: " + stderr.Trim());

        if (proc.ExitCode != 0)
            throw new InvalidOperationException(
                $"signtool exited with code {proc.ExitCode}. See install.log.");
    }

    private static string? ResolveSignToolPath()
    {
        // 1. PATH lookup
        var pathDirs = Environment.GetEnvironmentVariable("PATH")?.Split(Path.PathSeparator)
            ?? Array.Empty<string>();
        foreach (var dir in pathDirs)
        {
            var candidate = Path.Combine(dir, "signtool.exe");
            if (File.Exists(candidate)) return candidate;
        }

        // 2. Windows SDK fallback
        var sdkRoot = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),
            "Windows Kits", "10", "bin");
        if (!Directory.Exists(sdkRoot)) return null;

        return Directory.EnumerateFiles(sdkRoot, "signtool.exe", SearchOption.AllDirectories)
            .Where(p => p.Contains(@"\x64\", StringComparison.OrdinalIgnoreCase))
            .OrderByDescending(p => p)
            .FirstOrDefault();
    }
}
