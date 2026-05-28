using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;
using System.Windows.Media;
using Plith.Services;

namespace Plith.ViewModels;

/// <summary>
/// Source-agnostic view model the OSD binds to. The orchestrator computes the normalized bar
/// fill and the display text from whichever source produced the change — Voicemeeter is dB,
/// Windows endpoint is percent — and just hands the formatted result to <see cref="Apply"/>.
/// </summary>
public sealed class OsdViewModel : INotifyPropertyChanged
{
    public const float VoicemeeterMinDb = -60f;
    public const float VoicemeeterMaxDb = 12f;

    public MediaViewModel Media { get; }

    public OsdViewModel()
    {
        Media = new MediaViewModel();
        // ShowMediaCard depends on Media.HasSession too — bubble that change up.
        Media.HasSessionChanged += () => OnPropertyChanged(nameof(ShowMediaCard));
    }

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

    private bool _compactMode;
    public bool CompactMode
    {
        get => _compactMode;
        set
        {
            if (Set(ref _compactMode, value))
                OnPropertyChanged(nameof(ShowMediaCard));
        }
    }

    /// <summary>The media card is shown only when there's an active session AND the user hasn't
    /// asked for compact mode.</summary>
    public bool ShowMediaCard => Media.HasSession && !_compactMode;

    private string _label = "Bus A1";
    public string Label { get => _label; set => Set(ref _label, value); }

    private double _gainNormalized;
    public double GainNormalized
    {
        get => _gainNormalized;
        set
        {
            if (Set(ref _gainNormalized, Math.Clamp(value, 0, 1)))
                OnPropertyChanged(nameof(GainColor));
        }
    }

    private string _gainText = "0.0 dB";
    public string GainText { get => _gainText; set => Set(ref _gainText, value); }

    private bool _muted;
    public bool Muted
    {
        get => _muted;
        set
        {
            if (Set(ref _muted, value))
                OnPropertyChanged(nameof(GainColor));
        }
    }

    // Cached frozen brushes so the GainColor getter is allocation-free in the hot path —
    // it fires on every poll tick (tens of Hz) and on every binding refresh. Frozen brushes
    // are also cross-thread safe and skip the WPF render-thread copy.
    private static readonly SolidColorBrush BrushMuted = FreezeBrush(Color.FromRgb(0x80, 0x80, 0x80));
    private static readonly SolidColorBrush BrushGreen = FreezeBrush(Color.FromRgb(0x4A, 0xD6, 0x95));
    private static readonly SolidColorBrush BrushAmber = FreezeBrush(Color.FromRgb(0xF5, 0xC2, 0x42));
    private static readonly SolidColorBrush BrushRed = FreezeBrush(Color.FromRgb(0xE5, 0x4B, 0x4B));

    private static SolidColorBrush FreezeBrush(Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        return b;
    }

    public Brush GainColor
    {
        get
        {
            if (_muted) return BrushMuted;
            if (!_useColorThresholds) return BrushGreen;
            return _gainNormalized switch
            {
                // Heuristic thresholds that work for both Voicemeeter dB and Windows scalar:
                // 0.70 ≈ -7 dB on the VM scale, 70 % on the Windows scale.
                // 0.90 ≈  6 dB on the VM scale, 90 % on the Windows scale.
                <= 0.70 => BrushGreen,
                <= 0.90 => BrushAmber,
                _       => BrushRed,
            };
        }
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
