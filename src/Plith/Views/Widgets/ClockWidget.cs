using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Plith.Services;
using Plith.ViewModels;

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

    private readonly MediaViewModel? _media;

    /// <param name="media">Optional. Without it the page is the time and the battery, which is
    /// what it was — the track line never appears rather than appearing empty.</param>
    public ClockWidget(MediaViewModel? media = null)
    {
        InitializeComponent();
        _media = media;

        // Repainted on every media change, not only on the tick: a track can change while this
        // page is the one being looked at, and up to a second of saying nothing is long enough
        // to notice.
        if (_media is not null) _media.PropertyChanged += (_, _) => Render();

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

        // Painted once at construction as well. Binding the first paint to IsVisibleChanged
        // alone means a widget that has never been shown holds nothing at all — true offscreen,
        // and true for any host that lays the control out before it is on a rendered surface.
        // The timer still starts and stops with visibility; only the content is unconditional.
        Render();
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
        BatteryText.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        BatteryText.Text = text;
        ChargingBolt.Visibility = show && charging ? Visibility.Visible : Visibility.Collapsed;

        RenderNowPlaying();

        // The announced name is the whole reading, not just the digits: a screen reader user
        // landing on "21:04" alone has no way to know what it is. Everything joins it rather
        // than announcing separately — a StackPanel has no automation peer, so a name set there
        // would reach nothing at all.
        var announced = $"{Time.Text}, {Date.Text}";
        if (show) announced += $", battery {text}";
        if (NowPlaying.Visibility == Visibility.Visible) announced += $", playing {NowTitle.Text}";
        System.Windows.Automation.AutomationProperties.SetName(Time, announced);
    }

    /// <summary>
    /// The track line, present only while something is playing.
    ///
    /// Collapsed rather than blank, and the rule above it collapses with it: a divider over
    /// nothing says "more below" and then does not deliver it.
    /// </summary>
    private void RenderNowPlaying()
    {
        var playing = _media is { HasSession: true } && !string.IsNullOrWhiteSpace(_media.Title);

        NowPlaying.Visibility = playing ? Visibility.Visible : Visibility.Collapsed;
        Rule.Visibility = playing ? Visibility.Visible : Visibility.Collapsed;
        if (!playing) return;

        NowTitle.Text = string.IsNullOrWhiteSpace(_media!.Artist)
            ? _media.Title
            : $"{_media.Title} — {_media.Artist}";
        NowArt.Source = _media.AlbumArt;
    }
}
