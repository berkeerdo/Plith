using System.Collections.ObjectModel;
using System.Linq;
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

    /// <summary>The card currently holding the OSD to itself, or null. Cleared by the card
    /// going invisible or by any other card asking to be shown, so it can never outlive the
    /// appearance it belongs to.</summary>
    private string? _exclusiveCardId;

    private readonly Action<string>? _log;
    private string _lastLoggedVisible = "";

    /// <param name="log">Optional. Receives the visible card set whenever it changes, which is
    /// the one thing a log could never answer before: what was actually on screen. Kept as a
    /// plain delegate rather than a DiagnosticLog reference, so this class still depends on
    /// nothing it cannot be tested without.</param>
    public CardHost(SettingsService settings, IShowSuppressor? suppressor = null, Action<string>? log = null)
    {
        _settings = settings;
        _suppressor = suppressor;
        _log = log;
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
    /// <summary>
    /// Asks the shell to show the OSD. Carries the reason as well as the duration, because in
    /// notch mode the reason decides the SHAPE — a track change gets a wider HUD than a volume
    /// key — and a shell that had to infer it from the visible cards would be inferring
    /// something this class already knew.
    /// </summary>
    public event Action<ShowReason, TimeSpan>? ShowRequested;

    /// <summary>The OSD should disappear now, regardless of its hide timer.</summary>
    public event Action? HideRequested;

    public void Register(ICard card)
    {
        // Registration happens once, deterministically, at startup wiring time — closer to a DI
        // container's AddSingleton than to a runtime event. A re-registration here is always a
        // programming/config bug (Phase 6's data-driven registration listing a card twice), and
        // the failure mode it silently causes otherwise — a duplicated list entry plus a
        // double-subscribed ShowRequested, so the OSD pops or hides twice per event — surfaces
        // far from this call and is hard to trace back. Throwing at the call site instead points
        // straight at the bug while the stack still names the offending card.
        // TODO(Phase 6): this is reference-equality only (List<T>.Contains, default
        // comparer) — it catches the same instance registered twice but not two distinct
        // instances sharing an Id. Data-driven registration will also want duplicate-Id
        // detection. See docs/ROADMAP.md's Phase 5 section for why that's deferred.
        if (_cards.Contains(card))
            throw new InvalidOperationException($"Card '{card.Id}' is already registered.");

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

        // Set or cleared on every request, so an ordinary show ends an exclusive one rather
        // than being swallowed by it.
        _exclusiveCardId = request.Exclusive ? request.OriginCardId : null;

        RecomputeVisibleCards();

        var duration = request.DurationOverride
            ?? TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs);
        ShowRequested?.Invoke(request.Reason, duration);
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
        // An exclusive card that has gone invisible releases the surface. Checked here rather
        // than only where it is set, because the card's own hide timer is what usually ends it
        // and that arrives as a visibility change.
        var exclusive = _exclusiveCardId is null
            ? null
            : _cards.Find(c => c.Id == _exclusiveCardId && c.IsVisible);
        if (_exclusiveCardId is not null && exclusive is null) _exclusiveCardId = null;

        // _cards is already Order-sorted, so a positional in-place reconcile preserves order
        // without clearing the collection — clearing would make the ItemsControl rebuild every
        // card container and restart any animation the views own.
        int target = 0;
        foreach (var card in _cards)
        {
            if (!card.IsVisible) continue;
            if (exclusive is not null && !ReferenceEquals(card, exclusive)) continue;

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

        LogVisibleSet();
    }

    /// <summary>Report the visible set, and only when it actually changed. Recompute runs on
    /// every settings save and every visibility flip, most of which change nothing.</summary>
    private void LogVisibleSet()
    {
        if (_log is null) return;

        var ids = VisibleCards.Count == 0 ? "(none)" : string.Join(", ", VisibleCards.Select(c => c.Id));
        if (ids == _lastLoggedVisible) return;

        _lastLoggedVisible = ids;
        // "Visible set" rather than "on screen": this is the policy output, and it is recomputed
        // while the OSD is hidden too. A log that says "on screen" about a hidden window sends
        // the next reader looking for a bug that is not there.
        _log(_exclusiveCardId is null ? $"Visible set: {ids}." : $"Visible set: {ids} (exclusive).");
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
