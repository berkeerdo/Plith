namespace Plith.Cards;

/// <summary>
/// A self-contained OSD feature. Owns its view-model, its own visibility opinion, and its
/// own trigger conditions. Deliberately knows nothing about the window it renders in — the
/// view is resolved from <see cref="ViewModel"/>'s type via an implicit DataTemplate, which
/// is what keeps cards constructible in headless tests.
/// </summary>
public interface ICard
{
    /// <summary>Stable identifier, e.g. "audio" / "media". Used in ShowRequest.OriginCardId.</summary>
    string Id { get; }

    /// <summary>Render order inside the OSD stack; lower renders higher up. Spaced by 10
    /// so new cards can slot between existing ones without renumbering.</summary>
    int Order { get; }

    /// <summary>The card's own opinion on whether it has anything to show right now.</summary>
    bool IsVisible { get; }

    /// <summary>DataContext for the card's view.</summary>
    object ViewModel { get; }

    /// <summary>Raised when <see cref="IsVisible"/> flips. Must be raised on the UI
    /// dispatcher: <see cref="CardHost"/> reconciles it directly into an
    /// <see cref="System.Collections.ObjectModel.ObservableCollection{T}"/> bound to a
    /// live ItemsControl, which throws off-thread.</summary>
    event Action? VisibilityChanged;

    /// <summary>Raised when the card wants the OSD on screen. Must be raised on the UI
    /// dispatcher — see the note on <see cref="VisibilityChanged"/>.</summary>
    event Action<ShowRequest>? ShowRequested;

    /// <summary>Subscribe to whatever sources this card reads.</summary>
    void Activate();

    /// <summary>Unsubscribe. Must be safe to call without a prior Activate.</summary>
    void Deactivate();

    /// <summary>
    /// The active palette or accent changed; re-resolve any cached brushes. Default-empty
    /// so cards holding no brush cache ignore it.
    /// </summary>
    void OnThemeChanged() { }
}
