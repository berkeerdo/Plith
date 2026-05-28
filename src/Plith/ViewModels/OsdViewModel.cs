using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows.Media;
using Plith.Services;

namespace Plith.ViewModels;

public sealed class OsdViewModel : INotifyPropertyChanged
{
    public const float MinDb = -60f;
    public const float MaxDb = 12f;

    public MediaViewModel Media { get; } = new();

    private string _label = "Bus A1";
    public string Label { get => _label; set => Set(ref _label, value); }

    private float _gainDb = 0f;
    public float GainDb
    {
        get => _gainDb;
        set
        {
            if (Set(ref _gainDb, value))
            {
                OnPropertyChanged(nameof(GainNormalized));
                OnPropertyChanged(nameof(GainText));
                OnPropertyChanged(nameof(GainColor));
            }
        }
    }

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

    public double GainNormalized
    {
        get
        {
            var clamped = Math.Clamp(_gainDb, MinDb, MaxDb);
            return (clamped - MinDb) / (MaxDb - MinDb);
        }
    }

    public string GainText => Muted ? "MUTED" : $"{_gainDb:+0.0;-0.0;0.0} dB";

    public Brush GainColor
    {
        get
        {
            if (Muted) return new SolidColorBrush(Color.FromRgb(0x80, 0x80, 0x80));
            return _gainDb switch
            {
                <= 0f  => new SolidColorBrush(Color.FromRgb(0x4A, 0xD6, 0x95)), // green
                <= 6f  => new SolidColorBrush(Color.FromRgb(0xF5, 0xC2, 0x42)), // amber
                _      => new SolidColorBrush(Color.FromRgb(0xE5, 0x4B, 0x4B)), // red
            };
        }
    }

    public void Apply(VoicemeeterParameterSnapshot snapshot)
    {
        Label = snapshot.Label;
        GainDb = snapshot.GainDb;
        Muted = snapshot.Muted;
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
