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
        Fall.Fill = ink;
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
            Fall.Data = (Geometry)FindResource(snow ? "IconWxFlake" : "IconWxDrop");

            // The drop hangs below the cloud rather than sitting in the middle of it, and is
            // small enough to read as falling FROM it.
            Fall.RenderTransform = FallDrop;
            Canvas.SetLeft(Fall, 3);
            Canvas.SetTop(Fall, 7);
            Fall.Width = 18;
            Fall.Height = 18;
            Fall.Stretch = Stretch.Uniform;
            Fall.StrokeThickness = snow ? 1.4 : 0;
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
