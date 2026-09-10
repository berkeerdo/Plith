using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Plith.Services;

namespace Plith.Views.Widgets;

/// <summary>
/// Time and date, centred in the frame.
///
/// The tick runs only while the widget is on screen. That is not a micro-optimisation: in notch
/// mode the OSD's window is never hidden, so a timer left running is a permanent cost on an
/// overlay that is invisible most of the time — the same rule the sky's storyboards follow, and
/// the reason the ledger's idle-resource item has to be measurable at all.
/// </summary>
public partial class ClockWidget : UserControl
{
    private readonly DispatcherTimer _tick;

    public ClockWidget()
    {
        InitializeComponent();

        _tick = new DispatcherTimer(DispatcherPriority.Background, Dispatcher)
        {
            Interval = TimeSpan.FromSeconds(1),
        };
        _tick.Tick += (_, _) => Render();

        // IsVisibleChanged rather than Loaded/Unloaded: the frame swaps pages by adding and
        // removing them, but it also stays loaded between opens, and a widget that only stopped
        // on Unloaded would keep ticking for the whole session after the first close.
        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) Start();
            else _tick.Stop();
        };
    }

    private void Start()
    {
        // Painted before the first tick, so a page that has been away for a minute is never
        // shown holding the time it had when it left.
        Render();
        _tick.Start();
    }

    private void Render()
    {
        // Reuses the ambient row's formatter rather than a second one: the clock in the notch
        // and the clock in the ambient row must never disagree about how a time is written.
        var (time, date) = AmbientFormatter.FormatClock(DateTime.Now, CultureInfo.CurrentCulture);
        Time.Text = time;
        Date.Text = date;

        // Read every tick rather than cached. GetSystemPowerStatus is a struct read, and the
        // states that matter — a cable going in, a machine dropping to low power — are exactly
        // the ones a cached value would show wrong for as long as the page stayed open.
        var (show, text, charging) = AmbientFormatter.FormatBattery(BatteryReader.Read());

        // Collapsed on a desktop, which the formatter decides: a battery line reading "no
        // battery" is worse than no line, and this is the same call the ambient row used.
        Battery.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        BatteryText.Text = text;
        ChargingBolt.Visibility = charging ? Visibility.Visible : Visibility.Collapsed;

        // The announced name is the whole reading, not just the digits: a screen reader user
        // landing on "21:04" alone has no way to know what it is. The battery joins it rather
        // than announcing separately — a StackPanel has no automation peer, so a name set there
        // would reach nothing at all.
        var announced = show ? $"{Time.Text}, {Date.Text}, battery {text}" : $"{Time.Text}, {Date.Text}";
        System.Windows.Automation.AutomationProperties.SetName(Time, announced);
    }
}
