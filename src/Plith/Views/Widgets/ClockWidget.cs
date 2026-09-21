using System.Globalization;
using System.Linq;
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
        var (digits, meridiem) = AmbientFormatter.FormatClockParts(DateTime.Now, CultureInfo.CurrentCulture);
        Time.Text = digits;
        Meridiem.Text = meridiem;
        Meridiem.Visibility = string.IsNullOrEmpty(meridiem) ? Visibility.Collapsed : Visibility.Visible;
        Date.Text = date;
        Weekday.Text = AmbientFormatter.FormatWeekday(DateTime.Now, CultureInfo.CurrentCulture);

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
        // The FULL time here, designator included, rather than the digits the page draws large:
        // a screen reader saying "12:49" on a 12-hour machine has dropped the half that says
        // which 12:49 it is. `time` is FormatClock's one-string form, which is why that method
        // stays.
        var announced = $"{time}, {Date.Text}";
        if (show) announced += $", battery {text}";
        if (MicMark.Visibility == Visibility.Visible) announced += ", microphone muted";
        // The WORD follows IsPlaying, not the row's visibility. The row shows whatever the
        // session holds, playing or paused, and this used to announce "playing" for both: found
        // in the evidence of a hardware verdict on 2026-09-21, where SMTC reported the session
        // paused and the clock page announced "playing Gotta Be Cool". A screen reader was being
        // told something the product knew to be false.
        if (NowPlaying.Visibility == Visibility.Visible)
        {
            var verb = _media?.IsPlaying == true ? "playing" : "paused";
            announced += $", {verb} {NowTitle.Text}";
        }
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
        if (!fresh)
        {
            Range.Visibility = Visibility.Collapsed;
            Forecast.Visibility = Visibility.Collapsed;
            _forecastReady = false;
            return;
        }

        var reading = snapshot!.Value;
        Temperature.Text = string.Create(CultureInfo.CurrentCulture, $"{Math.Round(reading.TemperatureC):0}°");

        // Today's two ends, from the daily block that arrives with the same reading. Today is the
        // FIRST day Open-Meteo returns, and it is matched by date rather than taken by index: a
        // reading that survives midnight would otherwise put yesterday's range beside today's
        // temperature.
        RenderForecast(reading.Days);

        var today = DateOnly.FromDateTime(DateTime.Now);
        var range = reading.Days?.FirstOrDefault(d => d.Date == today);
        if (range is { } day)
        {
            Range.Text = string.Create(CultureInfo.CurrentCulture,
                $"{Math.Round(day.MaxC):0}° / {Math.Round(day.MinC):0}°");
            Range.Visibility = Visibility.Visible;
        }
        else
        {
            Range.Visibility = Visibility.Collapsed;
        }

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

    /// <summary>How many days the band has room for. Two chips fit beside each other in 320 DIP
    /// with room to spare; three start to crowd the page's left edge.</summary>
    private const int ForecastDays = 2;

    /// <summary>Whether the band has chips to show, so <see cref="RenderNowPlaying"/> can decide
    /// between the two things that share it without rebuilding either.</summary>
    private bool _forecastReady;

    /// <summary>
    /// The next two days, as chips for the band along the bottom.
    ///
    /// Horizontal, unlike the weather page's vertical columns, because this band is 20 DIP tall
    /// and 320 wide: the same three facts turned on their side. The day-selection rule is shared
    /// (<see cref="WeatherDays.Next"/>); the shape is not, and should not be.
    ///
    /// Visibility is NOT decided here. Two things share this row and only one is ever up, so one
    /// method decides, and it is the one that runs last.
    /// </summary>
    private void RenderForecast(IReadOnlyList<WeatherDay>? days)
    {
        Forecast.Children.Clear();

        var next = WeatherDays.Next(days, ForecastDays, DateOnly.FromDateTime(DateTime.Now));
        _forecastReady = next is not null;
        if (next is null)
        {
            Forecast.Visibility = Visibility.Collapsed;
            return;
        }

        foreach (var day in next) Forecast.Children.Add(BuildChip(day));
    }

    /// <summary>
    /// One day: its name, its mark, and its two ends, on one line.
    ///
    /// The mark is the same <see cref="WeatherMark"/> the clock's own reading and the weather
    /// page both use, so one code cannot be drawn three ways. Its ink is bound to the palette
    /// rather than set, because the theme can change under a page that is already built.
    ///
    /// The whole chip is announced on the day name, which is the only element here WPF gives an
    /// automation peer: a mark says nothing out loud and a StackPanel cannot carry a name.
    /// </summary>
    private static FrameworkElement BuildChip(WeatherDay day)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Margin = new Thickness(0, 0, 18, 0),
        };

        var name = new TextBlock
        {
            Text = CultureInfo.CurrentCulture.DateTimeFormat.GetAbbreviatedDayName(day.Date.DayOfWeek),
            FontSize = 12,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
        };
        name.SetResourceReference(TextBlock.ForegroundProperty, "NotchInk");

        System.Windows.Automation.AutomationProperties.SetName(name, string.Create(
            CultureInfo.CurrentCulture,
            $"{CultureInfo.CurrentCulture.DateTimeFormat.GetDayName(day.Date.DayOfWeek)}, " +
            $"{WeatherCodeMap.Describe(day.WeatherCode)}, " +
            $"{Math.Round(day.MaxC):0}° / {Math.Round(day.MinC):0}°"));
        row.Children.Add(name);

        var mark = new WeatherMark
        {
            Width = 18,
            Height = 18,
            Margin = new Thickness(7, 0, 5, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        mark.SetResourceReference(WeatherMark.InkProperty, "NotchInk");
        // Noon, deliberately: a forecast for tomorrow has no hour, and asking for the current one
        // would draw tomorrow's clear sky as a night sky when looked at in the evening.
        mark.Show(SkyCondition.From(day.WeatherCode, 12));
        row.Children.Add(mark);

        var ends = new TextBlock
        {
            Text = string.Create(CultureInfo.CurrentCulture,
                $"{Math.Round(day.MaxC):0}° / {Math.Round(day.MinC):0}°"),
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };
        ends.SetResourceReference(TextBlock.ForegroundProperty, "NotchInkMuted");
        row.Children.Add(ends);

        return row;
    }

    /// <summary>
    /// The track line, present only while something is playing.
    ///
    /// Collapsed rather than blank, and the rule above it collapses with it: a divider over
    /// nothing says "more below" and then does not deliver it.
    /// </summary>
    private void RenderNowPlaying()
    {
        // Named for what it is. It was called "playing", which is what led the announcement above
        // to say "playing" for a paused session: a local whose name is not true is a comment that
        // lies, and this one was read as though it were.
        var hasTrack = _media is { HasSession: true } && !string.IsNullOrWhiteSpace(_media.Title);

        // No divider any more: the track line sits on the page's bottom edge with the readings
        // at the top, and space between two blocks says "separate" without a line drawn to say it.
        //
        // BOTH occupants of the band are decided here, and here only. The track wins when there
        // is one, because it is the more immediate fact and it is the reason this row exists; the
        // forecast takes the row when there is not, which is what keeps the page from being a
        // clock with a third of a frame under it. Deciding this in two methods is how a page ends
        // up drawing both at once or neither.
        NowPlaying.Visibility = hasTrack ? Visibility.Visible : Visibility.Collapsed;
        Forecast.Visibility = !hasTrack && _forecastReady ? Visibility.Visible : Visibility.Collapsed;
        if (!hasTrack) return;

        NowTitle.Text = string.IsNullOrWhiteSpace(_media!.Artist)
            ? _media.Title
            : $"{_media.Title} — {_media.Artist}";
        NowArt.Source = _media.AlbumArt;
    }
}
