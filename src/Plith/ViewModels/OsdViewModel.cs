using System.ComponentModel;
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

    public MediaViewModel Media { get; } = new();

    private string _label = "Bus A1";
    public string Label { get => _label; set => Set(ref _label, value); }

    private double _gainNormalized;
    public double GainNormalized
    {
        get => _gainNormalized;
        set => Set(ref _gainNormalized, Math.Clamp(value, 0, 1));
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

    public Brush GainColor => _muted
        ? new SolidColorBrush(Color.FromRgb(0x80, 0x80, 0x80))
        : new SolidColorBrush(Color.FromRgb(0x4A, 0xD6, 0x95));

    /// <summary>Voicemeeter back-compat path — derives normalized + dB text from the snapshot.</summary>
    public void Apply(VoicemeeterParameterSnapshot snapshot)
    {
        double normalized = (Math.Clamp(snapshot.GainDb, VoicemeeterMinDb, VoicemeeterMaxDb) - VoicemeeterMinDb)
                          / (VoicemeeterMaxDb - VoicemeeterMinDb);
        string text = snapshot.Muted ? "MUTED" : $"{snapshot.GainDb:+0.0;-0.0;0.0} dB";
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
