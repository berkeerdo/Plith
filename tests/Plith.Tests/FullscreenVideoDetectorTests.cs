using Plith.Services;

namespace Plith.Tests;

public class FullscreenVideoDetectorTests
{
    private static readonly string[] DefaultList = { "mpv", "PotPlayerMini64" };

    private const uint D3D = FullscreenVideoDetector.QUNS_RUNNING_D3D_FULL_SCREEN;
    private const uint Busy = FullscreenVideoDetector.QUNS_BUSY;

    [Fact]
    public void Disabled_NeverSuppresses()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: false, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "vlc", hideList: DefaultList));
    }

    [Fact]
    public void NotFullscreen_DoesNotSuppress()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: false, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "vlc", hideList: DefaultList));
    }

    [Fact]
    public void ExclusiveFullscreenGame_DoesNotSuppress_EvenWithPlayingMedia()
    {
        // The whole point of Phase 4h: an exclusive-fullscreen game must never lose the OSD,
        // even if Spotify happens to be playing behind it.
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: D3D,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "cs2", hideList: DefaultList));
    }

    [Fact]
    public void ExclusiveFullscreenGame_DoesNotSuppress_EvenWhenProcessIsOnHideList()
    {
        // The veto must hold against both arms of the disjunction: D3D is an unconditional hard veto
        // that overrides the decision even if the foreground process is on the hide list.
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: D3D,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "mpv", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenBrowserPlayingMedia_Suppresses()
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "chrome", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenListedPlayerWithoutSmtc_Suppresses()
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "mpv", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenBorderlessGame_DoesNotSuppress()
    {
        // Borderless-windowed games report QUNS_BUSY like any other fullscreen window. What
        // keeps them safe is that they own no playing SMTC session and aren't in the list.
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "valorant", hideList: DefaultList));
    }

    [Theory]
    [InlineData("MPV")]
    [InlineData("mpv.exe")]
    [InlineData("MPV.EXE")]
    public void HideListMatch_IsCaseInsensitiveAndTolerates_ExeSuffix(string processName)
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: processName, hideList: DefaultList));
    }

    [Fact]
    public void EmptyProcessName_DoesNotMatchAnything()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "", hideList: DefaultList));
    }

    [Fact]
    public void ParseHideList_SplitsTrimsAndDropsEmptyEntries()
    {
        var parsed = FullscreenVideoDetector.ParseHideList(" mpv , ,PotPlayerMini64 ,");
        Assert.Equal(new[] { "mpv", "PotPlayerMini64" }, parsed);
    }

    [Fact]
    public void ParseHideList_HandlesNullAndEmpty()
    {
        Assert.Empty(FullscreenVideoDetector.ParseHideList(null));
        Assert.Empty(FullscreenVideoDetector.ParseHideList("   "));
    }
}
