using System.Collections.ObjectModel;
using Plith.Services;

namespace Plith.Cards;

/// <summary>
/// Owns the registered card set, decides which cards are visible, and is the single
/// authority for when the OSD appears.
///
/// Deliberately holds no reference to any WPF window: it raises <see cref="ShowRequested"/>
/// and <see cref="HideRequested"/> and lets OsdHost subscribe. That constraint is what makes
/// the show policy unit-testable with no Application and no HWND, and it is the reason this
/// type must never grow a Window, Dispatcher, or Visual dependency of its own.
///
/// That rule is about this type's own implementation, not about which thread calls it:
/// <see cref="VisibleCards"/> is an <see cref="ObservableCollection{T}"/> bound directly to
/// a live <c>ItemsControl</c>, so every member of this class — <see cref="Register"/>,
/// <see cref="Start"/>, <see cref="RequestShow"/>, and both <see cref="ICard"/> events it
/// subscribes to — must be called on the UI dispatcher. A future card that raises
/// <see cref="ICard.VisibilityChanged"/> or <see cref="ICard.ShowRequested"/> from a worker
/// thread (a WMI or COM callback, for example) will throw a <see cref="NotSupportedException"/>
/// deep inside the WPF binding engine, far from this file.
/// </summary>
public sealed class CardHost : IDisposable
{
    private readonly SettingsService _settings;
    private readonly IShowSuppressor? _suppressor;
    private readonly List<ICard> _cards = new();
    private bool _disposed;

    public CardHost(SettingsService settings, IShowSuppressor? suppressor = null)
    {
        _settings = settings;
        _suppressor = suppressor;
        if (_suppressor is not null)
            _suppressor.SuppressionChanged += OnSuppressionChanged;
    }

    /// <summary>Every registered card, sorted by <see cref="ICard.Order"/>.</summary>
    public IReadOnlyList<ICard> Cards => _cards;

    /// <summary>The suppressor this host was constructed with, if any. Exposed so a second
    /// show authority outside CardHost (currently OsdHost.OnMouseEnter's hover keep-alive)
    /// can consult the same suppression state RequestShow gates on, instead of resurrecting
    /// the OSD independently of the one authority this class is meant to be.</summary>
    public IShowSuppressor? Suppressor => _suppressor;

    /// <summary>Policy output: the cards that should render right now, in Order.
    /// Bound directly by OsdShellViewModel.</summary>
    public ObservableCollection<ICard> VisibleCards { get; } = new();

    /// <summary>The OSD should appear for this long.</summary>
    public event Action<TimeSpan>? ShowRequested;

    /// <summary>The OSD should disappear now, regardless of its hide timer.</summary>
    public event Action? HideRequested;

    public void Register(ICard card)
    {
        // Keep _cards sorted on insert so Cards and VisibleCards share one ordering rule.
        int index = _cards.FindIndex(c => c.Order > card.Order);
        if (index < 0) _cards.Add(card); else _cards.Insert(index, card);

        card.VisibilityChanged += OnCardVisibilityChanged;
        card.ShowRequested += OnCardShowRequested;
        RecomputeVisibleCards();
    }

    public void Start()
    {
        foreach (var card in _cards) card.Activate();
        RecomputeVisibleCards();
    }

    /// <summary>Fan a theme/accent swap out to every card, visible or not — an invisible
    /// card must already hold correct brushes by the time it becomes visible.</summary>
    public void NotifyThemeChanged()
    {
        foreach (var card in _cards) card.OnThemeChanged();
    }

    public void RequestShow(ShowRequest request)
    {
        if (_disposed) return;
        if (_suppressor?.IsSuppressed == true) return;

        RecomputeVisibleCards();

        var duration = request.DurationOverride
            ?? TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs);
        ShowRequested?.Invoke(duration);
    }

    private void OnCardShowRequested(ShowRequest request) => RequestShow(request);

    // A card going away mid-display must collapse the OSD in place without re-popping it —
    // this is how ShowMediaCard behaved in 0.1.5 and the behaviour must not change.
    private void OnCardVisibilityChanged() => RecomputeVisibleCards();

    private void OnSuppressionChanged(bool suppressed)
    {
        if (_disposed) return;
        // Only the rising edge matters: suppression turning on must pull an on-screen OSD
        // down immediately rather than let it ride out its hide timer.
        if (suppressed) HideRequested?.Invoke();
    }

    private void RecomputeVisibleCards()
    {
        // _cards is already Order-sorted, so a positional in-place reconcile preserves order
        // without clearing the collection — clearing would make the ItemsControl rebuild every
        // card container and restart any animation the views own.
        int target = 0;
        foreach (var card in _cards)
        {
            if (!card.IsVisible) continue;

            if (target < VisibleCards.Count && ReferenceEquals(VisibleCards[target], card))
            {
                target++;
                continue;
            }

            int existing = VisibleCards.IndexOf(card);
            if (existing >= 0) VisibleCards.Move(existing, target);
            else VisibleCards.Insert(target, card);
            target++;
        }

        while (VisibleCards.Count > target) VisibleCards.RemoveAt(VisibleCards.Count - 1);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (_suppressor is not null)
            _suppressor.SuppressionChanged -= OnSuppressionChanged;

        foreach (var card in _cards)
        {
            card.VisibilityChanged -= OnCardVisibilityChanged;
            card.ShowRequested -= OnCardShowRequested;
            card.Deactivate();
        }
    }
}
