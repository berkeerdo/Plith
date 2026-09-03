namespace Plith.Cards;

/// <summary>Why the OSD is being asked to appear. Phase 5 treats every reason identically;
/// Phase 6 notch modes use it to decide which cards a given trigger should surface.</summary>
public enum ShowReason
{
    AudioChange,
    MediaChange,
    MediaCommand,
    SummonHotkey,
    VolumeKey,
    EditModeExit,
}

/// <param name="Reason">What triggered the request.</param>
/// <param name="OriginCardId">The <see cref="ICard.Id"/> that raised it, or null for shell-level triggers.</param>
/// <param name="DurationOverride">Visible-for override; null means use SettingsModel.ShowDurationMs.</param>
public sealed record ShowRequest(
    ShowReason Reason,
    string? OriginCardId = null,
    TimeSpan? DurationOverride = null);
