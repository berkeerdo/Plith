using Plith.Services;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class PresentationPolicyTests
{
    [Fact]
    public void Classic_KeepsTheExistingEdgeMargin()
    {
        // 96 is the value OsdHost.EdgeMarginDip has always used. Classic must be
        // byte-identical to today, so this is a regression pin, not a preference.
        Assert.Equal(96, PresentationPolicy.EdgeMarginDip(PresentationMode.ClassicOsd));
    }

    [Fact]
    public void Notch_SitsFlushWithTheEdge()
    {
        Assert.Equal(0, PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch));
    }

    [Fact]
    public void Classic_IsAtRest_WhenOpacityIsBelowTarget()
    {
        Assert.True(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 0, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_IsNotAtRest_WhenFullyVisible()
    {
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 1, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_IsNotAtRest_AtAReducedTargetOpacity()
    {
        // OsdOpacityPercent can be as low as 50. "At rest" means below the target, not
        // below 1.0 — reading it as the latter would treat a 70 %-opacity OSD as hidden
        // and restart its fade-in on every repeat, which is the Phase 5 flicker defect.
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 0.7, targetOpacity: 0.7, isParked: false));
    }

    [Fact]
    public void Notch_IsAtRest_DependsOnParkedNotOpacity()
    {
        // The notch is fully opaque while parked, so opacity says nothing about it.
        Assert.True(PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, opacity: 1, targetOpacity: 1, isParked: true));
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, opacity: 1, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_AlwaysWantsHitTesting()
    {
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: true));
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: false));
    }

    [Fact]
    public void Notch_WantsHitTesting_AtEveryExpansion()
    {
        // This asserted the opposite until the click-through mechanism changed, and the reason is
        // worth keeping rather than just flipping the expectation.
        //
        // The old contract switched the whole window between "solid to the mouse" and
        // "click-through" via WS_EX_TRANSPARENT. That is all-or-nothing, and the moment the panel
        // opened it captured its ENTIRE rectangle — including the wide transparent margins around
        // the drawn shape — so the OSD sat over browser tabs and window controls and took the
        // clicks meant for them.
        //
        // Worse, WS_EX_TRANSPARENT makes the system skip the window without sending WM_NCHITTEST
        // at all, so no per-point rule could run while it was set. That was measured: with the
        // notch parked, a direct WM_NCHITTEST probe never reached the window's WndProc.
        //
        // So the notch now keeps hit-testing on at every expansion and answers per point instead,
        // through BandWindow.HitTestFilter — HTCLIENT on the pixels it actually occupies,
        // HTTRANSPARENT everywhere else. The geometry behind that decision is covered by
        // NotchGeometryTests.SurfaceSize_*; what this test pins is that the policy no longer
        // tries to make the same decision at window granularity.
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, isParked: true));
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, isParked: false));
    }

    [Fact]
    public void Classic_IsFullyHidden_OnlyAtZeroOpacity()
    {
        Assert.True(PresentationPolicy.IsFullyHidden(PresentationMode.ClassicOsd, opacity: 0, isParked: false));
        Assert.False(PresentationPolicy.IsFullyHidden(PresentationMode.ClassicOsd, opacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_MidFadeIn_IsAtRestButNotFullyHidden()
    {
        // The regression this predicate exists to prevent. HideOsd stops the hide timer before
        // its guard, so if it read IsAtRest here it would return early, the fade-in would
        // complete, and nothing would ever take the OSD back down.
        const double midFade = 0.3;
        Assert.True(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, midFade, targetOpacity: 1.0, isParked: false));
        Assert.False(PresentationPolicy.IsFullyHidden(PresentationMode.ClassicOsd, midFade, isParked: false));
    }

    [Fact]
    public void Notch_IsFullyHidden_TracksParked()
    {
        Assert.True(PresentationPolicy.IsFullyHidden(PresentationMode.AmbientNotch, opacity: 1, isParked: true));
        Assert.False(PresentationPolicy.IsFullyHidden(PresentationMode.AmbientNotch, opacity: 1, isParked: false));
    }
}
