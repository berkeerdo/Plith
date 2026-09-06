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
    public void Notch_WantsHitTesting_OnlyWhileDescended()
    {
        // The top edge is where users drag windows and reach Snap Layouts. A parked strip
        // that swallowed clicks there would be a defect, not a feature.
        Assert.False(PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, isParked: true));
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
