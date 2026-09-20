using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// Now-playing card. Visible only while an SMTC session exists and the user hasn't asked for
/// compact mode — CompactMode means exactly "hide the media card" and nothing else, so the
/// rule belongs here rather than in the shell.
/// </summary>
public sealed class MediaCard : ICard
{
    private readonly SettingsService _settings;
    private bool _lastVisible;

    public MediaCard(SettingsService settings)
    {
        _settings = settings;
        Vm = new MediaViewModel();
        Vm.HasSessionChanged += OnHasSessionChanged;
        Vm.CommandRequested += OnCommandRequested;
        Vm.SeekRequested += OnSeekRequested;
        _lastVisible = IsVisible;
    }

    public string Id => "media";
    public string AccessibleName => "Now playing";
    public int Order => 10;
    public object ViewModel => Vm;
    public MediaViewModel Vm { get; }

    public bool IsVisible => Vm.HasSession && !_settings.Current.CompactMode;

    // Load-bearing for accessibility, not a debugging aid: WPF's ItemAutomationPeer names the
    // OSD's list container from this. See ICard.AccessibleName.
    public override string ToString() => AccessibleName;

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    /// <summary>Raised when the user clicks a transport button. The orchestrator dispatches it
    /// to the SMTC session.</summary>
    public event EventHandler<MediaCommand>? CommandInvoked;

    public void Activate() => _settings.Changed += OnSettingsChanged;

    public void Deactivate() => _settings.Changed -= OnSettingsChanged;

    public void Apply(MediaSnapshot snapshot)
    {
        Vm.Apply(snapshot);
        RaiseVisibilityIfChanged();

        if (_settings.Current.AutoShowOnMedia && snapshot.HasSession)
            ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaChange, Id));
    }

    /// <summary>
    /// A new position for the track already showing.
    ///
    /// Deliberately not routed through <see cref="Apply"/>: Apply raises ShowRequested when
    /// AutoShowOnMedia is on, and this arrives about once a second, so the OSD would be summoned
    /// every second by a bar moving. It also cannot change IsVisible, so there is nothing to
    /// reconcile.
    /// </summary>
    public void ApplyTimeline(MediaTimeline? timeline) => Vm.Timeline = timeline;

    private void OnSettingsChanged(SettingsModel m) => RaiseVisibilityIfChanged();

    private void OnHasSessionChanged() => RaiseVisibilityIfChanged();

    private void OnCommandRequested(MediaCommand command)
    {
        CommandInvoked?.Invoke(this, command);
        ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaCommand, Id));
    }

    /// <summary>Raised when the user has finished dragging the track. The orchestrator writes it
    /// to the SMTC session.</summary>
    public event EventHandler<TimeSpan>? SeekInvoked;

    /// <summary>
    /// The user dragged the track and let go.
    ///
    /// No ShowRequested, unlike a transport command, and that is the whole difference: a seek
    /// happens on a surface the person is already looking at and already holding open, so asking
    /// for a show would replace the page under their own hand with a HUD about it.
    /// </summary>
    private void OnSeekRequested(TimeSpan position) => SeekInvoked?.Invoke(this, position);

    // Both inputs to IsVisible (HasSession, CompactMode) change independently, and either can
    // fire without the result actually flipping. Gate on the computed value so CardHost isn't
    // asked to reconcile on every settings save.
    private void RaiseVisibilityIfChanged()
    {
        bool now = IsVisible;
        if (now == _lastVisible) return;
        _lastVisible = now;
        VisibilityChanged?.Invoke();
    }
}
