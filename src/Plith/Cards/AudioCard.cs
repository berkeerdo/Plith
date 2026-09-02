using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// Volume card. Always visible — the OSD has no state in which it shows nothing about audio.
///
/// Owns the baseline-suppression rule that used to live in OsdOrchestrator.HandleValueChange:
/// the first read after attaching to a source establishes a baseline silently, so switching
/// sources doesn't pop the OSD with whatever value the new source happened to hold.
/// </summary>
public sealed class AudioCard : ICard
{
    // Matches the threshold OsdOrchestrator used before the refactor. Small enough to catch a
    // single volume-key step, large enough to ignore float noise from repeated reads.
    private const double ChangeEpsilon = 0.0005;

    private readonly SettingsService _settings;
    private float? _lastNormalized;
    private bool? _lastMuted;

    public AudioCard(SettingsService settings)
    {
        _settings = settings;
        Vm = new AudioCardViewModel { UseColorThresholds = settings.Current.UseColorThresholds };
    }

    public string Id => "audio";
    public int Order => 20;
    public bool IsVisible => true;
    public object ViewModel => Vm;
    public AudioCardViewModel Vm { get; }

    // Never raised: IsVisible is a constant. Empty accessors rather than a field-like event,
    // because a field-like event that is never invoked trips CS0067.
    public event Action? VisibilityChanged { add { } remove { } }
    public event Action<ShowRequest>? ShowRequested;

    public void Activate() => _settings.Changed += OnSettingsChanged;
    public void Deactivate() => _settings.Changed -= OnSettingsChanged;

    public void OnThemeChanged() => Vm.RefreshThresholdBrushes();

    private void OnSettingsChanged(SettingsModel m) => Vm.UseColorThresholds = m.UseColorThresholds;

    /// <summary>
    /// Applies a normalized 0..1 value plus a pre-formatted display string. The view model is
    /// updated on every call; the show request fires only on a real change after a baseline.
    /// </summary>
    public void Apply(string label, double normalized, string text, bool muted)
    {
        bool isFirstRead = _lastNormalized is null;
        bool changed = isFirstRead
            || Math.Abs(_lastNormalized!.Value - normalized) > ChangeEpsilon
            || _lastMuted != muted;

        _lastNormalized = (float)normalized;
        _lastMuted = muted;

        Vm.Apply(label, normalized, text, muted);

        if (changed && !isFirstRead)
            ShowRequested?.Invoke(new ShowRequest(ShowReason.AudioChange, Id));
    }

    /// <summary>Forget the baseline so the next <see cref="Apply"/> is silent. Called on every
    /// audio-source transition — the new source's first value is not a user action.</summary>
    public void ResetBaseline()
    {
        _lastNormalized = null;
        _lastMuted = null;
    }
}
