using System.Windows;
using System.Windows.Media.Animation;
using Plith.Views;

namespace Plith.Tests;

public class SettleTests
{
    private static ResourceDictionary LoadTheme() =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri("/Plith;component/Resources/Theme.xaml", UriKind.Relative));

    [StaFact]
    public void Settle_NeverOvershoots()
    {
        var theme = LoadTheme();
        var ease = theme["LoftSettleEase"];

        // Bounce zero is the rule. Any easing that returns above 1.0 before the end overshoots,
        // which is exactly the batten springing past its duck.
        Assert.IsNotType<BounceEase>(ease);
        Assert.IsNotType<ElasticEase>(ease);
        Assert.IsNotType<BackEase>(ease);

        var function = Assert.IsAssignableFrom<IEasingFunction>(ease);
        for (var t = 0.0; t <= 1.0; t += 0.01)
        {
            Assert.InRange(function.Ease(t), -0.0001, 1.0001);
        }
    }

    [StaFact]
    public void Settle_UsesTheCalmDuration()
    {
        var theme = LoadTheme();
        var duration = Assert.IsType<Duration>(theme["LoftSettleDuration"]);

        Assert.Equal(TimeSpan.FromMilliseconds(400), duration.TimeSpan);
    }

    private static SettledBorder Bar(bool motion)
    {
        var theme = LoadTheme();
        return new SettledBorder
        {
            MotionAllowed = motion,
            SettleDuration = (Duration)theme["LoftSettleDuration"],
            SettleEase = (IEasingFunction)theme["LoftSettleEase"],
        };
    }

    // HasAnimatedProperties is true as soon as a clock is attached; GetValueSource().IsAnimated
    // only flips after the first render tick, which a unit test never gets.
    private static bool IsAnimated(SettledBorder bar) => bar.HasAnimatedProperties;

    [StaFact]
    public void FirstValue_JumpsWithoutAnimation()
    {
        var bar = Bar(motion: true);
        bar.Target = 0.6;

        Assert.Equal(0.6, bar.Displayed);
        Assert.False(IsAnimated(bar));
    }

    [StaFact]
    public void ReducedMotion_JumpsAndAllocatesNoAnimation()
    {
        var bar = Bar(motion: false);
        bar.Target = 0.2;
        bar.Target = 0.8;

        Assert.Equal(0.8, bar.Displayed);
        Assert.False(IsAnimated(bar));
    }

    [StaFact]
    public void Change_SettlesFromTheShownValueNotTheTarget()
    {
        var bar = Bar(motion: true);
        bar.Target = 0.2;
        bar.Target = 0.8;

        Assert.True(IsAnimated(bar));
        // Starts from the 0.2 on screen, ends on the target.
        var animation = Assert.IsType<DoubleAnimation>(bar.LastAnimation);
        Assert.Equal(0.2, animation.From);
        Assert.Equal(0.8, animation.To);
    }

    [StaFact]
    public void Retarget_HandsOffFromTheCurrentValueAndKeepsOneClock()
    {
        var bar = Bar(motion: true);
        bar.Target = 0.2;
        bar.Target = 0.8;
        bar.Target = 0.5;

        // A second animation replaced the first on the same property. It starts from where the
        // bar is shown (the clock has not ticked, so still 0.2, not the previous target 0.8).
        Assert.True(IsAnimated(bar));
        var animation = Assert.IsType<DoubleAnimation>(bar.LastAnimation);
        Assert.Equal(0.2, animation.From);
        Assert.Equal(0.5, animation.To);
    }

    [StaFact]
    public void Animation_ReleasesItsClockAtTheEnd()
    {
        var bar = Bar(motion: true);
        bar.Target = 0.2;
        bar.Target = 0.8;

        var animation = Assert.IsType<DoubleAnimation>(bar.LastAnimation);
        // Stop, not HoldEnd: a held animation keeps its clock alive for ever.
        Assert.Equal(FillBehavior.Stop, animation.FillBehavior);
        Assert.Equal(0.8, animation.To);
        Assert.Equal(1, animation.RepeatBehavior.Count);
    }

    [StaFact]
    public void SameValue_StartsNothing()
    {
        var bar = Bar(motion: true);
        bar.Target = 0.5;
        bar.Target = 0.5;

        Assert.False(IsAnimated(bar));
    }
}
