using Plith.Cards;
using Plith.Views.Presentation;

namespace Plith.Tests;

public sealed class NotchEventPolicyTests
{
    /// <summary>
    /// The change this type exists for, and the case that was measured doing harm.
    ///
    /// A track advancing because the song ended is not something the person did. Taking away a
    /// frame they deliberately opened, to tell them about it, interrupts them with news they did
    /// not ask for. Measured on 2026-09-20: it also broke a verification run, which paged to the
    /// shelf widget and then clicked on nothing because Spotify had advanced in between.
    /// </summary>
    [Fact]
    public void AnUncausedTrackChangeLeavesTheOpenFrameAlone()
    {
        Assert.True(NotchEventPolicy.KeepsOpenFrame(
            ShowReason.MediaChange, frameIsOpen: true, pageAlreadyShowsIt: false));
    }

    /// <summary>
    /// The other half, and why "an event never takes the frame" was tried once and reverted: a
    /// volume key with the frame open would then show nothing at all, because no widget page
    /// carries the volume. A key the person just pressed is feedback for what they did, and
    /// feedback has to be visible.
    /// </summary>
    [Fact]
    public void AVolumeKeyTakesTheFrameSoItsFeedbackIsSeen()
    {
        Assert.False(NotchEventPolicy.KeepsOpenFrame(
            ShowReason.VolumeKey, frameIsOpen: true, pageAlreadyShowsIt: false));
    }

    /// <summary>
    /// "The volume changed" does not say who changed it, so it is not treated as the person's
    /// doing. Their own key press arrives separately as VolumeKey, which is what makes this safe:
    /// nothing is lost by declining to interrupt on the ambiguous signal.
    /// </summary>
    [Fact]
    public void AnAudioChangeIsNotTreatedAsSomethingThePersonDid()
    {
        Assert.True(NotchEventPolicy.KeepsOpenFrame(
            ShowReason.AudioChange, frameIsOpen: true, pageAlreadyShowsIt: false));
    }

    /// <summary>
    /// The rule that was already there and still holds: an event about the page you are standing
    /// on updates that page. Replacing the media page with a media HUD would take away the place
    /// you went to precisely in order to see this.
    /// </summary>
    [Theory]
    [InlineData(ShowReason.MediaCommand)]
    [InlineData(ShowReason.MediaChange)]
    public void AnEventTheOpenPageAlreadyShowsNeverTakesTheFrame(ShowReason reason)
    {
        Assert.True(NotchEventPolicy.KeepsOpenFrame(reason, frameIsOpen: true, pageAlreadyShowsIt: true));
    }

    /// <summary>A transport command from a page that does not show media is still the person's
    /// own press, so it earns its HUD.</summary>
    [Fact]
    public void ATransportCommandFromAnotherPageTakesTheFrame()
    {
        Assert.False(NotchEventPolicy.KeepsOpenFrame(
            ShowReason.MediaCommand, frameIsOpen: true, pageAlreadyShowsIt: false));
    }

    /// <summary>
    /// With no frame open there is nothing to keep, whatever the reason. Stated because the
    /// caller's other branches read better when this one cannot surprise them.
    /// </summary>
    [Theory]
    [InlineData(ShowReason.MediaChange)]
    [InlineData(ShowReason.VolumeKey)]
    [InlineData(ShowReason.AudioChange)]
    public void NothingIsKeptWhenNoFrameIsOpen(ShowReason reason)
    {
        Assert.False(NotchEventPolicy.KeepsOpenFrame(reason, frameIsOpen: false, pageAlreadyShowsIt: false));
        Assert.False(NotchEventPolicy.KeepsOpenFrame(reason, frameIsOpen: false, pageAlreadyShowsIt: true));
    }

    /// <summary>
    /// A null reason reaches this from callers that do not carry one. It is not the person's
    /// doing by any evidence available here, so it does not interrupt.
    /// </summary>
    [Fact]
    public void AnUnknownReasonDoesNotInterrupt()
    {
        Assert.True(NotchEventPolicy.KeepsOpenFrame(null, frameIsOpen: true, pageAlreadyShowsIt: false));
    }
}
