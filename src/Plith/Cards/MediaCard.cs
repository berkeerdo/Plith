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

        // Called before the setting is consulted, and that is deliberate: the baseline has to
        // follow the session whether or not anyone is watching it, or switching the setting on
        // mid-session would make the next snapshot look like a change.
        var newTrack = RecordTrackAndDetectChange(snapshot);

        if (newTrack && _settings.Current.AutoShowOnMedia)
            ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaChange, Id));
    }

    /// <summary>
    /// The title this card last saw, or null before it has seen one.
    ///
    /// This is the whole fix for a setting that was called "Show on track change" and meant
    /// "show on any session event at all". SMTC raises MediaPropertiesChanged and
    /// PlaybackInfoChanged into the same handler, and the card used to ask only whether a
    /// session existed. So seeking inside a YouTube video, pausing it, and Windows handing the
    /// session to a Spotify that was sitting paused all popped the OSD.
    /// </summary>
    private string? _lastTitle;

    /// <summary>
    /// True while the next snapshot belongs to a session the person did not ask for.
    ///
    /// Windows moves the current session between players on its own: pausing a video hands it
    /// to a Spotify that is sitting paused, and resuming hands it back. By title alone that
    /// return is indistinguishable from a new track, so the swap itself has to say otherwise.
    /// </summary>
    private bool _sessionJustReplaced;

    /// <summary>
    /// The current session was swapped for another. Call before the snapshot that follows it.
    ///
    /// The snapshot after a swap is the new session's existing state, not an event: nothing
    /// started, the player Windows happened to pick simply changed. It still updates the
    /// baseline, so the track it carries is the one the next real change is measured against.
    /// </summary>
    public void NoteSessionReplaced() => _sessionJustReplaced = true;

    /// <summary>
    /// Whether this snapshot starts a track the person has not been shown, and remembers it
    /// either way.
    ///
    /// Title only, not title-and-artist. Sources fill their properties progressively, and an
    /// artist that lands a moment after the title would read as a second change and show twice
    /// for one track. For the same reason an empty title is not a track: a transition can pass
    /// through one, and treating it as real would show on the way out and again on the way in.
    ///
    /// The playing check is what separates a track change from a session change. Pausing a
    /// video hands the current session to whatever else holds one, and that session's title is
    /// genuinely different, but nobody played it, so it is not an event to announce.
    /// </summary>
    private bool RecordTrackAndDetectChange(MediaSnapshot snapshot)
    {
        if (!snapshot.HasSession) return false;

        var title = snapshot.Title;
        if (string.IsNullOrEmpty(title)) return false;

        // Null means this is the first title this card has seen: the session was already
        // playing when Plith looked at it. Nothing changed; the person did nothing.
        var changed = _lastTitle is not null && !string.Equals(_lastTitle, title, StringComparison.Ordinal);
        _lastTitle = title;

        // Consumed here rather than in Apply, so it is cleared by the snapshot it suppresses
        // and not by one that arrived without a title to record.
        if (_sessionJustReplaced)
        {
            _sessionJustReplaced = false;
            return false;
        }

        return changed && snapshot.IsPlaying;
    }

    private void OnSettingsChanged(SettingsModel m) => RaiseVisibilityIfChanged();

    private void OnHasSessionChanged() => RaiseVisibilityIfChanged();

    private void OnCommandRequested(MediaCommand command)
    {
        CommandInvoked?.Invoke(this, command);
        ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaCommand, Id));
    }

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
