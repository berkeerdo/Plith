namespace Plith.Cards;

/// <summary>
/// Consulted by <see cref="CardHost"/> before honouring any show request. Implemented in
/// Phase 5 by FullscreenVideoWatcher; the nullable constructor parameter means CardHost
/// works without one.
/// </summary>
public interface IShowSuppressor
{
    bool IsSuppressed { get; }

    /// <summary>Raised only on an actual transition, with the new value.</summary>
    event Action<bool>? SuppressionChanged;
}
