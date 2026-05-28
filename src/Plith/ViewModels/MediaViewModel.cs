using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows.Media.Imaging;
using Plith.Services;

namespace Plith.ViewModels;

public sealed class MediaViewModel : INotifyPropertyChanged
{
    private string _title = "";
    public string Title { get => _title; set => Set(ref _title, value); }

    private string _artist = "";
    public string Artist { get => _artist; set => Set(ref _artist, value); }

    private BitmapSource? _albumArt;
    public BitmapSource? AlbumArt
    {
        get => _albumArt;
        set
        {
            if (Set(ref _albumArt, value))
                OnPropertyChanged(nameof(HasAlbumArt));
        }
    }

    public bool HasAlbumArt => _albumArt is not null;

    private bool _isPlaying;
    public bool IsPlaying
    {
        get => _isPlaying;
        set
        {
            if (Set(ref _isPlaying, value))
                OnPropertyChanged(nameof(PlayPauseGlyph));
        }
    }

    private bool _hasSession;
    public bool HasSession { get => _hasSession; set => Set(ref _hasSession, value); }

    /// <summary>Segoe Fluent Icons glyph for the play/pause toggle button (U+E769 Pause / U+E768 Play).</summary>
    public string PlayPauseGlyph => _isPlaying ? "" : "";

    public void Apply(MediaSnapshot snapshot)
    {
        Title = snapshot.Title;
        Artist = snapshot.Artist;
        IsPlaying = snapshot.IsPlaying;
        HasSession = snapshot.HasSession;
        AlbumArt = DecodeThumbnail(snapshot.ThumbnailBytes);
    }

    private static BitmapSource? DecodeThumbnail(byte[]? bytes)
    {
        if (bytes is null || bytes.Length == 0) return null;
        try
        {
            // OnLoad fully decodes inside EndInit(), so the stream is safe to dispose
            // immediately — without `using`, the MemoryStream leaks per track change.
            using var ms = new MemoryStream(bytes);
            var bitmap = new BitmapImage();
            bitmap.BeginInit();
            bitmap.CacheOption = BitmapCacheOption.OnLoad;
            bitmap.StreamSource = ms;
            bitmap.DecodePixelWidth = 96; // capped — we only display 48 dip ≤ ~96 px at 200% DPI
            bitmap.EndInit();
            bitmap.Freeze(); // cross-thread safe
            return bitmap;
        }
        catch
        {
            return null;
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(name);
        return true;
    }
}
