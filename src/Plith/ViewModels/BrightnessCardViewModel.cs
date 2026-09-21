using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;

namespace Plith.ViewModels;

/// <summary>
/// What the Brightness card shows: one number and the bar that draws it.
/// </summary>
public sealed class BrightnessCardViewModel : INotifyPropertyChanged
{
    private int _percent;

    /// <summary>0 to 100, already normalised from the device's own span by the caller.</summary>
    public int Percent
    {
        get => _percent;
        set
        {
            if (!Set(ref _percent, value)) return;
            OnPropertyChanged(nameof(Normalized));
            OnPropertyChanged(nameof(DisplayText));
            OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    public double Normalized => Math.Clamp(_percent / 100.0, 0, 1);

    public string DisplayText => string.Create(CultureInfo.CurrentCulture, $"{_percent}%");

    /// <summary>Read by the card view's live region. See AudioCardView for why the
    /// AutomationProperties belong on the UserControl root.</summary>
    public string AccessibleSummary => string.Create(CultureInfo.CurrentCulture, $"Brightness {_percent} percent");

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
