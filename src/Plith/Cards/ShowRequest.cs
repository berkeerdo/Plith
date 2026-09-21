namespace Plith.Cards;

/// <summary>Why the OSD is being asked to appear. Phase 5 treats every reason identically;
/// Phase 6 notch modes use it to decide which cards a given trigger should surface.</summary>
public enum ShowReason
{
    AudioChange,
    BrightnessChange,
    MediaChange,
    MediaCommand,
    SummonHotkey,
    VolumeKey,
    EditModeExit,
}

/// <param name="Reason">What triggered the request.</param>
/// <param name="OriginCardId">The <see cref="ICard.Id"/> that raised it, or null for shell-level triggers.</param>
/// <param name="DurationOverride">Visible-for override; null means use SettingsModel.ShowDurationMs.</param>
/// <param name="Exclusive">The OSD should show this card ALONE for as long as it stays visible.
///
/// The Audio card is always visible, because the OSD began as a volume OSD. Once other cards
/// could raise it, that default meant a brightness key press put a volume bar on screen: an
/// answer to a question nobody asked, next to the one they did. A card that owns its own
/// gesture asks for the surface rather than sharing it.</param>
public sealed record ShowRequest(
    ShowReason Reason,
    string? OriginCardId = null,
    TimeSpan? DurationOverride = null,
    bool Exclusive = false);
