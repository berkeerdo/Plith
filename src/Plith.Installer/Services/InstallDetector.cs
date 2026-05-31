using System.Diagnostics;
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Detects whether Plith is already installed at the standard location and reads its
/// version. WelcomePage uses this to switch the primary button label between
/// "Install Plith" / "Reinstall Plith vX.Y.Z" / "Update Plith vX.Y.Z → vN.M.P".
/// </summary>
public sealed class InstallDetector
{
    public static readonly string DefaultInstalledExePath =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            "Plith", "Plith.exe");

    private readonly string _installedExePath;

    public InstallDetector(string? installedExePath = null)
    {
        _installedExePath = installedExePath ?? DefaultInstalledExePath;
    }

    public string InstalledExePath => _installedExePath;

    /// <summary>Returns the ProductVersion of the installed Plith.exe, or null if not installed.
    /// Strips any SemVer build-metadata suffix (text after '+') so the result is
    /// suitable for direct display (e.g. "0.1.0", not "0.1.0+76906d7...").</summary>
    public string? GetInstalledVersion()
    {
        if (!File.Exists(_installedExePath)) return null;
        try
        {
            var info = FileVersionInfo.GetVersionInfo(_installedExePath);
            var raw = info.ProductVersion ?? info.FileVersion;
            if (raw is null) return null;
            var plus = raw.IndexOf('+', StringComparison.Ordinal);
            return plus >= 0 ? raw[..plus] : raw;
        }
        catch
        {
            return null;
        }
    }
}
