using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using Plith.Services;

namespace Plith.Views.Widgets;

/// <summary>
/// The weather as a shape, with a little life in it.
///
/// It replaces a word. "Overcast" under a temperature is the slowest possible way to say
/// something a silhouette says before it is read, and on a surface glanced at for two seconds
/// that difference is the whole of it.
///
/// The motion is deliberately small — a sun that breathes, a cloud that drifts a pixel and back,
/// a drop that falls. Anything larger on a 22 DIP mark stops being weather and becomes a
/// spinner, which is what the sky's own bloom is already careful not to be.
/// </summary>
public partial class WeatherMark : UserControl
{
    private readonly List<(DependencyObject Target, DependencyProperty Property)> _running = new();

    public WeatherMark()
    {
        InitializeComponent();

        // Stops with visibility, like every other moving thing on this branch: the OSD's window
        // is never hidden in notch mode, so an animation left running is a permanent cost on a
        // surface that is invisible most of the time.
        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) StartMotion();
            else StopMotion();
        };
        Unloaded += (_, _) => StopMotion();
    }

    public static readonly DependencyProperty InkProperty =
        DependencyProperty.Register(nameof(Ink), typeof(Brush), typeof(WeatherMark),
            new PropertyMetadata(Brushes.White, (d, e) => ((WeatherMark)d).ApplyInk((Brush)e.NewValue)));

    /// <summary>The colour every part is drawn in. A real property rather than inherited
    /// Foreground, for the reason MarqueeText learned the hard way: a control that builds its own
    /// visual children is where that inheritance quietly does not arrive.</summary>
    public Brush Ink
    {
        get => (Brush)GetValue(InkProperty);
        set => SetValue(InkProperty, value);
    }

    private void ApplyInk(Brush ink)
    {
        Sun.Fill = ink;
        Moon.Fill = ink;
        Cloud.Fill = ink;
        // Rain is strokes and snow is a stroked flake, so neither wants a fill: filled, the
        // three rain strokes close into three wedges.
        Fall.Fill = null;
        Rays.Stroke = ink;
        Fall.Stroke = ink;
    }

    private SkyKind _kind = SkyKind.Clear;

    /// <summary>
    /// Show the mark for a sky.
    ///
    /// Takes the same <see cref="SkyKind"/> the page's gradient is drawn from rather than a
    /// weather code of its own — the mark and the sky behind it must never describe different
    /// weather, and the surest way to guarantee that is for them to read one value.
    /// </summary>
    public void Show(SkyKind kind)
    {
        _kind = kind;

        var clear = kind == SkyKind.Clear;
        var night = kind == SkyKind.Night;
        var rain = kind == SkyKind.Rain;
        var snow = kind == SkyKind.Snow;

        Sun.Visibility = clear ? Visibility.Visible : Visibility.Collapsed;
        Rays.Visibility = clear ? Visibility.Visible : Visibility.Collapsed;
        Moon.Visibility = night ? Visibility.Visible : Visibility.Collapsed;

        // Cloud under everything wet, and under overcast on its own.
        Cloud.Visibility = clear || night ? Visibility.Collapsed : Visibility.Visible;

        Fall.Visibility = rain || snow ? Visibility.Visible : Visibility.Collapsed;
        if (rain || snow)
        {
            // Three strokes for rain, a flake for snow. Both are line art at this size, which
            // is what makes them readable in a 26 DIP column: see IconWxDrops.
            Fall.Data = (Geometry)FindResource(snow ? "IconWxFlake" : "IconWxDrops");

            // The drop hangs below the cloud rather than sitting in the middle of it, and is
            // small enough to read as falling FROM it.
            //
            // THE NUMBERS NOW DO WHAT THAT SENTENCE SAYS. They were Left 3, Top 7, 18 by 18,
            // against a cloud that occupies y=10 to 18.4 of the 24 box: the drop started three
            // units ABOVE the cloud's top and covered nearly all of it. On the big sky page the
            // fall animation pulls it clear and the eye follows the motion, so it read correctly
            // there and nowhere else. In a 26 DIP forecast column, static, it was one white blob
            // and the page could not answer "what will Tuesday be". Reported from a real session.
            //
            // 9 by 9 at Top 15 leaves the drop hanging from the cloud's lower edge with three
            // units of overlap, and the fall animation travels from -2 to +6, which keeps it
            // inside the 24 box at both ends.
            Fall.RenderTransform = FallDrop;
            if (snow)
            {
                Canvas.SetLeft(Fall, 7.5);
                Canvas.SetTop(Fall, 15);
                Fall.Width = 9;
                Fall.Height = 9;
            }
            else
            {
                // The strokes span the cloud's own width and start at its lower edge, so they
                // read as coming FROM it rather than as a mark beside it.
                Canvas.SetLeft(Fall, 6);
                Canvas.SetTop(Fall, 15.5);
                Fall.Width = 12;
                Fall.Height = 6.5;
            }

            Fall.Stretch = Stretch.Uniform;
            // Stroked for both now. Rain was a filled teardrop with no stroke at all, which is
            // why it merged into the cloud above it.
            Fall.StrokeThickness = 1.5;
        }

        ApplyInk(Ink);
        if (IsVisible) StartMotion();
    }

    private void StartMotion()
    {
        StopMotion();
        if (!SystemParameters.ClientAreaAnimation) return;

        var ease = new SineEase { EasingMode = EasingMode.EaseInOut };

        if (_kind == SkyKind.Clear)
        {
            // Breathing, not spinning: at this size a turning ray wheel is a loading indicator.
            // The rays do turn, but slowly enough that it reads as light moving rather than a
            // control waiting for something.
            Run(SunPulse, ScaleTransform.ScaleXProperty, Loop(1.0, 1.09, 3.2, ease));
            Run(SunPulse, ScaleTransform.ScaleYProperty, Loop(1.0, 1.09, 3.2, ease));
            Run(RaySpin, RotateTransform.AngleProperty,
                new DoubleAnimation(0, 360, TimeSpan.FromSeconds(48)) { RepeatBehavior = RepeatBehavior.Forever });
        }

        if (Cloud.Visibility == Visibility.Visible)
            Run(CloudDrift, TranslateTransform.XProperty, Loop(-0.8, 0.8, 4.5, ease));

        if (Fall.Visibility == Visibility.Visible)
        {
            // Falls and repeats rather than reversing: rain that rises again on the way back is
            // the one motion a drop must never make.
            Run(FallDrop, TranslateTransform.YProperty,
                new DoubleAnimation(-2, 6, TimeSpan.FromSeconds(_kind == SkyKind.Snow ? 2.6 : 1.4))
                {
                    RepeatBehavior = RepeatBehavior.Forever,
                });
            Run(Fall, OpacityProperty,
                new DoubleAnimation(1, 0, TimeSpan.FromSeconds(_kind == SkyKind.Snow ? 2.6 : 1.4))
                {
                    RepeatBehavior = RepeatBehavior.Forever,
                });
        }
    }

    private static DoubleAnimation Loop(double from, double to, double seconds, IEasingFunction ease) =>
        new(from, to, TimeSpan.FromSeconds(seconds))
        {
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = ease,
        };

    private void Run(DependencyObject target, DependencyProperty property, AnimationTimeline animation)
    {
        switch (target)
        {
            case Animatable a: a.BeginAnimation(property, animation); break;
            case UIElement e: e.BeginAnimation(property, animation); break;
            default: return;
        }
        _running.Add((target, property));
    }

    private void StopMotion()
    {
        foreach (var (target, property) in _running)
        {
            switch (target)
            {
                case Animatable a: a.BeginAnimation(property, null); break;
                case UIElement e: e.BeginAnimation(property, null); break;
            }
        }
        _running.Clear();

        // Cleared as well as stopped: a held animated value outranks a local write, so a mark
        // that came back from a page turn would keep whatever opacity its last fade reached.
        Fall.Opacity = 1;
    }
}
