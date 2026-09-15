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
    private readonly Func<WeatherSnapshot?>? _weather;
    private readonly Func<MicrophoneSnapshot?>? _microphone;

    /// <param name="media">Optional. Without it the page is the time and the battery, which is
    /// what it was — the track line never appears rather than appearing empty.</param>
    /// <param name="weather">Optional. The same reading the weather page draws its sky from —
    /// read here rather than mirrored, so the two can never disagree about the temperature.</param>
    /// <param name="microphone">Optional. Null, or a machine with no capture device, leaves the
    /// mic mark absent entirely rather than showing an unmuted microphone nobody has.</param>
    public ClockWidget(MediaViewModel? media = null, Func<WeatherSnapshot?>? weather = null,
                       Func<MicrophoneSnapshot?>? microphone = null)
    {
        InitializeComponent();
        _media = media;
        _weather = weather;
        _microphone = microphone;

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

    /// <summary>Repaint from outside, when something the page reads has changed but the page
    /// itself has no way of knowing. The tick would get there within a second; a mute is the one
    /// state where a second is long enough to say the wrong thing.</summary>
    public void Refresh() => Render();

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

        // Shown only while muted — see the comment on the mark itself.
        MicMark.Visibility = _microphone?.Invoke() is { Muted: true }
            ? Visibility.Visible
            : Visibility.Collapsed;

        RenderWeather();
        RenderNowPlaying();

        // The announced name is the whole reading, not just the digits: a screen reader user
        // landing on "21:04" alone has no way to know what it is. Everything joins it rather
        // than announcing separately — a StackPanel has no automation peer, so a name set there
        // would reach nothing at all.
        var announced = $"{Time.Text}, {Date.Text}";
        if (show) announced += $", battery {text}";
        if (MicMark.Visibility == Visibility.Visible) announced += ", microphone muted";
        if (NowPlaying.Visibility == Visibility.Visible) announced += $", playing {NowTitle.Text}";
        System.Windows.Automation.AutomationProperties.SetName(Time, announced);
    }

    /// <summary>
    /// The weather, if there is a reading worth showing.
    ///
    /// Stale readings collapse the block rather than showing an old number, which is the same
    /// rule the weather page follows — a temperature from three hours ago is a wrong answer
    /// presented as a right one.
    /// </summary>
    private void RenderWeather()
    {
        var snapshot = _weather?.Invoke();
        var fresh = snapshot is { } w && WeatherCodeMap.IsFresh(w, DateTimeOffset.Now, WeatherMaxAgeMinutes);

        WeatherBlock.Visibility = fresh ? Visibility.Visible : Visibility.Collapsed;
        if (!fresh) return;

        var reading = snapshot!.Value;
        Temperature.Text = string.Create(CultureInfo.CurrentCulture, $"{Math.Round(reading.TemperatureC):0}°");

        // The same SkyKind the weather page's gradient is drawn from, so the mark here and the
        // sky there can never describe different weather.
        Mark.Show(SkyCondition.From(reading.WeatherCode, DateTime.Now.Hour));

        // The word still exists, as the announced name. A shape is faster to read and says
        // nothing at all to a screen reader.
        System.Windows.Automation.AutomationProperties.SetName(
            Mark, WeatherCodeMap.Describe(reading.WeatherCode));
    }

    /// <summary>How old a reading may be and still be shown here. The same 45 minutes the
    /// ambient row used: three refresh cycles, so one failed fetch does not blank it.</summary>
    private const int WeatherMaxAgeMinutes = 45;

    /// <summary>
    /// The track line, present only while something is playing.
    ///
    /// Collapsed rather than blank, and the rule above it collapses with it: a divider over
    /// nothing says "more below" and then does not deliver it.
    /// </summary>
    private void RenderNowPlaying()
    {
        var playing = _media is { HasSession: true } && !string.IsNullOrWhiteSpace(_media.Title);

        // No divider any more: the track line sits on the page's bottom edge with the readings
        // at the top, and space between two blocks says "separate" without a line drawn to say it.
        NowPlaying.Visibility = playing ? Visibility.Visible : Visibility.Collapsed;
        if (!playing) return;

        NowTitle.Text = string.IsNullOrWhiteSpace(_media!.Artist)
            ? _media.Title
            : $"{_media.Title} — {_media.Artist}";
        NowArt.Source = _media.AlbumArt;
    }
}
