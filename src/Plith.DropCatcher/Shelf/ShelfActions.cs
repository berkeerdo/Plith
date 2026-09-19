using System.ComponentModel;
using System.Diagnostics;

namespace Plith.DropCatcher.Shelf;

/// <summary>
/// Opening a file, from the right process.
///
/// This runs at Medium integrity, which is the point rather than an accident. The same call from
/// Plith would start the person's document at High, because a child inherits its parent's token
/// and Plith has UIAccess. A text file opened at High is a text file whose editor cannot be
/// dragged onto by anything, and the person has no way of knowing why.
///
/// Neither method here touches the wire: Open and ShowInFileManager are actions this process
/// carries out on its own account, never requests Plith answers. See ShelfWindow for the split
/// between the four verbs that do cross the wire (RemoveItems, ClearShelf, NewStack, Restack) and
/// these two, which do not.
/// </summary>
public static class ShelfActions
{
    public static void Open(string path)
    {
        try
        {
            using var _ = Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
        }
        catch (Win32Exception) { /* no handler for this type; nothing useful to say */ }
        catch (InvalidOperationException) { }
    }

    /// <summary>
    /// The file manager, whichever one this is. Explorer's /select verb is honoured by the
    /// default handler for a folder, and on this machine that is Files rather than Explorer:
    /// <c>explorer.exe &lt;path&gt;</c> opens a WinUIDesktopWin32WindowClass window here, not a
    /// CabinetWClass one. That is measured, and it is why nothing downstream may look for
    /// Explorer's window class.
    /// </summary>
    public static void ShowInFileManager(string path)
    {
        try
        {
            using var _ = Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"")
            {
                UseShellExecute = true,
            });
        }
        catch (Win32Exception) { }
    }
}
