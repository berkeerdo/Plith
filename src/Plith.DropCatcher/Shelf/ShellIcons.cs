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

    // A place to send anything that goes wrong, set once by whoever composes the catcher process
    // (ShelfWindow's constructor). Left null in the render harness and any other caller that
    // never sets it, in which case a failure is silent rather than thrown - logging is a nicety
    // this static class must not require a caller to provide.
    public static Action<string>? Log { get; set; }

    // Cached by path, because a repaint costs an extraction otherwise and the shelf repaints on
    // every selection change. Only a SUCCESS is cached: the catcher is one long-lived process
    // per signed-in session, and caching a failure would mean a share that was briefly offline
    // at first paint keeps showing the drawn fallback for that path until the person signs out,
    // even after the share answers again. A retry on every miss costs one more extraction for a
    // path that keeps failing, which is the cheaper mistake. The cache is not bounded by the
    // shelf's own cap of twenty: nothing evicts a key, so it holds one entry for every distinct
    // path this process has ever resolved a real icon for, for the life of the process.
    private static readonly ConcurrentDictionary<string, ImageSource> Cache =
        new(StringComparer.OrdinalIgnoreCase);

    // SHGetFileInfo is not safe to call from several threads at once: a shelf of six tiles fires
    // six Task.Run extractions together, and measured directly (a standalone repro outside this
    // process, six different paths, six concurrent calls, repeated), the first concurrent burst
    // in a fresh process loses several of them - not the slow one, different files each time -
    // while every call after that first burst succeeds. A lost call is not a slow one that would
    // eventually resolve: without the fix below it came back false, and (before the cache was
    // changed to never store a failure) Cache would have held that false forever. Serialising
    // every call behind this lock removed the failure across five repeated runs of that repro;
    // extraction still happens off the UI thread, it just also happens one at a time.
    //
    // What this has NOT settled: SHGetFileInfo's own documentation calls for COM to be
    // initialized on the calling thread, and Task.Run hands out MTA threadpool threads with no
    // CoInitialize. That fits the observed signature (first burst in a fresh process loses
    // calls, every burst after succeeds) at least as well as a plain thread-safety bug does, and
    // this lock was not tested against that specific hypothesis, it was tested against the
    // symptom, which it removed. Do not read the lock as proof the cause is understood; read it
    // as proof the symptom is gone. A future reader who understands the apartment question
    // should feel free to replace it with a narrower fix, but should not remove it on the belief
    // that "SHGetFileInfo is just fine on any thread": that was never established here.
    private static readonly object ExtractLock = new();

    /// <summary>
    /// Looks up the shell's icon for <paramref name="path"/>. Returns false, with
    /// <paramref name="icon"/> left null, for anything the shell could not answer for: a path
    /// that no longer exists and carries no recognisable extension, or a network path that
    /// answered with nothing. The caller owns the fallback; this method never draws one.
    ///
    /// Costs a real extraction on a cache miss, which can be slow for a network path that does
    /// not answer, so this should be called off the UI thread. Cheap on a cache hit. Prefer
    /// <see cref="TryGetCached"/> from the UI thread when only a hit is useful there.
    /// </summary>
    public static bool TryGet(string path, out ImageSource icon)
    {
        if (TryGetCached(path, out icon)) return true;

        var extracted = Extract(path);
        if (extracted is not null) Cache[path] = extracted;

        icon = extracted!;
        return extracted is not null;
    }

    /// <summary>
    /// The cache-only half of <see cref="TryGet"/>: never extracts, so it is cheap enough to call
    /// from the UI thread. Exists so a repaint that already knows the answer can paint the real
    /// icon on the very first frame instead of drawing the fallback and racing a background swap
    /// into it a moment later, harmless, but visible, on a surface meant to look finished.
    /// </summary>
    public static bool TryGetCached(string path, out ImageSource icon)
    {
        if (Cache.TryGetValue(path, out var cached))
        {
            icon = cached;
            return true;
        }

        icon = null!;
        return false;
    }

    private static BitmapSource? Extract(string path)
    {
        // A REAL PREVIEW FIRST, and it is the difference between a shelf that looks like a file
        // dialog and one that looks like what you put on it. The commonest thing on this shelf is
        // a screenshot, and a screenshot drawn as the generic picture icon says nothing: two of
        // them are the same tile. Reported as the page's design not saying what it is.
        //
        // THUMBNAILONLY, so this returns null for everything with no preview of its own: a
        // document, a folder, an exe. Those fall through to the shell icon below, which is the
        // right answer for them and was the only answer before.
        var thumbnail = TryThumbnail(path);
        if (thumbnail is not null) return thumbnail;

        // A path that is gone (deleted since Plith stat'd it, or a network share that does not
        // answer) cannot be asked "what icon do you have", only "what icon would a path shaped
        // like this have" - that is what SHGFI_USEFILEATTRIBUTES asks for instead, going by the
        // extension alone rather than touching the file system a second time. Probed OUTSIDE the
        // lock: neither call needs serialising, and a dead UNC path can block for the SMB
        // timeout, which would otherwise stall every other tile's extraction behind it too.
        var exists = File.Exists(path) || Directory.Exists(path);
        var flags = SHGFI_ICON | SHGFI_LARGEICON;
        var attributes = 0u;
        if (!exists)
        {
            flags |= SHGFI_USEFILEATTRIBUTES;
            attributes = FILE_ATTRIBUTE_NORMAL;
        }

        // See ExtractLock's own comment: this call is serialised, on purpose, because
        // SHGetFileInfo loses calls rather than merely slowing them down when several threads
        // reach it together.
        lock (ExtractLock)
        {
            var info = new SHFILEINFO();
            var listHandle = SHGetFileInfo(path, attributes, ref info, (uint)Marshal.SizeOf<SHFILEINFO>(), flags);

            // Checked first, before anything else looks at listHandle: an HICON is the caller's
            // to destroy the moment it exists, regardless of what the rest of the call reports.
            if (info.hIcon == 0) return null;

            try
            {
                // A defensive case, not one measured in the wild: SHGetFileInfo documents a zero
                // return as failure, and if that ever comes back paired with a non-zero hIcon
                // anyway, the handle is still destroyed below rather than trusted.
                if (listHandle == 0) return null;

                var bitmap = Imaging.CreateBitmapSourceFromHIcon(
                    info.hIcon, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
                // Freeze, so the ImageSource can be built off the UI thread and handed to it.
                // Without this the extraction has to happen on the dispatcher, and a slow
                // network path stalls the surface for as long as the shell takes to answer.
                bitmap.Freeze();
                return bitmap;
            }
            catch (Exception ex) when (ex is COMException or ArgumentException
                or OutOfMemoryException or InvalidOperationException)
            {
                // WPF's own HRESULT check routes a bad handle through
                // Marshal.GetExceptionForHR, so a failure here can surface as ArgumentException
                // (E_INVALIDARG) or OutOfMemoryException (E_OUTOFMEMORY, which for a GDI-backed
                // conversion like this one means the image data was unusable, not that the
                // process is actually out of memory) as readily as COMException, and a refused
                // Freeze throws InvalidOperationException. All four are treated the same as no
                // icon at all, so the caller falls back to the drawn geometry, but logged first,
                // because this call runs inside a Task.Run nothing awaits, and an exception that
                // is only swallowed there vanishes with no cache entry and no way to see it
                // happened at all.
                Log?.Invoke($"ShellIcons: extraction for '{path}' threw {ex.GetType().Name}: {ex.Message}");
                return null;
            }
            finally
            {
                // DestroyIcon unconditionally, always, whatever happened above. SHGetFileInfo
                // hands over an HICON that belongs to the caller, and a shelf that redraws on
                // every change would leak one per tile per repaint otherwise.
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

    /// <summary>
    /// The file's own thumbnail, or null when it has none.
    ///
    /// IShellItemImageFactory is what Explorer itself asks, so a file previews here exactly as it
    /// does in a folder window, including for anything a codec pack or a shell extension has
    /// taught Windows to render.
    ///
    /// ONE method on the interface, which is why this interop is low risk, and saying so is not
    /// idle: docs/PHASE6-VERIFICATION.md section 21 records an undocumented interface on this
    /// branch that needed ten reserved vtable slots, where nine compiled perfectly and landed the
    /// call on the wrong method. An interface with a single method has no ordering to get wrong.
    ///
    /// Asked for at 96 pixels rather than the tile's 22 DIP: this can be on a 200 per cent
    /// display, WPF scales a bitmap down cleanly and up badly, and the cache holds one image per
    /// path either way.
    /// </summary>
    private static BitmapSource? TryThumbnail(string path)
    {
        if (!File.Exists(path)) return null;

        nint bitmap = 0;
        IShellItemImageFactory? factory = null;
        try
        {
            var factoryId = typeof(IShellItemImageFactory).GUID;
            factory = (IShellItemImageFactory)SHCreateItemFromParsingName(path, 0, ref factoryId);

            // THUMBNAILONLY means "no icon, thank you". Without it every file comes back with
            // something and the shell-icon fallback would never run, which would cost documents
            // and folders the icon they should have. BIGGERSIZEOK lets the shell hand over a size
            // it already has cached rather than rescaling to order.
            var size = new SIZE { cx = 96, cy = 96 };
            if (factory.GetImage(size, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK, out bitmap) != 0)
                return null;
            if (bitmap == 0) return null;

            var source = Imaging.CreateBitmapSourceFromHBitmap(
                bitmap, 0, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
            source.Freeze();
            return source;
        }
        catch (Exception ex) when (ex is COMException or ArgumentException
            or OutOfMemoryException or InvalidOperationException or FileNotFoundException)
        {
            // A file the shell cannot preview, a provider that throws, a path that went away
            // between the check above and the call. Every one means "no thumbnail", which is a
            // fallback rather than a failure, so it is logged at the same level the icon path
            // logs its own refusals and nothing else happens.
            Log?.Invoke($"ShellIcons: no thumbnail for '{path}' ({ex.GetType().Name})");
            return null;
        }
        finally
        {
            // The HBITMAP is ours the moment GetImage succeeds, and
            // CreateBitmapSourceFromHBitmap copies rather than taking ownership.
            if (bitmap != 0) DeleteObject(bitmap);
            if (factory is not null) Marshal.ReleaseComObject(factory);
        }
    }

    private const uint SIIGBF_BIGGERSIZEOK = 0x00000001;
    private const uint SIIGBF_THUMBNAILONLY = 0x00000008;

    [StructLayout(LayoutKind.Sequential)]
    private struct SIZE
    {
        public int cx;
        public int cy;
    }

    /// <summary>The shell's image factory, as Explorer uses it.</summary>
    [ComImport]
    [Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItemImageFactory
    {
        [PreserveSig]
        int GetImage(SIZE size, uint flags, out nint bitmap);
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = false)]
    [return: MarshalAs(UnmanagedType.Interface)]
    private static extern object SHCreateItemFromParsingName(
        string path, nint bindContext, ref Guid riid);

    [DllImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(nint handle);
}
