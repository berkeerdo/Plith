using Plith.Services;

namespace Plith.Tests;

public class AudioLabelTests
{
    [Fact]
    public void ANameWithNoAdapterSuffixIsUnchanged()
    {
        Assert.Equal("Logitech G733", AudioLabel.Shorten("Logitech G733"));
    }

    [Fact]
    public void TheAdapterSuffixIsDroppedWhenTheHeadAlreadyNamesIt()
    {
        // The Sonar case: Chat, Gaming and Media all bundle the driver name into the
        // parenthesis, so keeping it would say the same thing twice.
        Assert.Equal(
            "SteelSeries Sonar - Chat",
            AudioLabel.Shorten("SteelSeries Sonar - Chat (SteelSeries Sonar Virtual Audio Device)"));
    }

    [Fact]
    public void AnUnrelatedAdapterIsKeptWhole()
    {
        // Two endpoints can share a name on different drivers. Dropping the adapter entirely
        // would make them identical on screen, which is worse than a long line. The original
        // doc comment claimed this became "Hoparlör (Realtek)"; it never did, and it should not
        // - the adapter is two words, so the two-word hint IS the adapter.
        Assert.Equal("Hoparlör (Realtek(R) Audio)", AudioLabel.Shorten("Hoparlör (Realtek(R) Audio)"));
    }

    [Fact]
    public void ALongAdapterIsCutToTwoWords()
    {
        Assert.Equal(
            "Headphones (NVIDIA High)",
            AudioLabel.Shorten("Headphones (NVIDIA High Definition Audio)"));
    }

    [Fact]
    public void ASingleWordAdapterSurvivesWhole()
    {
        Assert.Equal("Speakers (Realtek)", AudioLabel.Shorten("Speakers (Realtek)"));
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void AnEmptyNameFallsBackRatherThanReturningNothing(string? name)
    {
        // Reachable: a device can report an empty name, and a failed read falls back to
        // whatever the caller had. A blank device line gives no clue anything is attached.
        Assert.Equal("Unknown device", AudioLabel.Shorten(name));
    }

    [Fact]
    public void SurroundingWhitespaceIsTrimmed()
    {
        Assert.Equal("Logitech G733", AudioLabel.Shorten("  Logitech G733  "));
    }

    [Fact]
    public void AnUnclosedParenthesisIsNotTreatedAsASuffix()
    {
        // "Headset (2" is a name, not a name plus an adapter. Slicing on " (" without checking
        // for the closing bracket would cut it in half.
        Assert.Equal("Headset (2", AudioLabel.Shorten("Headset (2"));
    }

    [Fact]
    public void ANameThatIsOnlyAParenthesisIsLeftAlone()
    {
        Assert.Equal("(Realtek)", AudioLabel.Shorten("(Realtek)"));
    }

    [Theory]
    [InlineData(0, "Voicemeeter · A1")]
    [InlineData(4, "Voicemeeter · A5")]
    [InlineData(5, "Voicemeeter · B1")]
    [InlineData(7, "Voicemeeter · B3")]
    public void VoicemeeterBusesUseTheirOwnNames(int index, string expected)
    {
        Assert.Equal(expected, AudioLabel.BusLine(AudioRail.VoicemeeterBus, index, muted: false));
    }

    [Fact]
    public void AnOutOfRangeBusFallsBackToItsNumber()
    {
        // The index comes from a settings file a person can edit by hand, so it must not throw.
        Assert.Equal("Voicemeeter · Bus 12", AudioLabel.BusLine(AudioRail.VoicemeeterBus, 12, muted: false));
    }

    [Fact]
    public void StripsAreCountedFromOne()
    {
        Assert.Equal("Voicemeeter · Strip 1", AudioLabel.BusLine(AudioRail.VoicemeeterStrip, 0, muted: false));
    }

    [Fact]
    public void AWindowsEndpointSaysSo()
    {
        Assert.Equal("Windows · Output", AudioLabel.BusLine(AudioRail.WindowsEndpoint, 0, muted: false));
    }

    [Theory]
    [InlineData(AudioRail.VoicemeeterBus)]
    [InlineData(AudioRail.VoicemeeterStrip)]
    [InlineData(AudioRail.WindowsEndpoint)]
    public void MutedWinsOverTheRail(AudioRail rail)
    {
        // When the sound is off, which bus it is off on is the second question.
        Assert.Equal("Muted", AudioLabel.BusLine(rail, 0, muted: true));
    }
}
