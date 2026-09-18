using System.Collections.Concurrent;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace Plith.DropCatcher.Shelf;

/// <summary>
/// Extracts the shell's own icon for a path, the same icon Explorer would show for it.
///
/// This lives in the catcher and not in Plith on purpose. Extracting a shell icon loads whatever
/// icon handler the file's extension is registered to, and that handler is third-party code the
/// shell chose, not code this product wrote. Plith runs UIAccess-signed and at High integrity,
/// which is exactly the process that must not be the one loading an arbitrary shell extension on
/// a stranger's behalf. The catcher runs at Medium, launched by Explorer rather than by Plith, so
/// it is where this kind of extraction belongs.
/// </summary>
internal static class ShellIcons
{
    private const uint SHGFI_ICON = 0x000000100;
    private const uint SHGFI_LARGEICON = 0x000000000;
    private const uint SHGFI_USEFILEATTRIBUTES = 0x000000010;
    private const uint FILE_ATTRIBUTE_NORMAL = 0x00000080;

    // Cached by path, because a repaint costs an extraction otherwise and the shelf repaints on
    // every selection change. The cache is per-run and unbounded by design: it is bounded in
    // practice by the shelf's own cap of twenty.
    private static readonly ConcurrentDictionary<string, ImageSource?> Cache =
        new(StringComparer.OrdinalIgnoreCase);

    // SHGetFileInfo is not safe to call from several threads at once: a shelf of six tiles fires
    // six Task.Run extractions together, and measured directly (a standalone repro outside this
    // process, six different paths, six concurrent calls, repeated), the first concurrent burst
    // in a fresh process loses several of them - not the slow one, different files each time -
    // while every call after that first burst succeeds. A lost call is not a slow one that would
    // eventually resolve: it comes back false and Cache would hold that false forever, so a
    // shelf's first paint could show fallback icons for files the shell was fully able to answer
    // for, a few hundred milliseconds later, from the very same process. Serialising every call
    // behind this lock removed the failure across five repeated runs of that repro; extraction
    // still happens off the UI thread, it just also happens one at a time.
    private static readonly object ExtractLock = new();

    /// <summary>
    /// Looks up the shell's icon for <paramref name="path"/>. Returns false, with
    /// <paramref name="icon"/> left null, for anything the shell could not answer for: a path
    /// that no longer exists and carries no recognisable extension, or a network path that
    /// answered with nothing. The caller owns the fallback; this method never draws one.
    ///
    /// Costs a real extraction on a cache miss, which can be slow for a network path that does
    /// not answer, so this should be called off the UI thread. Cheap on a cache hit.
    /// </summary>
    public static bool TryGet(string path, out ImageSource icon)
    {
        var found = Cache.GetOrAdd(path, Extract);
        icon = found!;
        return found is not null;
    }

    private static ImageSource? Extract(string path)
    {
        // See ExtractLock's own comment: this whole call is serialised, on purpose, because
        // SHGetFileInfo loses calls rather than merely slowing them down when several threads
        // reach it together.
        lock (ExtractLock)
        {
            // A path that is gone (deleted since Plith stat'd it, or a network share that does
            // not answer) cannot be asked "what icon do you have", only "what icon would a path
            // shaped like this have" - that is what SHGFI_USEFILEATTRIBUTES asks for instead,
            // going by the extension alone rather than touching the file system a second time.
            var exists = File.Exists(path) || Directory.Exists(path);
            var flags = SHGFI_ICON | SHGFI_LARGEICON;
            var attributes = 0u;
            if (!exists)
            {
                flags |= SHGFI_USEFILEATTRIBUTES;
                attributes = FILE_ATTRIBUTE_NORMAL;
            }

            var info = new SHFILEINFO();
            var listHandle = SHGetFileInfo(path, attributes, ref info, (uint)Marshal.SizeOf<SHFILEINFO>(), flags);
            if (listHandle == 0 || info.hIcon == 0) return null;

            try
            {
                var bitmap = Imaging.CreateBitmapSourceFromHIcon(
                    info.hIcon, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
                // Freeze, so the ImageSource can be built off the UI thread and handed to it.
                // Without this the extraction has to happen on the dispatcher, and a slow
                // network path stalls the surface for as long as the shell takes to answer.
                bitmap.Freeze();
                return bitmap;
            }
            catch (COMException)
            {
                // A handle SHGetFileInfo handed back but WPF could not turn into a bitmap.
                // Treated the same as no icon at all, so the caller falls back to the drawn
                // geometry.
                return null;
            }
            finally
            {
                // DestroyIcon in a finally, always. SHGetFileInfo hands over an HICON that
                // belongs to the caller, and a shelf that redraws on every change would leak one
                // per tile per repaint.
                DestroyIcon(info.hIcon);
            }
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct SHFILEINFO
    {
        public nint hIcon;
        public int iIcon;
        public uint dwAttributes;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szDisplayName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 80)]
        public string szTypeName;
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint SHGetFileInfo(
        string pszPath, uint dwFileAttributes, ref SHFILEINFO psfi, uint cbFileInfo, uint uFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(nint hIcon);
}
