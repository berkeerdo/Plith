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
        return new(home, new SettingsService(path), null);
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
        card.Activate();   // _home.Changed is only wired up between Activate() and Deactivate()
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

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), new CultureInfo("tr-TR"), null, null);

        Assert.Equal("14:05", card.Vm.ClockTime);
    }

    [Fact]
    public void AccessibleSummaryReadsTheClockAsATime()
    {
        // Bound as AmbientCardView's card-level AutomationProperties.Name — a screen reader
        // must hear the actual clock value, not a static "Time" label with nothing after it.
        var card = Build(new NotchHomeState());

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), new CultureInfo("tr-TR"), null, null);

        Assert.Equal("Time 14:05, 7 Eylül", card.Vm.AccessibleSummary);
    }

    [Fact]
    public void AccessibleSummaryAppendsTheBatterySegmentWhenTheColumnIsShown()
    {
        // The battery StackPanel carries no AutomationProperties.Name of its own — WPF gives
        // panels no automation peer — so the composed summary is the only place a screen
        // reader can hear the battery reading at all.
        var card = Build(new NotchHomeState());

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), new CultureInfo("tr-TR"),
                  new BatteryStatusRaw(BatteryFlag: 0, BatteryLifePercent: 73, ACLineStatus: 1), null);

        Assert.Equal("Time 14:05, 7 Eylül, Battery 73%, charging", card.Vm.AccessibleSummary);
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

    [Fact]
    public void TickCollapsesTheBatteryColumnOnADesktop()
    {
        var card = Build(new NotchHomeState());

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), CultureInfo.InvariantCulture,
                  new BatteryStatusRaw(BatteryFlag: 128, BatteryLifePercent: 255, ACLineStatus: 1), null);

        Assert.False(card.Vm.HasBattery);
    }

    [Fact]
    public void TickCollapsesTheWeatherColumnWhenThereIsNoSnapshot()
    {
        var card = Build(new NotchHomeState());
        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), CultureInfo.InvariantCulture, null, null);
        Assert.False(card.Vm.HasWeather);
    }

    [Fact]
    public void TickShowsAFreshSnapshot()
    {
        var card = Build(new NotchHomeState());
        var snap = new WeatherSnapshot(21.4, 0, DateTimeOffset.UtcNow);

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), CultureInfo.InvariantCulture, null, snap);

        Assert.True(card.Vm.HasWeather);
        Assert.Contains("21", card.Vm.WeatherText);
    }

    [Fact]
    public void TickCollapsesTheWeatherColumnForAStaleSnapshot()
    {
        var card = Build(new NotchHomeState());
        var snap = new WeatherSnapshot(21.4, 0, DateTimeOffset.UtcNow.AddHours(-4));

        card.Tick(new DateTime(2026, 9, 7, 14, 5, 0), CultureInfo.InvariantCulture, null, snap);

        Assert.False(card.Vm.HasWeather);
    }
}
