using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace Plith.Views;

/// <summary>
/// A border whose fill fraction settles on a new target instead of snapping to it. The volume
/// and brightness bars bind their width to <see cref="Displayed"/>, and the view model's value to
/// <see cref="Target"/>.
///
/// Three rules, each for a defect this app has had in some other form:
/// <list type="bullet">
/// <item>Reduced motion JUMPS. No animation object is created at all, because even a zero
/// duration one allocates a clock and raises completion callbacks.</item>
/// <item>A new target hands off from the value currently on screen, replacing the running
/// animation. A held volume key therefore never queues, restarts from zero or runs backwards.</item>
/// <item>The animation uses <see cref="FillBehavior.Stop"/> with the final value already set as
/// the base value, so the clock is released the moment it completes and nothing keeps the UI
/// thread awake at rest.</item>
/// </list>
/// The first value jumps too: a card that opens already showing 40 % should not sweep up from 0.
/// </summary>
public class SettledBorder : Border
{
    public static readonly DependencyProperty TargetProperty = DependencyProperty.Register(
        nameof(Target), typeof(double), typeof(SettledBorder),
        new PropertyMetadata(0.0, OnTargetChanged));

    // Deliberately NOT RegisterReadOnly: WPF refuses to animate a read-only property
    // (IsAnimationProhibited), so the control could not drive it. The CLR wrapper is get-only and
    // the control owns every write; bind to it one way.
    public static readonly DependencyProperty DisplayedProperty = DependencyProperty.Register(
        nameof(Displayed), typeof(double), typeof(SettledBorder), new PropertyMetadata(0.0));

    public static readonly DependencyProperty SettleDurationProperty = DependencyProperty.Register(
        nameof(SettleDuration), typeof(Duration), typeof(SettledBorder),
        new PropertyMetadata(new Duration(TimeSpan.FromMilliseconds(400))));

    public static readonly DependencyProperty SettleEaseProperty = DependencyProperty.Register(
        nameof(SettleEase), typeof(IEasingFunction), typeof(SettledBorder),
        new PropertyMetadata(null));

    private bool _hasValue;
    private AnimationClock? _clock;
    private double _from;
    private double _to;

    /// <summary>The value the bar is settling toward, 0 to 1.</summary>
    public double Target
    {
        get => (double)GetValue(TargetProperty);
        set => SetValue(TargetProperty, value);
    }

    /// <summary>The value on screen right now, 0 to 1. Animated; bind widths to this.</summary>
    public double Displayed => (double)GetValue(DisplayedProperty);

    public Duration SettleDuration
    {
        get => (Duration)GetValue(SettleDurationProperty);
        set => SetValue(SettleDurationProperty, value);
    }

    public IEasingFunction? SettleEase
    {
        get => (IEasingFunction?)GetValue(SettleEaseProperty);
        set => SetValue(SettleEaseProperty, value);
    }

    /// <summary>
    /// Overrides the system setting. Null reads <see cref="SystemParameters.ClientAreaAnimation"/>
    /// on every change, so toggling reduced motion takes effect without a restart.
    /// </summary>
    public bool? MotionAllowed { get; set; }

    /// <summary>The most recent animation started, kept so a test can inspect its shape.</summary>
    public DoubleAnimation? LastAnimation { get; private set; }

    private static void OnTargetChanged(DependencyObject d, DependencyPropertyChangedEventArgs e) =>
        ((SettledBorder)d).Settle((double)e.NewValue);

    private void Settle(double target)
    {
        var first = !_hasValue;
        _hasValue = true;
        var allowed = MotionAllowed ?? SystemParameters.ClientAreaAnimation;

        if (first || !allowed)
        {
            // Drop any running clock, then write the value. No animation object is created.
            Land(target);
            return;
        }

        var current = ShownValue();
        if (current == target)
        {
            // Not a no-op: a clock may still be carrying the bar toward an EARLIER target (0.2,
            // then 0.8, then 0.2 inside one frame). Returning here would leave it running and
            // the bar would settle on 0.8 while the model says 0.2.
            Land(target);
            return;
        }

        // Base value first, so the bar lands on the target the instant the clock is released.
        SetValue(DisplayedProperty, target);
        var animation = new DoubleAnimation(current, target, SettleDuration)
        {
            EasingFunction = SettleEase,
            FillBehavior = FillBehavior.Stop,
        };
        LastAnimation = animation;
        _from = current;
        _to = target;
        // Removes the running clock from the time tree before replacing it. Applying a new clock
        // alone only changes what the property reads; the old root clock would keep ticking to
        // its own end, so a held key would pile up orphans.
        DropClock();
        var clock = animation.CreateClock();
        // Forget the clock once it ends, so a finished animation is never asked for a value.
        clock.Completed += (sender, _) =>
        {
            if (!ReferenceEquals(sender, _clock)) return;
            // Remove, not just forget: a finished root clock stays in the time tree until it is
            // removed, and once _clock is null no later DropClock() can reach it.
            _clock.Controller?.Remove();
            _clock = null;
        };
        _clock = clock;
        ApplyAnimationClock(DisplayedProperty, clock);
    }

    /// <summary>
    /// Puts the bar on <paramref name="target"/> now, whatever it was showing. For a bar that
    /// has just come on screen: its last value belongs to an earlier appearance, and sweeping
    /// from it would show a bar growing from wherever it was left. Later targets settle as usual.
    /// </summary>
    public void Jump(double target)
    {
        _hasValue = true;
        Land(target);
        // Settle runs if the target changed, finds the bar already on it, and lands again.
        SetValue(TargetProperty, target);
    }

    /// <summary>Stops any animation and writes the value directly. Creates no animation object.</summary>
    private void Land(double target)
    {
        DropClock();
        BeginAnimation(DisplayedProperty, null);
        SetValue(DisplayedProperty, target);
        LastAnimation = null;
    }

    private void DropClock()
    {
        _clock?.Controller?.Remove();
        _clock = null;
    }

    /// <summary>
    /// Where the bar is on screen right now. A running clock is asked directly, because the
    /// property itself reports the base value until the next render tick, and a second key press
    /// inside one frame would otherwise hand off from the previous target instead of from the
    /// bar.
    /// </summary>
    private double ShownValue()
    {
        if (_clock is null) return Displayed;

        // Applied but not yet ticked: the bar has not moved from where it started. A null
        // progress is also what a Stopped clock reports, but Completed clears _clock in the same
        // tick the clock stops, so a stopped clock is never seen here.
        if (_clock.CurrentProgress is null) return _from;
        return (double)_clock.GetCurrentValue(_from, _to);
    }
}
