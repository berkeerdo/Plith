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

    public string ClockTime
    {
        get => _clockTime;
        private set
        {
            if (Set(ref _clockTime, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    public string ClockDate
    {
        get => _clockDate;
        private set
        {
            if (Set(ref _clockDate, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    /// <summary>What a screen reader announces for the whole ambient row — bound on
    /// AmbientCardView's UserControl root together with AutomationProperties.LiveSetting,
    /// the same place AudioCardViewModel.AccessibleSummary is bound on AudioCardView. Only the
    /// clock exists today, so this is just the clock segment; Tasks 3 (battery) and 7 (weather)
    /// each extend this by appending their own ", "-joined segment when their column has
    /// content, following AudioCardViewModel's precedent of one computed summary property
    /// rather than a name per sub-element. That is the contract those tasks read.
    ///
    /// Battery appends here (Task 3): a ", Battery {text}[, charging]" segment when HasBattery
    /// is true, and nothing at all — no empty fragment, no dangling separator — when the
    /// column is collapsed. The StackPanel column itself carries no AutomationProperties.Name
    /// of its own; WPF gives panels no automation peer, so a name set there would never reach
    /// UI Automation. This composed property is the only place the battery announces.</summary>
    public string AccessibleSummary
    {
        get
        {
            var summary = $"Time {ClockTime}, {ClockDate}";
            if (HasBattery)
                summary += $", Battery {BatteryText}{(IsCharging ? ", charging" : string.Empty)}";
            return summary;
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Culture is a parameter rather than read from the thread so a test can pin it.
    /// The card passes CultureInfo.CurrentCulture.</summary>
    public void ApplyClock(DateTime now, CultureInfo culture)
    {
        var (time, date) = AmbientFormatter.FormatClock(now, culture);
        ClockTime = time;
        ClockDate = date;
    }

    private bool _hasBattery;
    private string _batteryText = string.Empty;
    private bool _isCharging;

    public bool HasBattery
    {
        get => _hasBattery;
        private set
        {
            if (Set(ref _hasBattery, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    public string BatteryText
    {
        get => _batteryText;
        private set
        {
            if (Set(ref _batteryText, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    public bool IsCharging
    {
        get => _isCharging;
        private set
        {
            if (Set(ref _isCharging, value))
                OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    /// <summary>Apply a power reading to the battery column. Show=false (desktop, unknown
    /// percent, or a failed read) collapses the column and drops its AccessibleSummary
    /// segment — see AmbientFormatter.FormatBattery for the decision.</summary>
    public void ApplyBattery(BatteryStatusRaw? raw)
    {
        var (show, text, charging) = AmbientFormatter.FormatBattery(raw);
        HasBattery = show;
        BatteryText = text;
        IsCharging = charging;
    }

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name!));

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(name);
        return true;
    }
}
