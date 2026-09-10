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
    /// answers with whether the content is still pushed above the top edge. Reading opacity
    /// for the notch would make ShowOsd think it was already visible and skip the descent
    /// entirely — which is why both modes answer this one question through here rather than
    /// through the window's opacity directly.
    ///
    /// Only OsdHost's ShowOsd consults this predicate: it asks whether a show transition still
    /// has work to do. HideOsd consults <see cref="IsFullyHidden"/> instead, which asks whether
    /// there is anything on screen to take down. The two questions have different answers
    /// mid-transition, which is why they are separate predicates — see IsFullyHidden for the
    /// defect that conflating them produces.
    /// </summary>
    public static bool IsAtRest(PresentationMode mode, double opacity, double targetOpacity, bool isParked)
        => mode == PresentationMode.AmbientNotch ? isParked : opacity < targetOpacity - 0.01;

    /// <summary>
    /// Whether the window should accept mouse messages right now.
    ///
    /// Both modes now do, and for the notch that is a change worth explaining.
    ///
    /// It used to return false while parked, so the window carried WS_EX_TRANSPARENT and the
    /// top of the screen stayed usable. That works, but it is all-or-nothing: the moment the
    /// panel opens the whole window becomes solid to the mouse, including the large transparent
    /// margins around the drawn shape. With hover-to-open that meant the notch was open most of
    /// the time the pointer was anywhere near the top of the screen, and clicks meant for
    /// browser tabs underneath went to Plith instead.
    ///
    /// WS_EX_TRANSPARENT also short-circuits WM_NCHITTEST entirely — the system skips the window
    /// without ever asking it — so a per-point filter cannot run while it is set. Measured: with
    /// the notch parked, a direct WM_NCHITTEST probe never reached the window's WndProc at all.
    ///
    /// So the notch keeps hit-testing on and answers per point instead, through
    /// BandWindow.HitTestFilter: HTCLIENT on the pixels the notch actually occupies, and
    /// HTTRANSPARENT everywhere else. The top of the screen stays usable at every expansion,
    /// not just while parked.
    /// </summary>
    public static bool WantsHitTesting(PresentationMode mode, bool isParked) => true;

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
