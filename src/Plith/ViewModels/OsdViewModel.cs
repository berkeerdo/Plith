using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.ViewModels;

/// <summary>
/// Transitional composite binding root for the OSD: the Audio card's view model, the Media
/// card's view model, and the media-visibility rule that used to live inline. Replaced by
/// OsdShellViewModel + CardHost in Task 6 of the Phase 5 plan.
/// </summary>
public sealed class OsdViewModel : INotifyPropertyChanged
{
    public AudioCardViewModel Audio { get; } = new();
    public MediaViewModel Media { get; }

    public OsdViewModel()
    {
        Media = new MediaViewModel();
        Media.HasSessionChanged += () => OnPropertyChanged(nameof(ShowMediaCard));
    }

    private bool _compactMode;
    public bool CompactMode
    {
        get => _compactMode;
        set
        {
            if (_compactMode == value) return;
            _compactMode = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(ShowMediaCard));
        }
    }

    /// <summary>The media card shows only when there's an active session AND the user hasn't
    /// asked for compact mode.</summary>
    public bool ShowMediaCard => Media.HasSession && !_compactMode;

    public event PropertyChangedEventHandler? PropertyChanged;
    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
