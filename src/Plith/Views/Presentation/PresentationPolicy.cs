using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// The decisions that differ between presentation modes, extracted from the WPF adapters
/// so they can be tested. The adapters hold a BandWindow — a ContentControl behind an
/// HwndSource — which the headless test suite cannot construct at all.
/// </summary>
public static class PresentationPolicy
{
    /// <summary>The margin the OSD has always kept from the working-area edge.</summary>
    public const double ClassicEdgeMarginDip = 96;

    /// <summary>The notch is flush with the top edge; a margin would leave a floating bar.</summary>
    public const double NotchEdgeMarginDip = 0;

    public static double EdgeMarginDip(PresentationMode mode)
        => mode == PresentationMode.AmbientNotch ? NotchEdgeMarginDip : ClassicEdgeMarginDip;

    /// <summary>
    /// True when the host is showing nothing the user would read as "the OSD is up".
    ///
    /// Classic answers with opacity. Notch cannot: it is fully opaque while parked, so it
    /// answers with whether the content is still pushed above the top edge. This is what
    /// OsdHost's ShowOsd and HideOsd both consult; reading opacity there would make ShowOsd
    /// think the notch was already visible and skip the descent entirely.
    /// </summary>
    public static bool IsAtRest(PresentationMode mode, double opacity, double targetOpacity, bool isParked)
        => mode == PresentationMode.AmbientNotch ? isParked : opacity < targetOpacity - 0.01;

    /// <summary>
    /// Whether the window should accept mouse messages right now.
    ///
    /// Classic always does — hover keep-alive needs it. The notch does not while parked:
    /// the top edge of the screen is where users drag windows to maximise, reach browser
    /// tabs and trigger Snap Layouts, so a strip that swallowed clicks there would be a
    /// defect rather than a feature.
    /// </summary>
    public static bool WantsHitTesting(PresentationMode mode, bool isParked)
        => mode != PresentationMode.AmbientNotch || !isParked;

    /// <summary>
    /// True when there is nothing on screen to take down. Deliberately NOT the same question
    /// as <see cref="IsAtRest"/>, which asks whether a show transition still has work to do.
    ///
    /// The two diverge mid-transition, and conflating them is a real defect rather than a
    /// nicety: HideOsd stops the hide timer before consulting this, so answering "at rest"
    /// during a fade-in would let the fade finish with no timer left to take the OSD down,
    /// stranding it on screen over the fullscreen video that asked it to leave.
    /// </summary>
    public static bool IsFullyHidden(PresentationMode mode, double opacity, bool isParked)
        => mode == PresentationMode.AmbientNotch ? isParked : opacity < 0.01;
}
