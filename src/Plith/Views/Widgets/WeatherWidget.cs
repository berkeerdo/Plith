using System.Globalization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Shapes;
using Plith.Services;
using Plith.Views.Presentation;

namespace Plith.Views.Widgets;

/// <summary>
/// The weather page: a full-bleed sky with the reading over it.
///
/// Two rules govern everything below, and both are about cost rather than looks.
///
/// <b>Every storyboard stops when the page leaves.</b> In notch mode the OSD's window is never
/// hidden, so an animation left running is a permanent CPU cost on an overlay that is invisible
/// most of the time. Started on the way in, stopped on the way out, with the count logged so the
/// ledger's oldest unmeasured item can finally be measured.
///
/// <b>The first look of the day is an event; every look after it is wallpaper.</b> The trigger is
/// a stored date, not a session flag — see <see cref="FirstLookLedger"/>.
/// </summary>
public partial class WeatherWidget : UserControl
{
    private readonly Func<WeatherSnapshot?> _read;
    private readonly Func<DateOnly?> _readLastReveal;
    private readonly Action<DateOnly> _writeLastReveal;
    private readonly DiagnosticLog? _log;

    private readonly List<Storyboard> _running = new();
    private SkyKind _sky = SkyKind.Overcast;

    public WeatherWidget(
        Func<WeatherSnapshot?> read,
        Func<DateOnly?> readLastReveal,
        Action<DateOnly> writeLastReveal,
        DiagnosticLog? log = null)
    {
        ArgumentNullException.ThrowIfNull(read);
        ArgumentNullException.ThrowIfNull(readLastReveal);
        ArgumentNullException.ThrowIfNull(writeLastReveal);

        InitializeComponent();
        _read = read;
        _readLastReveal = readLastReveal;
        _writeLastReveal = writeLastReveal;
        _log = log;

        // The bleed itself is the layer's negative margin; this transform rests at 1 and is
        // only driven by the day's arrival, which swells uniformly.

        // The scale's centre is the element's own centre, taken from the size it was actually
        // arranged at. Anything else makes the sky grow off-axis: the reveal's swell would drift
        // towards one corner instead of arriving evenly.
        SizeChanged += (_, e) =>
        {
            Bleed.CenterX = e.NewSize.Width / 2;
            Bleed.CenterY = e.NewSize.Height / 2;
        };

        // The reveal fades the sky in, not the page. Fading Root took the readout with it, so
        // the day's first look began with an unreadable temperature.

        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) Enter();
            else StopEverything();
        };
        Unloaded += (_, _) => StopEverything();

        // Content once at construction, animations only on the way in. Same reasoning as the
        // clock: a page whose first paint is bound to IsVisibleChanged is blank until something
        // shows it, which is invisible in the app and total offscreen.
        Render(_read());
    }

    /// <summary>
    /// A new reading arrived. Take it, whether or not this page is on screen.
    ///
    /// This exists because the page could not see one otherwise, and that was a real defect
    /// rather than a refinement: the widget read the snapshot once, when it became visible, and
    /// nothing told it about later ones. The first fetch needs a location lookup and a network
    /// round trip, so the common case was opening the notch, swiping to weather before that
    /// finished, and getting "Weather unavailable" that never went away until the page was left
    /// and re-entered. Only AmbientCard subscribed to WeatherService.Updated, and slice 3 made
    /// AmbientCard unreachable — so in notch mode nothing was listening at all.
    ///
    /// Off screen it repaints and stops. On screen it repaints, and restarts the ambient loops
    /// only if the SKY changed — the same drops falling for a new temperature is not worth a
    /// restart, and restarting would also re-run the arrival for a page the user is already
    /// looking at.
    /// </summary>
    public void OnWeatherUpdated()
    {
        var before = _sky;
        Render(_read());

        if (!IsVisible || _sky == before) return;

        StopEverything();
        StartAmbient(withReveal: false);
    }

    /// <summary>
    /// Come on screen: read the current weather, paint the sky, and start the loops — once, and
    /// only after deciding whether this is the day's first look.
    /// </summary>
    private void Enter()
    {
        // Anything from a previous visit goes first. Entering twice without this would stack a
        // second set of loops on the same elements, which is how an idle cost doubles quietly.
        StopEverything();

        var snapshot = _read();
        Render(snapshot);

        var today = DateOnly.FromDateTime(DateTime.Now);
        var firstLook = FirstLookLedger.IsFirstLook(_readLastReveal(), today);

        // Written before the reveal plays, not in its completion handler. A completion that does
        // not fire — the page turned away mid-reveal, a competing animation replaced the clock —
        // would leave the date unwritten and give a second reveal on the next open. The spec's
        // guarantee is exactly one per day, so the write belongs where the decision is made.
        if (firstLook) _writeLastReveal(today);

        StartAmbient(firstLook);
    }

    private void Render(WeatherSnapshot? snapshot)
    {
        if (snapshot is not WeatherSnapshot w)
        {
            // No reading: no location, no network, or a stale one. Said plainly rather than
            // shown as a blank sky with an empty number, which reads as a broken page — and
            // pointed at the one place it can be fixed, because the most common cause is a
            // system location switch that Plith cannot ask about (it is unpackaged, so Windows
            // never shows a consent prompt) and that nothing else would ever mention.
            _sky = SkyKind.Overcast;
            Temperature.Text = "—";
            Condition.Text = "Weather unavailable";
            Detail.Text = "Check location in Settings, or type a city";
            PaintSky();
            AutomationProperties.SetName(Readout, "Weather unavailable");
            return;
        }

        _sky = SkyCondition.From(w.WeatherCode, DateTime.Now.Hour);
        var label = WeatherCodeMap.Describe(w.WeatherCode);

        Temperature.Text = string.Create(CultureInfo.CurrentCulture, $"{Math.Round(w.TemperatureC):0}°");
        Condition.Text = label;
        Detail.Text = string.Create(CultureInfo.CurrentCulture,
            $"Updated {w.FetchedAt.ToLocalTime():t}");

        PaintSky();

        // On the panel, which has a peer. The whole reading, because a bare "18°" announced on
        // its own says nothing about what it measures.
        AutomationProperties.SetName(Readout, $"{Temperature.Text}, {label}");
    }

    private void PaintSky()
    {
        Sky.Fill = SkyBrush(_sky);
        Bloom.Visibility = _sky == SkyKind.Clear ? Visibility.Visible : Visibility.Collapsed;
        Moon.Visibility = _sky == SkyKind.Night ? Visibility.Visible : Visibility.Collapsed;
    }

    private static LinearGradientBrush SkyBrush(SkyKind kind)
    {
        // 165 degrees rather than straight down: a slight tilt is what keeps a two-stop gradient
        // from reading as a flat band on a surface this wide.
        var brush = new LinearGradientBrush { StartPoint = new Point(0, 0), EndPoint = new Point(0.28, 1) };

        (string top, string mid, string bottom) = kind switch
        {
            SkyKind.Clear => ("#FF2E6F9E", "#FF57A0C9", "#FF86C2DE"),
            SkyKind.Overcast => ("#FF4A5765", "#FF667584", "#FF8794A2"),
            SkyKind.Rain => ("#FF2A3947", "#FF3D5568", "#FF56718A"),
            SkyKind.Snow => ("#FF5A6B7C", "#FF7C8E9F", "#FFAFBECB"),
            _ => ("#FF10192B", "#FF1D2C45", "#FF2C3F5E"),
        };

        brush.GradientStops.Add(new GradientStop(Parse(top), 0));
        brush.GradientStops.Add(new GradientStop(Parse(mid), 0.5));
        brush.GradientStops.Add(new GradientStop(Parse(bottom), 1));
        brush.Freeze();
        return brush;
    }

    private static Color Parse(string hex) => (Color)ColorConverter.ConvertFromString(hex)!;

    /// <summary>
    /// Build and start the loops for the current sky, optionally preceded by the day's reveal.
    /// </summary>
    private void StartAmbient(bool withReveal)
    {
        Clouds.Children.Clear();
        Fall.Children.Clear();

        var delay = withReveal ? RevealDuration : TimeSpan.Zero;
        if (withReveal) PlayReveal();

        if (_sky == SkyKind.Clear) StartBloom(delay);
        if (_sky is SkyKind.Overcast or SkyKind.Clear or SkyKind.Rain) StartClouds(delay);
        if (_sky is SkyKind.Rain or SkyKind.Snow) StartFall(delay);

        _log?.Info("WeatherWidget",
            $"Sky started: kind={_sky}, storyboards={_running.Count}, reveal={withReveal}");
    }

    private static readonly TimeSpan RevealDuration = TimeSpan.FromMilliseconds(1500);

    /// <summary>The day's first look: the sky arrives rather than being already there.</summary>
    private void PlayReveal()
    {
        var ease = new CubicEase { EasingMode = EasingMode.EaseOut };

        var swell = new DoubleAnimation(1.25, 1.0, RevealDuration) { EasingFunction = ease };
        var swellY = new DoubleAnimation(1.25, 1.0, RevealDuration) { EasingFunction = ease };
        var fade = new DoubleAnimation(0, 1, RevealDuration) { EasingFunction = ease };

        Run(Board((Bleed, ScaleTransform.ScaleXProperty, swell),
                  (Bleed, ScaleTransform.ScaleYProperty, swellY)));
        Run(Board((SkyLayer, OpacityProperty, fade)));
    }

    private void StartBloom(TimeSpan delay)
    {
        // Slow, small, and never a full cycle at the edges: a bloom that visibly "beats" reads
        // as a progress indicator rather than as light.
        var pulse = new DoubleAnimation(1.0, 1.12, TimeSpan.FromSeconds(7))
        {
            BeginTime = delay,
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new SineEase { EasingMode = EasingMode.EaseInOut },
        };
        Run(Board((BloomPulse, ScaleTransform.ScaleXProperty, pulse),
                  (BloomPulse, ScaleTransform.ScaleYProperty, pulse.Clone())));
    }

    private void StartClouds(TimeSpan delay)
    {
        var count = _sky == SkyKind.Clear ? 2 : 4;
        var width = NotchGeometry.OpenFrameDip.Width;

        for (int i = 0; i < count; i++)
        {
            var puff = new Ellipse
            {
                Width = 90 + i * 26,
                Height = 26 + i * 4,
                Fill = new SolidColorBrush(Color.FromArgb((byte)(_sky == SkyKind.Clear ? 44 : 66), 255, 255, 255)),
                RenderTransform = new TranslateTransform(),
            };
            Canvas.SetTop(puff, 6 + i * 17);
            Clouds.Children.Add(puff);

            // Staggered by both duration and start, so the set never lines up into one moving
            // band. Derived from the index rather than randomised: this runs in a workflow where
            // randomness would make the same page look different on every open for no reason.
            var travel = new DoubleAnimation(-puff.Width, width + puff.Width,
                TimeSpan.FromSeconds(34 + i * 11))
            {
                BeginTime = delay + TimeSpan.FromSeconds(i * 4),
                RepeatBehavior = RepeatBehavior.Forever,
            };
            Run(Board(((TranslateTransform)puff.RenderTransform, TranslateTransform.XProperty, travel)));
        }
    }

    private void StartFall(TimeSpan delay)
    {
        var snow = _sky == SkyKind.Snow;
        var count = snow ? 18 : 26;
        var width = NotchGeometry.OpenFrameDip.Width;
        var height = NotchGeometry.OpenFrameDip.Height;

        for (int i = 0; i < count; i++)
        {
            Shape drop = snow
                ? new Ellipse { Width = 3, Height = 3, Fill = Brushes.White, Opacity = 0.75 }
                : new Rectangle
                {
                    Width = 1.5,
                    Height = 13,
                    RadiusX = 1,
                    RadiusY = 1,
                    Fill = new SolidColorBrush(Color.FromArgb(0xBF, 0xC8, 0xE0, 0xFF)),
                };

            drop.RenderTransform = new TranslateTransform();

            // Spread across the width by index, so the set covers the page evenly instead of
            // clustering the way independent random draws do at this count.
            Canvas.SetLeft(drop, (i * width / count) + (i % 3) * 4);
            Canvas.SetTop(drop, -16);
            Fall.Children.Add(drop);

            var fall = new DoubleAnimation(0, height + 26,
                TimeSpan.FromSeconds(snow ? 5.5 + i % 4 : 1.1 + (i % 5) * 0.12))
            {
                BeginTime = delay + TimeSpan.FromMilliseconds(i * (snow ? 260 : 90)),
                RepeatBehavior = RepeatBehavior.Forever,
            };
            Run(Board(((TranslateTransform)drop.RenderTransform, TranslateTransform.YProperty, fall)));
        }
    }

    private static Storyboard Board(params (DependencyObject Target, DependencyProperty Property, AnimationTimeline Animation)[] parts)
    {
        var board = new Storyboard();
        foreach (var (target, property, animation) in parts)
        {
            Storyboard.SetTarget(animation, target);
            Storyboard.SetTargetProperty(animation, new PropertyPath(property));
            board.Children.Add(animation);
        }
        return board;
    }

    private void Run(Storyboard board)
    {
        _running.Add(board);
        board.Begin();
    }

    /// <summary>
    /// Stop and forget every loop.
    ///
    /// Storyboards are tracked in a list rather than being stopped through their targets. A
    /// target-based stop would need every element to still be in the tree, and the cloud and
    /// drop elements are cleared on the next entry — so the ones from the previous visit would
    /// keep their clocks alive with nothing left pointing at them.
    /// </summary>
    private void StopEverything()
    {
        if (_running.Count == 0) return;

        foreach (var board in _running) board.Stop();
        _log?.Info("WeatherWidget", $"Sky stopped: storyboards={_running.Count}");
        _running.Clear();

        // Cleared as well as stopped: a stopped storyboard leaves the property at its animated
        // value under FillBehavior.HoldEnd, and the reveal's own fade would leave Root parked at
        // whatever opacity it had reached when the page turned away mid-reveal.
        SkyLayer.BeginAnimation(OpacityProperty, null);
        SkyLayer.Opacity = 1;
    }
}
