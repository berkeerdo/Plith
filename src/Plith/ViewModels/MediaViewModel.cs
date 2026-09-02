using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows.Media.Imaging;
using Plith.Cards;
using Plith.Services;

namespace Plith.ViewModels;

public sealed class MediaViewModel : INotifyPropertyChanged
{
    private string _title = "";
    public string Title
    {
        get => _title;
        set
        {
            if (Set(ref _title, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    private string _artist = "";
    public string Artist
    {
        get => _artist;
        set
        {
            if (Set(ref _artist, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

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
            {
                OnPropertyChanged(nameof(PlayPauseGlyph));
                OnPropertyChanged(nameof(PlayPauseLabel));
                OnPropertyChanged(nameof(AccessibleSummary));
            }
        }
    }

    private bool _hasSession;
    public bool HasSession
    {
        get => _hasSession;
        set
        {
            if (Set(ref _hasSession, value))
            {
                HasSessionChanged?.Invoke();
                OnPropertyChanged(nameof(AccessibleSummary));
            }
        }
    }

    /// <summary>Live-region text for the media card.</summary>
    public string AccessibleSummary => _hasSession
        ? $"{_title} by {_artist}, {(_isPlaying ? "playing" : "paused")}"
        : string.Empty;

    /// <summary>Raised whenever HasSession flips, so MediaCard can recompute its
    /// <c>IsVisible</c> (which depends on HasSession + CompactMode). Always fires from
    /// the setter, so callers that go around <see cref="Apply"/> still get the notification.</summary>
    public event Action? HasSessionChanged;

    /// <summary>Segoe Fluent Icons glyph for the play/pause toggle button (U+E769 Pause / U+E768 Play).</summary>
    public string PlayPauseGlyph => _isPlaying ? "" : "";

    /// <summary>Screen-reader label for the play/pause toggle. The glyph beside it is a Segoe
    /// Fluent Icons private-use codepoint, which a screen reader would otherwise read aloud
    /// verbatim — this is the only text a non-sighted user gets for that button.</summary>
    public string PlayPauseLabel => _isPlaying ? "Pause" : "Play";

    /// <summary>Raised when the user clicks a transport button. The view calls
    /// <see cref="RequestCommand"/> on its DataContext rather than surfacing an event on the
    /// UserControl, because under CardHost the view is created by a DataTemplate and no owner
    /// holds a named reference to it.</summary>
    public event Action<MediaCommand>? CommandRequested;

    public void RequestCommand(MediaCommand command) => CommandRequested?.Invoke(command);

    /// <summary>
    /// Apply a fresh SMTC snapshot to this view-model. Must be called on the WPF dispatcher
    /// thread — the orchestrator marshals SMTC threadpool callbacks before invoking this,
    /// so any future caller has to match that contract.
    /// </summary>
    public void Apply(MediaSnapshot snapshot)
    {
        Title = snapshot.Title;
        Artist = snapshot.Artist;
        IsPlaying = snapshot.IsPlaying;
        HasSession = snapshot.HasSession;   // setter raises HasSessionChanged on actual change
        AlbumArt = DecodeThumbnail(snapshot.ThumbnailBytes);
    }

    private static BitmapImage? DecodeThumbnail(byte[]? bytes)
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
