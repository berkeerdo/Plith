using System.IO;
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

    // ---- AUMID -> process matching -------------------------------------------------
    //
    // This predicate is what game safety rests on: Windows 11 turns most "exclusive
    // fullscreen" games into borderless windows reporting QUNS_BUSY, so the D3D veto never
    // fires for them and nothing else stops a focused game being suppressed. It had no test
    // coverage at all until these, because it lived in the Win32 gatherer.
    //
    // The Spotify string below is not invented: it is what a real machine reported.

    private const string PackagedSpotifyAumid = "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify";

    [Theory]
    [InlineData("vlc.exe", "vlc")]
    [InlineData("VLC.EXE", "vlc")]                              // case-insensitive
    [InlineData("vlc.exe", "VLC")]
    [InlineData(@"C:\Program Files\VideoLAN\VLC\vlc.exe", "vlc")]   // full-path AUMID
    [InlineData("chrome.exe", "chrome")]
    public void AumidMatchesProcess_MatchesWin32Players(string aumid, string processName)
    {
        Assert.True(FullscreenVideoDetector.AumidMatchesProcess(aumid, processName));
    }

    [Fact]
    public void AumidMatchesProcess_PackagedAumidReducesToAStem_NotToNothing()
    {
        // Documents what actually happens rather than the tempting shorthand that packaged
        // AUMIDs "cannot match". GetFileNameWithoutExtension trims from the last dot.
        Assert.Equal("SpotifyAB", Path.GetFileNameWithoutExtension(PackagedSpotifyAumid));
    }

    [Fact]
    public void AumidMatchesProcess_DoesNotCreditThePackagedPlayersOwnProcess()
    {
        // Consequence of the above: fullscreen video in a packaged player is NOT auto-hidden.
        // Failing this direction is safe (the OSD stays visible) and the hide list is the
        // override, but it is deliberate rather than accidental.
        Assert.False(FullscreenVideoDetector.AumidMatchesProcess(PackagedSpotifyAumid, "Spotify"));
    }

    [Theory]
    [InlineData("VALORANT-Win64-Shipping")]
    [InlineData("csgo")]
    [InlineData("Spotify")]
    [InlineData("Music")]
    public void AumidMatchesProcess_NeverCreditsAForegroundGame(string gameProcess)
    {
        // The false-positive that would silently destroy the headline feature: a focused game
        // credited with a media session owned by something playing in the background.
        Assert.False(FullscreenVideoDetector.AumidMatchesProcess(PackagedSpotifyAumid, gameProcess));
    }

    [Theory]
    [InlineData("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic", "Zune")]
    [InlineData("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic", "Microsoft")]
    [InlineData("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", "Spotify")]
    public void AumidMatchesProcess_RejectsSubstrings(string aumid, string processName)
    {
        // Guards the exact regression the implementation comment warns about: relaxing this
        // to Contains would make every one of these true.
        Assert.False(FullscreenVideoDetector.AumidMatchesProcess(aumid, processName));
    }

    [Theory]
    [InlineData(null, "vlc")]
    [InlineData("", "vlc")]
    [InlineData("   ", "vlc")]
    [InlineData("vlc.exe", null)]
    [InlineData("vlc.exe", "")]
    public void AumidMatchesProcess_FailsTowardShowingTheOsd(string? aumid, string? processName)
    {
        Assert.False(FullscreenVideoDetector.AumidMatchesProcess(aumid, processName));
    }
}
