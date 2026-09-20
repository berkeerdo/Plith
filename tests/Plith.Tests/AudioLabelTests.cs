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

    // --- ShortenAll: uniqueness is a property of the SET ---------------------------------------

    [Fact]
    public void ShortenAll_LeavesDistinctNamesShortened()
    {
        var shortened = AudioLabel.ShortenAll([
            "Hoparlor (Realtek(R) Audio)",
            "PG27AQDM (NVIDIA High Definition Audio)",
        ]);

        Assert.Equal(["Hoparlor (Realtek(R) Audio)", "PG27AQDM (NVIDIA High)"], shortened);
    }

    [Fact]
    public void ShortenAll_GivesCollidingNamesTheirFullNamesBack()
    {
        // Measured on this machine on 2026-09-21, and the reason this method exists: both of
        // these keep the adapter's first two words, so Shorten reduces them to the same string
        // and the settings endpoint combo has been showing two identical rows.
        var shortened = AudioLabel.ShortenAll([
            "Hoparlor (Steam Streaming Speakers)",
            "Hoparlor (Steam Streaming Microphone)",
        ]);

        Assert.Equal([
            "Hoparlor (Steam Streaming Speakers)",
            "Hoparlor (Steam Streaming Microphone)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_ExpandsOnlyTheCollidingGroup()
    {
        var shortened = AudioLabel.ShortenAll([
            "Hoparlor (Steam Streaming Speakers)",
            "PG27AQDM (NVIDIA High Definition Audio)",
            "Hoparlor (Steam Streaming Microphone)",
        ]);

        Assert.Equal([
            "Hoparlor (Steam Streaming Speakers)",
            "PG27AQDM (NVIDIA High)",
            "Hoparlor (Steam Streaming Microphone)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_WithAThreeWayCollisionExpandsAllThree()
    {
        var shortened = AudioLabel.ShortenAll([
            "Speakers (Virtual Audio Cable A)",
            "Speakers (Virtual Audio Cable B)",
            "Speakers (Virtual Audio Cable C)",
        ]);

        Assert.Equal([
            "Speakers (Virtual Audio Cable A)",
            "Speakers (Virtual Audio Cable B)",
            "Speakers (Virtual Audio Cable C)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_OfAnEmptyListIsEmpty()
    {
        Assert.Empty(AudioLabel.ShortenAll([]));
    }

    [Fact]
    public void ShortenAll_KeepsTheOrderItWasGiven()
    {
        // The caller pairs these back up with endpoint ids BY INDEX, so a reordering here would
        // route audio to the wrong device under a perfectly plausible label.
        var shortened = AudioLabel.ShortenAll([
            "PG27AQDM (NVIDIA High Definition Audio)",
            "Hoparlor (Realtek(R) Audio)",
        ]);

        Assert.Equal("PG27AQDM (NVIDIA High)", shortened[0]);
    }

    // --- DistinctLabels: the short name, unless it stops telling two devices apart ------------

    [Fact]
    public void DistinctLabels_PrefersTheShorterNameWhenItIsUnique()
    {
        var labels = AudioLabel.DistinctLabels(
            ["Logitech G733 Gaming Headset", "Realtek(R) Audio"],
            ["Hoparlor (Logitech G733 Gaming Headset)", "Hoparlor (Realtek(R) Audio)"]);

        Assert.Equal(["Logitech G733 Gaming Headset", "Realtek(R) Audio"], labels);
    }

    [Fact]
    public void DistinctLabels_FallsBackOnlyForTheItemsThatCollide()
    {
        // Two identical headsets: their descriptions cannot tell them apart, so those two rows
        // get the endpoint names, which can. Nothing else pays for it.
        var labels = AudioLabel.DistinctLabels(
            ["Realtek(R) Audio", "Realtek(R) Audio", "Steam Streaming Speakers"],
            ["Hoparlor (Realtek(R) Audio)", "Line In (Realtek(R) Audio)", "Hoparlor (Steam)"]);

        Assert.Equal([
            "Hoparlor (Realtek(R) Audio)",
            "Line In (Realtek(R) Audio)",
            "Steam Streaming Speakers",
        ], labels);
    }

    [Fact]
    public void DistinctLabels_FallsBackWhenThereIsNoShorterName()
    {
        // A device can report an id and an endpoint name and no description at all.
        var labels = AudioLabel.DistinctLabels(
            ["", "Realtek(R) Audio"],
            ["Hoparlor (Some Device)", "Hoparlor (Realtek(R) Audio)"]);

        Assert.Equal(["Hoparlor (Some Device)", "Realtek(R) Audio"], labels);
    }

    [Fact]
    public void DistinctLabels_TwoMissingShorterNamesAreNotACollisionWithEachOther()
    {
        // Both fall back on their own account rather than because they matched: an empty name is
        // not a name two devices share.
        var labels = AudioLabel.DistinctLabels(
            ["", ""],
            ["Hoparlor (First)", "Hoparlor (Second)"]);

        Assert.Equal(["Hoparlor (First)", "Hoparlor (Second)"], labels);
    }

    [Fact]
    public void DistinctLabels_RefusesListsOfDifferentLengths()
    {
        // A guard rather than a Math.Min: these are paired by index, and a shorter list would
        // put one device's label on another device, which is the worst outcome available here.
        var ex = Assert.Throws<ArgumentException>(() =>
            AudioLabel.DistinctLabels(["a", "b"], ["only one"]));

        Assert.Contains("paired by index", ex.Message);
    }

    [Fact]
    public void DistinctLabels_OfEmptyListsIsEmpty()
    {
        Assert.Empty(AudioLabel.DistinctLabels([], []));
    }
}
