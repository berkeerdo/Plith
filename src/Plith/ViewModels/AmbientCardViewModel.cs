using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;
using Plith.Services;

namespace Plith.ViewModels;

/// <summary>
/// Bindable surface for the ambient row. Holds formatted strings only — every decision about
/// what those strings say lives in AmbientFormatter, which is the half the tests can reach.
/// </summary>
public sealed class AmbientCardViewModel : INotifyPropertyChanged
{
    private string _clockTime = string.Empty;
    private string _clockDate = string.Empty;

    public string ClockTime { get => _clockTime; private set => Set(ref _clockTime, value); }
    public string ClockDate { get => _clockDate; private set => Set(ref _clockDate, value); }

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Culture is a parameter rather than read from the thread so a test can pin it.
    /// The card passes CultureInfo.CurrentCulture.</summary>
    public void ApplyClock(DateTime now, CultureInfo culture)
    {
        var (time, date) = AmbientFormatter.FormatClock(now, culture);
        ClockTime = time;
        ClockDate = date;
    }

    private void Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return;
        field = value;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name!));
    }
}
