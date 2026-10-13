using Plith.ViewModels;
using Plith.Views;
using Plith.Views.Widgets;

namespace Plith.Tests;

/// <summary>
/// The notch HUD's bars settle like the card's, except that the first value after the HUD comes
/// on screen LANDS. Tests have no render tick, so HasAnimatedProperties stands in for "a clock is
/// running", as SettleTests does.
/// </summary>
public class NotchHudSettleTests
{
    private static (NotchHud Hud, AudioCardViewModel Audio, SettledBorder Fill) Make(bool onScreen = true)
    {
        var audio = new AudioCardViewModel();
        var hud = new NotchHud(audio, new MediaViewModel())
        {
            IsOnScreen = () => onScreen,
        };
        var fill = (SettledBorder)hud.FindName("LevelFill");
        fill.MotionAllowed = true;
        return (hud, audio, fill);
    }

    [StaFact]
    public void FirstWriteAfterAShow_LandsInsteadOfSweeping()
    {
        var (hud, audio, fill) = Make();
        audio.GainNormalized = 0.2;
        hud.Show(NotchHudKind.Volume, freshlyShown: true);
        Assert.False(fill.HasAnimatedProperties);

        // The HUD is torn down and comes back at a different level: that must land, not sweep.
        audio.GainNormalized = 0.8;
        hud.Show(NotchHudKind.Volume, freshlyShown: true);

        Assert.False(fill.HasAnimatedProperties);
        Assert.Null(fill.LastAnimation);
        Assert.Equal(0.8, fill.Displayed);
    }

    [StaFact]
    public void LaterWriteInAVisibleHud_Settles()
    {
        var (hud, audio, fill) = Make();
        audio.GainNormalized = 0.2;
        hud.Show(NotchHudKind.Volume, freshlyShown: true);

        audio.GainNormalized = 0.8;

        Assert.True(fill.HasAnimatedProperties);
        Assert.NotNull(fill.LastAnimation);
        Assert.Equal(0.8, fill.Target);
    }

    [StaFact]
    public void WriteWhileNotOnScreen_Lands()
    {
        var (hud, audio, fill) = Make(onScreen: false);
        hud.Show(NotchHudKind.Volume, freshlyShown: true);

        audio.GainNormalized = 0.7;

        Assert.False(fill.HasAnimatedProperties);
        Assert.Equal(0.7, fill.Displayed);
    }

    [StaFact]
    public void Jump_AfterASettle_DropsTheClockAndLands()
    {
        var (hud, audio, fill) = Make();
        hud.Show(NotchHudKind.Volume, freshlyShown: true);
        audio.GainNormalized = 0.9;
        Assert.True(fill.HasAnimatedProperties);

        fill.Jump(0.3);

        Assert.False(fill.HasAnimatedProperties);
        Assert.Equal(0.3, fill.Displayed);
    }
}
