using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Creates and removes the Start menu .lnk via WScript.Shell COM. The shortcut lives in
/// %ProgramData%\Microsoft\Windows\Start Menu\Programs so all users see it (matches
/// the all-users install posture). Removing the .lnk on uninstall is a single File.Delete.
/// </summary>
public sealed class ShortcutService
{
    public static readonly string StartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        "Programs", "Plith.lnk");

    public void CreateStartMenuShortcut(string targetExePath, string description)
    {
        var wshType = Type.GetTypeFromProgID("WScript.Shell")
            ?? throw new InvalidOperationException("WScript.Shell COM type not available.");
        dynamic shell = Activator.CreateInstance(wshType)!;

        // dynamic dispatch into WScript.Shell COM — no strongly-typed interop assembly
        // for this well-known scripting host; dynamic is the idiomatic .NET pattern here.
#pragma warning disable CA1711, CA1812
        dynamic shortcut = shell.CreateShortcut(StartMenuShortcutPath);
        shortcut.TargetPath = targetExePath;
        shortcut.WorkingDirectory = Path.GetDirectoryName(targetExePath) ?? string.Empty;
        shortcut.IconLocation = targetExePath;
        shortcut.Description = description;
        shortcut.Save();
#pragma warning restore CA1711, CA1812
    }

    public void RemoveStartMenuShortcut()
    {
        if (File.Exists(StartMenuShortcutPath))
            File.Delete(StartMenuShortcutPath);
    }
}
