using System.Globalization;
using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class AmbientCardTests
{
    private static AmbientCard Build(NotchHomeState home)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        return new(home, new SettingsService(path));
    }

    [Fact]
    public void IsInvisibleWhileTheHomeViewIsClosed()
    {
        Assert.False(Build(new NotchHomeState()).IsVisible);
    }

    [Fact]
    public void BecomesVisibleWhenTheHomeViewOpens()
    {
        var home = new NotchHomeState();
        var card = Build(home);

        home.Open();

        Assert.True(card.IsVisible);
    }

    [Fact]
    public void RaisesVisibilityChangedOnEachTransition()
    {
        // CardHost subscribes to this to reconcile VisibleCards. A card whose IsVisible flips
        // without the event never appears, and the failure looks like the home state not
        // working rather than like a missing event.
        var home = new NotchHomeState();
        var card = Build(home);
        int raised = 0;
        card.VisibilityChanged += () => raised++;

        home.Open();
        home.Close();

        Assert.Equal(2, raised);
    }

    [Fact]
    public void TickFormatsTheClockIntoTheViewModel()
    {
        var card = Build(new NotchHomeState());

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), new CultureInfo("tr-TR"));

        Assert.Equal("14:05", card.Vm.ClockTime);
    }

    [Fact]
    public void RendersAboveTheMediaAndAudioCards()
    {
        // Order is the ambient row's whole claim to the top of the panel. MediaCard is 10 and
        // AudioCard is 20; a regression here puts the clock under the volume bar.
        Assert.True(Build(new NotchHomeState()).Order < 10);
    }

    [Fact]
    public void ToStringReturnsTheAccessibleName()
    {
        // Load-bearing: WPF's ItemAutomationPeer names the OSD's list container from the bound
        // item's ToString(). Without this the row announces "Plith.Cards.AmbientCard".
        var card = Build(new NotchHomeState());
        Assert.Equal(card.AccessibleName, card.ToString());
    }
}
