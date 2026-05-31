using System.IO;
using Microsoft.Win32;

namespace Plith.Installer.Services;

/// <summary>
/// Manages the Add/Remove Programs registry entry under HKLM\...\Uninstall\Plith,
/// plus the per-user HKCU\...\Run autostart entry. Idempotent.
/// </summary>
public sealed class RegistryService
{
    private const string UninstallKeyPath = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith";
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";

    /// <summary>Writes the Add/Remove Programs entry. UninstallString points at the
    /// installer copied to ProgramFiles\Plith\Setup\Plith-Uninstaller.exe with --uninstall.</summary>
    public void WriteUninstallEntry(string installDir, string installedExePath, string version, string uninstallerPath)
    {
        using var key = Registry.LocalMachine.CreateSubKey(UninstallKeyPath, writable: true)
            ?? throw new InvalidOperationException(
                "Failed to create HKLM\\...\\Uninstall\\Plith — admin required?");

        key.SetValue("DisplayName", "Plith", RegistryValueKind.String);
        key.SetValue("DisplayVersion", version, RegistryValueKind.String);
        key.SetValue("Publisher", "Plith Self-Signed", RegistryValueKind.String);
        key.SetValue("InstallLocation", installDir, RegistryValueKind.String);
        key.SetValue("DisplayIcon", installedExePath, RegistryValueKind.String);
        key.SetValue("UninstallString", $"\"{uninstallerPath}\" --uninstall", RegistryValueKind.String);
        key.SetValue("NoModify", 1, RegistryValueKind.DWord);
        key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
        key.SetValue("EstimatedSize", ComputeEstimatedKb(installDir), RegistryValueKind.DWord);
    }

    public void RemoveUninstallEntry()
    {
        Registry.LocalMachine.DeleteSubKeyTree(UninstallKeyPath, throwOnMissingSubKey: false);
    }

    public void WriteAutoStart(string installedExePath)
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.SetValue("Plith", $"\"{installedExePath}\"", RegistryValueKind.String);
    }

    public void RemoveAutoStart()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.DeleteValue("Plith", throwOnMissingValue: false);
    }

    private static int ComputeEstimatedKb(string dir)
    {
        if (!Directory.Exists(dir)) return 0;
        long bytes = 0;
        foreach (var file in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories))
        {
            try { bytes += new FileInfo(file).Length; } catch { /* skip unreadable */ }
        }
        return (int)(bytes / 1024);
    }
}
