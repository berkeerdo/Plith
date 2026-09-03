using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class AudioCardTests
{
    private static SettingsService NewSettings()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        return new SettingsService(path);
    }

    private static (AudioCard card, List<ShowRequest> shows) NewCard()
    {
        var card = new AudioCard(NewSettings());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);
        return (card, shows);
    }

    [Fact]
    public void FirstApply_IsSilentBaseline()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void SecondApplyWithDifferentValue_RaisesShowRequested()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.6, "+2.0 dB", muted: false);

        Assert.Single(shows);
        Assert.Equal(ShowReason.AudioChange, shows[0].Reason);
        Assert.Equal("audio", shows[0].OriginCardId);
    }

    [Fact]
    public void ApplyWithUnchangedValue_RaisesNothing()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void ApplyWithinEpsilon_RaisesNothing()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        // 0.0005 is the orchestrator's original threshold; 0.0001 must stay below it.
        card.Apply("Bus A1", 0.5001, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void MuteFlipAtUnchangedGain_RaisesShowRequested()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: true);
        Assert.Single(shows);
    }

    [Fact]
    public void ResetBaseline_MakesTheNextApplySilentAgain()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.ResetBaseline();
        card.Apply("Bus A1", 0.9, "+10.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void Apply_AlwaysUpdatesTheViewModelEvenWhenSilent()
    {
        var (card, _) = NewCard();
        card.Apply("Speakers (G733)", 0.75, "75%", muted: false);

        Assert.Equal("Speakers (G733)", card.Vm.Label);
        Assert.Equal(0.75, card.Vm.GainNormalized);
        Assert.Equal("75%", card.Vm.GainText);
    }

    [Fact]
    public void IsVisible_IsAlwaysTrue()
    {
        var (card, _) = NewCard();
        Assert.True(card.IsVisible);
    }

    // ToString is what WPF's ItemAutomationPeer announces for this card's row in the OSD stack.
    // It read "Plith.Cards.AudioCard" in the live UIA tree until AccessibleName existed, so this
    // guards against a future "unused override" cleanup silently restoring that.
    [Fact]
    public void AccessibleName_IsHumanReadable_AndDrivesToString()
    {
        var (card, _) = NewCard();
        Assert.Equal("Volume", card.AccessibleName);
        Assert.Equal(card.AccessibleName, card.ToString());
        Assert.DoesNotContain("Plith.Cards", card.ToString());
    }
}
