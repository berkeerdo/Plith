using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Media;
using Plith.Services;

namespace Plith.ViewModels;

/// <summary>
/// Source-agnostic view model for the Audio card. The orchestrator computes the normalized
/// bar fill and the display text from whichever source produced the change — Voicemeeter is
/// dB, Windows endpoint is percent — and hands the formatted result to <see cref="Apply"/>.
/// </summary>
public sealed class AudioCardViewModel : INotifyPropertyChanged
{
    public const float VoicemeeterMinDb = -60f;
    public const float VoicemeeterMaxDb = 12f;

    public AudioCardViewModel() => RefreshThresholdBrushes();

    private bool _useColorThresholds;
    public bool UseColorThresholds
    {
        get => _useColorThresholds;
        set
        {
            if (Set(ref _useColorThresholds, value))
                OnPropertyChanged(nameof(GainColor));
        }
    }

    private string _label = "Bus A1";
    public string Label
    {
        get => _label;
        set
        {
            if (Set(ref _label, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    private double _gainNormalized;
    public double GainNormalized
    {
        get => _gainNormalized;
        set
        {
            if (Set(ref _gainNormalized, Math.Clamp(value, 0, 1)))
            {
                OnPropertyChanged(nameof(GainColor));
                OnPropertyChanged(nameof(IsQuiet));
                OnPropertyChanged(nameof(IsNearThreshold));
            }
        }
    }

    /// <summary>Below a third, where the speaker icon drops its outer wave. Derived rather than
    /// stored, so it cannot disagree with the level it describes.</summary>
    public bool IsQuiet => _gainNormalized < 0.33;

    /// <summary>
    /// Whether the level is close enough to the caution threshold for the tick to be worth
    /// drawing.
    ///
    /// The tick exists to give the colour change a cause: appearing only near it means a person
    /// sees the mark before the bar changes colour, rather than seeing a colour change with
    /// nothing to explain it. Drawn from 70 % up, which is far enough ahead to be noticed.
    /// </summary>
    public bool IsNearThreshold => _gainNormalized >= 0.70;

    /// <summary>Where the caution threshold sits, 0..1. The one place this number lives; the
    /// colour logic below reads the same constant.</summary>
    public const double CautionThreshold = 0.85;

    /// <summary>
    /// The same number as an instance property, because XAML cannot bind to a const.
    ///
    /// Not a nicety: <c>Path="(vm:AudioCardViewModel.CautionThreshold)"</c> compiles — it is
    /// valid attached-property syntax — and then resolves to nothing at run time, handing the
    /// converter UnsetValue and drawing the tick at zero. A binding that fails silently in the
    /// one place a value has to be right is worse than no tick.
    /// </summary>
    /// <remarks>Instance rather than static despite touching no instance state, because XAML
    /// binds through the DataContext and a static member is not reachable that way. The analyser
    /// is right about the code and wrong about the requirement.</remarks>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Performance", "CA1822:Mark members as static",
        Justification = "Bound from XAML through the DataContext, which cannot reach a static member.")]
    public double CautionThresholdPosition => CautionThreshold;

    private string _busLine = string.Empty;
    /// <summary>Which rail this level belongs to, in words — see AudioLabel.BusLine. Set by the
    /// orchestrator, which is the only thing that knows.</summary>
    public string BusLine
    {
        get => _busLine;
        set => Set(ref _busLine, value);
    }

    private string _gainText = "0.0 dB";
    public string GainText
    {
        get => _gainText;
        set
        {
            if (Set(ref _gainText, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    private bool _muted;
    public bool Muted
    {
        get => _muted;
        set
        {
            if (Set(ref _muted, value))
            {
                OnPropertyChanged(nameof(GainColor));
                OnPropertyChanged(nameof(AccessibleSummary));
            }
        }
    }

    /// <summary>What a screen reader announces when the volume changes. The OSD never takes
    /// focus, so this live-region text is the only audio feedback a non-sighted user gets.</summary>
    public string AccessibleSummary => _muted ? $"{_label}, muted" : $"{_label}, {_gainText}";

    // Cached brush references resolved from the active OSD palette ResourceDictionary.
    // The XAML brushes themselves are shared instances; we cache the refs so GainColor stays
    // allocation-free in the hot path. RefreshThresholdBrushes() must be called whenever the
    // theme palette or the Theme Studio accent swaps (the ThemeService raises ThemeApplied
    // for both).
    //
    // Seeds match the dark-theme keys so unit tests (which run without an Application.Current
    // and therefore can't resolve from XAML resources) still observe the expected colour-
    // mapping logic. In production these are overwritten on the first
    // RefreshThresholdBrushes() call, which AudioCardViewModel's own ctor makes.
    private Brush _brushMuted = FreezeBrush(Color.FromRgb(0x80, 0x80, 0x80));
    private Brush _brushAccent = FreezeBrush(Color.FromRgb(0x4A, 0xD6, 0x95));
    private Brush _brushGreen = FreezeBrush(Color.FromRgb(0x4A, 0xD6, 0x95));
    private Brush _brushAmber = FreezeBrush(Color.FromRgb(0xF5, 0xC2, 0x42));
    private Brush _brushRed = FreezeBrush(Color.FromRgb(0xE5, 0x4B, 0x4B));

    private static SolidColorBrush FreezeBrush(Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        return b;
    }

    /// <summary>Re-resolves every OSD-facing brush (accent + threshold set) from
    /// <see cref="Application.Current"/>'s resources and fires <see cref="PropertyChanged"/>
    /// for <c>GainColor</c>. Call this after a theme palette swap OR an accent change so
    /// the volume bar picks up the new tint. Named "Threshold" for historical reasons;
    /// accent is included here because the same event triggers both refreshes.</summary>
    public void RefreshThresholdBrushes()
    {
        _brushMuted = ResolveBrush("OsdGainMuted", _brushMuted);
        _brushAccent = ResolveBrush("OsdAccent", _brushAccent);
        _brushGreen = ResolveBrush("OsdGainGreen", _brushGreen);
        _brushAmber = ResolveBrush("OsdGainAmber", _brushAmber);
        _brushRed = ResolveBrush("OsdGainRed", _brushRed);
        OnPropertyChanged(nameof(GainColor));
    }

    private static Brush ResolveBrush(string key, Brush fallback)
    {
        // Application.Current is null in unit tests / the XAML designer; keep the previous
        // resolved value (or the seed fallback) rather than crash.
        return Application.Current?.TryFindResource(key) is Brush b ? b : fallback;
    }

    public Brush GainColor
    {
        get
        {
            if (_muted) return _brushMuted;
            // Thresholds OFF (the default): the volume bar is the OSD's headline surface,
            // so it takes whichever accent the user picked in the Theme Studio. That's
            // what makes the picker feel real — before this, the bar stayed emerald no
            // matter what preset was selected.
            if (!_useColorThresholds) return _brushAccent;

            // Thresholds ON: amber and red are WARNINGS, and the safe level keeps the accent.
            //
            // It used to replace the accent with a fixed green below 0.70, which made the mode do
            // two things when only one of them is its purpose. The cost was reported directly:
            // with thresholds on - and they are on by default for anyone who turns them on once -
            // the bar showed the same green whatever colour had been chosen, so the accent looked
            // broken rather than overridden. The safety signal is the amber and the red; the
            // green was never part of it.
            //
            // Unless the accent is itself a warning colour. An amber accent under an amber
            // caution is a warning that cannot be seen, so in that case the green comes back and
            // the mode behaves as it did. Measured by hue distance rather than assumed - see
            // AccentLooksLikeAWarning.
            var safe = AccentLooksLikeAWarning() ? _brushGreen : _brushAccent;

            return _gainNormalized switch
            {
                // Heuristic thresholds that work for both Voicemeeter dB and Windows scalar:
                // 0.70 ≈ -7 dB on the VM scale, 70 % on the Windows scale.
                // 0.90 ≈  6 dB on the VM scale, 90 % on the Windows scale.
                <= 0.70 => safe,
                <= 0.90 => _brushAmber,
                _       => _brushRed,
            };
        }
    }

    /// <summary>
    /// Whether the accent sits close enough to amber or red that using it as the safe colour
    /// would hide the warning.
    ///
    /// Hue distance, not a guess. Amber and red occupy the warm end - roughly 0째 to 60째 - and an
    /// accent in that arc would leave caution and danger indistinguishable from normal. Anything
    /// outside it can carry the safe state without weakening the signal, which is most accents.
    ///
    /// Saturation matters too: a near-grey accent has no hue worth comparing, and it contrasts
    /// with amber and red by luminance anyway.
    /// </summary>
    private bool AccentLooksLikeAWarning()
    {
        if (_brushAccent is not SolidColorBrush accent) return false;

        var (h, s, _) = Plith.Services.AccentTheme.RgbToHsl(accent.Color);
        if (s < 0.25) return false;

        const double WarmArcEnd = 60.0;
        return h <= WarmArcEnd || h >= 360.0 - 10.0;
    }

    /// <summary>Voicemeeter back-compat path — derives normalized + dB text from the snapshot.
    /// Decibel values use InvariantCulture: technical / audio-engineering convention is the
    /// period separator regardless of host locale, and the previous CurrentCulture behaviour
    /// surfaced "0,0 dB" on tr-TR / de-DE / fr-FR machines.</summary>
    public void Apply(VoicemeeterParameterSnapshot snapshot)
    {
        double normalized = (Math.Clamp(snapshot.GainDb, VoicemeeterMinDb, VoicemeeterMaxDb) - VoicemeeterMinDb)
                          / (VoicemeeterMaxDb - VoicemeeterMinDb);
        string text = snapshot.Muted
            ? "MUTED"
            : snapshot.GainDb.ToString("+0.0;-0.0;0.0", CultureInfo.InvariantCulture) + " dB";
        Apply(snapshot.Label, normalized, text, snapshot.Muted);
    }

    public void Apply(string label, double normalized, string text, bool muted)
    {
        Label = label;
        GainNormalized = normalized;
        GainText = muted ? "MUTED" : text;
        Muted = muted;
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(name);
        return true;
    }
}
