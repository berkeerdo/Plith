using System.Windows;
using System.Windows.Media;

namespace Plith.Views.Presentation;

/// <summary>
/// Geometry for the Ambient Notch, kept free of every WPF window type so it can be tested
/// on the headless suite. This mirrors the FullscreenVideoDetector / FullscreenVideoWatcher
/// split: the watcher gathers, the detector decides, and only the decision is testable.
///
/// Every length here is in device-independent units. The one place physical pixels enter
/// is <see cref="PhysicalToDip"/>, which exists so that conversion happens once, at a named
/// boundary, instead of being spread across call sites — mixing the two spaces silently
/// breaks every comparison on a non-100 % display.
///
/// The notch is ONE shape that changes size, not a card that travels. Everything below is
/// parameterised by a single expansion progress t (0 = collapsed pill, 1 = open panel), so
/// width, height, corner radius and content opacity cannot drift out of step with each
/// other — they are all derived from the same number by the same animation clock.
///
/// The first design translated a full-width card down from behind a separate strip element.
/// It was rejected on a running build: two objects sliding past each other reads as a drawer
/// opening, not as a notch growing, and the strip spanned the whole panel width so its
/// horizontal extent changed with the content. A notch is a fixed anchor whose shape morphs.
/// </summary>
public static class NotchGeometry
{
    /// <summary>Width of the collapsed pill, in DIP. Deliberately narrow and, unlike the
    /// panel it opens into, CONSTANT: the resting shape must not change size when a media
    /// card appears or goes away, or the anchor visibly jumps and the illusion breaks.</summary>
    public const double CollapsedWidthDip = 190;

    /// <summary>Bottom corner radius of the collapsed pill and of the open panel. Both are
    /// clamped against the surface height by <see cref="SurfaceRadius"/>, so a user who sets
    /// a 2 DIP resting height gets a 2 DIP radius rather than a squashed 8.</summary>
    public const double CollapsedRadiusDip = 8;
    public const double ExpandedRadiusDip = 16;

    /// <summary>
    /// Expansion progress at which the content starts to fade in. Below it the panel is a
    /// blank surface still growing.
    ///
    /// This ordering is the whole effect: the shape settles first and the content arrives
    /// into it. Cross-fading content while the surface is still moving reads as a window
    /// resizing, which is exactly what a notch must not look like.
    /// </summary>
    public const double ContentFadeStart = 0.55;

    /// <summary>
    /// How far the notch opens on hover alone: enough to acknowledge the pointer, nowhere near
    /// enough to show content.
    ///
    /// Hover used to open the panel outright, and that was wrong twice over. It is not what the
    /// reference apps do — there the notch grows a little under the pointer and opens on a
    /// click — and it made the panel solid to the mouse whenever the pointer was anywhere near
    /// the top of the screen, so clicks meant for browser tabs underneath went to the OSD.
    ///
    /// Below ContentFadeStart on purpose: a peek must never start fading content in, or it stops
    /// being a hint and becomes a half-open panel.
    /// </summary>
    public const double PeekExpand = 0.14;

    /// <summary>
    /// Minimum height of the cursor target over the collapsed pill. The resting height is a
    /// user setting that goes down to 2 DIP, and a 2 DIP tall target cannot be hit
    /// deliberately — the pointer skips it between two mouse samples. The target is
    /// invisible, so making it taller than what is drawn costs nothing on screen.
    /// </summary>
    public const double MinHoverHeightDip = 8;

    /// <summary>
    /// The one size every widget page opens to. Content moves through the frame; the frame does
    /// not move.
    ///
    /// Two earlier passes let the page drive the size and both were rejected on the mockup.
    /// Varying both dimensions made the notch itself appear to jump around while swiping;
    /// varying only the height still moved the bottom edge on every page turn. A single frame
    /// that content moves through is the only version that reads as one object.
    ///
    /// The height is set by the FULLEST page rather than chosen: the weather page needs ~73 DIP
    /// of content, plus 23 of padding and the 20 the dots lane occupies. The clock page carries
    /// empty space as a result, and centred in a steady frame that reads as deliberate.
    /// </summary>
    public static readonly Size OpenFrameDip = new(356, 116);

    /// <summary>
    /// The shelf surface, which is a second process's window and still belongs here.
    ///
    /// MEASURED rather than chosen: ShelfSurface wants 210 DIP of content at 384 wide, plus 14
    /// DIP of margin. See scripts/render-widgets.ps1, the shelf-surface section, which renders it
    /// at exactly this size.
    ///
    /// It lives in NotchGeometry because both ends of the wire need it and this file is already
    /// linked into the catcher: Plith computes the rectangle it hands over, the catcher's
    /// ShelfWindow.xaml declares the same numbers as its design size, and a third copy in a
    /// service on Plith's side would be the one free to drift.
    /// </summary>
    public static readonly Size ShelfFrameDip = new(384, 224);

    /// <summary>
    /// Height of the page-dot lane, and it is FIXED rather than sized to its content.
    ///
    /// Sized to content, a page measuring one pixel taller than its share pushes the dots past
    /// the panel's padding and onto the frame's edge — which is what the weather page did on the
    /// mockup. A fixed lane over a page row that cannot exceed its track means an over-tall page
    /// clips inside itself instead of displacing the chrome.
    /// </summary>
    public const double DotsLaneDip = 14;

    /// <summary>
    /// The event HUD: short, wide and transient. A different shape family from the widget frame,
    /// and the difference is the point — which shape the notch takes says who started the
    /// interaction. A volume key is not a request to go anywhere, so the answer to it must not
    /// look like a place you went.
    /// </summary>
    public static readonly Size HudDip = new(300, 46);

    /// <summary>The HUD with a media row in it, which needs the extra width for a title and the
    /// extra height for two lines of text beside the art.</summary>
    public static readonly Size HudWideDip = new(372, 54);

    /// <summary>Gap between the page row and the dots lane.</summary>
    public const double DotsGapDip = 6;

    /// <summary>How far an incoming page starts from its resting position, in DIP. Signed by
    /// the direction paged, so the page enters from the side the gesture came from and paging
    /// reads as movement through a frame rather than a crossfade in place.</summary>
    public const double PageSlideDip = 14;

    /// <summary>
    /// The notch's own outline, as a clip for a page that reaches its edges.
    ///
    /// Only the BOTTOM corners are round: the notch is flush with the top of the screen, and
    /// rounding the top would carve two notches of desktop out of the edge it is part of.
    /// Returns null for a size nothing can be clipped to, so a caller can assign the result
    /// straight to Clip.
    ///
    /// Shared rather than written twice. Two pages bleed to the frame's edges now - the sky and
    /// the album backdrop - and a second copy is a second place for the radius to drift.
    /// </summary>
    public static Geometry? BottomRoundedClip(Size size)
    {
        if (size.Width <= 0 || size.Height <= 0) return null;

        var r = Math.Min(ExpandedRadiusDip, Math.Min(size.Width, size.Height) / 2);
        var figure = new PathFigure { StartPoint = new Point(0, 0), IsClosed = true, IsFilled = true };
        figure.Segments.Add(new LineSegment(new Point(size.Width, 0), false));
        figure.Segments.Add(new LineSegment(new Point(size.Width, size.Height - r), false));
        figure.Segments.Add(new ArcSegment(
            new Point(size.Width - r, size.Height), new Size(r, r), 0, false, SweepDirection.Clockwise, false));
        figure.Segments.Add(new LineSegment(new Point(r, size.Height), false));
        figure.Segments.Add(new ArcSegment(
            new Point(0, size.Height - r), new Size(r, r), 0, false, SweepDirection.Clockwise, false));

        var geometry = new PathGeometry();
        geometry.Figures.Add(figure);
        geometry.Freeze();
        return geometry;
    }

    public static double Clamp01(double t) => t < 0 ? 0 : t > 1 ? 1 : t;

    public static double Lerp(double from, double to, double t) => from + (to - from) * Clamp01(t);

    /// <summary>
    /// Size of the notch surface at progress <paramref name="t"/>.
    ///
    /// <paramref name="expanded"/> is the measured size of the card content. Clamped at the
    /// collapsed size on both axes: during the first layout pass the measurement is zero, and
    /// interpolating toward zero would shrink the resting pill to nothing for a frame.
    /// </summary>
    public static Size SurfaceSize(double collapsedWidth, double collapsedHeight, Size expanded, double t)
    {
        var w = Lerp(collapsedWidth, Math.Max(expanded.Width, collapsedWidth), t);
        var h = Lerp(collapsedHeight, Math.Max(expanded.Height, collapsedHeight), t);
        return new Size(Math.Max(0, w), Math.Max(0, h));
    }

    /// <summary>Bottom corner radius at progress <paramref name="t"/>, never more than the
    /// surface height — past that WPF clamps it anyway, and the un-clamped value would make
    /// the collapsed pill's radius depend on a height it can no longer reach.</summary>
    public static double SurfaceRadius(double t, double surfaceHeight)
        => Math.Min(Lerp(CollapsedRadiusDip, ExpandedRadiusDip, t), Math.Max(0, surfaceHeight));

    /// <summary>Content opacity at progress <paramref name="t"/>: nothing until the surface
    /// is <see cref="ContentFadeStart"/> of the way open, then a linear ramp to full.</summary>
    public static double ContentOpacity(double t)
    {
        t = Clamp01(t);
        if (t <= ContentFadeStart) return 0;
        return (t - ContentFadeStart) / (1 - ContentFadeStart);
    }

    /// <summary>
    /// Depth of the band that counts as a drag arriving, in DIP.
    ///
    /// Deeper than the hover target, and for a different reason. A hover is a pointer placed
    /// deliberately; a drag is a hand carrying something toward the top of the screen, usually
    /// faster and rarely stopping within a few pixels of the edge.
    ///
    /// This was 28 and that was measured wrong. Every failed attempt on the first live run
    /// produced no band entry at all, and the one that worked entered at y=0 — the person had to
    /// press the file against the very top edge of the screen to be seen. A band you can only
    /// hit by hitting the edge is not a target.
    ///
    /// Too deep has a cost too: a window dragged to the top to maximise ends up in the same
    /// place. That error is the cheaper one, because the catcher withdraws on its own when no
    /// file drag follows it.
    /// </summary>
    public const double DragApproachHeightDip = 48;

    /// <summary>
    /// The band that counts as a drag arriving: as wide as the target it opens, and deep enough
    /// to catch a moving hand.
    ///
    /// The width matters as much as the depth and was wrong for the same reason. The hover
    /// target is 190 DIP because that is the resting pill; the panel a drop lands in is 356. A
    /// drag aimed at the middle of what it can see could be 80 DIP outside the band that decides
    /// whether it was aimed at all.
    /// </summary>
    public static Rect DragApproachRect(Rect hoverRect)
    {
        var target = DropTargetRect(hoverRect);
        return new Rect(target.Left, target.Top, target.Width,
                        Math.Max(hoverRect.Height, DragApproachHeightDip));
    }

    /// <summary>
    /// Where the catcher stands while it holds the notch's place: the open frame, centred on the
    /// same anchor the notch rests at.
    ///
    /// The open frame rather than the resting pill, because this is a target a person has to hit
    /// while already holding something — and rather than the OSD window's own rectangle, which
    /// is mostly transparent and whose width follows whatever card happens to be showing.
    /// </summary>
    public static Rect DropTargetRect(Rect hoverRect)
        => new(hoverRect.Left + (hoverRect.Width - OpenFrameDip.Width) / 2, hoverRect.Top,
               OpenFrameDip.Width, OpenFrameDip.Height);

    /// <summary>
    /// Where the shelf stands: the same top edge and the same centre as the open frame, in the
    /// shelf's own larger size.
    ///
    /// The centre has to match <see cref="DropTargetRect"/> exactly, because the shelf grows OUT
    /// of the open frame: the catcher animates from 356 x 116 to this rectangle, and a centre
    /// that moved by even a few DIP would read as the shape sliding sideways while it opened
    /// rather than as the notch continuing into something larger.
    /// </summary>
    public static Rect ShelfRect(Rect hoverRect)
        => new(hoverRect.Left + (hoverRect.Width - ShelfFrameDip.Width) / 2, hoverRect.Top,
               ShelfFrameDip.Width, ShelfFrameDip.Height);

    /// <summary>
    /// The one conversion back out of DIP, mirroring <see cref="PhysicalToDip"/>.
    ///
    /// Needed because the rectangle is handed to another process, which applies it with
    /// SetWindowPos and must not repeat this arithmetic with its own idea of the scale.
    /// </summary>
    public static (int X, int Y, int Width, int Height) DipToPhysical(Rect dip, double scale)
    {
        if (scale <= 0) scale = 1.0;
        return ((int)Math.Round(dip.Left * scale), (int)Math.Round(dip.Top * scale),
                (int)Math.Round(dip.Width * scale), (int)Math.Round(dip.Height * scale));
    }

    /// <summary>
    /// Screen rectangle of the cursor target over the collapsed pill, in DIP.
    ///
    /// <paramref name="windowLeft"/> and <paramref name="windowTop"/> are the values
    /// Reposition() computed, which are derived from Screen.WorkingArea — so a taskbar docked
    /// to the top moves this rectangle down with it and needs no special case here.
    ///
    /// The pill is centred in the window rather than inset from its edges, because the pill's
    /// width is fixed while the window's follows the content. Deriving the target from the
    /// window edges (which the previous full-width strip did) would have made the hover zone
    /// grow every time a media card appeared.
    /// </summary>
    public static Rect HoverRect(double windowLeft, double windowTop, double windowWidth, double collapsedHeight)
    {
        var width = Math.Min(CollapsedWidthDip, Math.Max(0, windowWidth));
        var left = windowLeft + (Math.Max(0, windowWidth) - width) / 2;
        var height = Math.Max(collapsedHeight, MinHoverHeightDip);
        return new Rect(left, windowTop, width, height);
    }



    /// <summary>
    /// Convert a GetCursorPos result (physical pixels) into DIP.
    ///
    /// A non-positive scale means the DPI query failed; treat it as 1:1 rather than
    /// dividing by zero, which would send the cursor to infinity and make the pill
    /// permanently unhoverable.
    /// </summary>
    public static Point PhysicalToDip(int physicalX, int physicalY, double dpiScale)
    {
        if (dpiScale <= 0) return new Point(physicalX, physicalY);
        return new Point(physicalX / dpiScale, physicalY / dpiScale);
    }

    /// <summary>Hit test, both operands in DIP. Rect.Contains is inclusive on left/top and
    /// exclusive on right/bottom, which is the behaviour we want at the screen edge.</summary>
    public static bool IsInsideNotch(Rect hoverRect, Point cursorDip) => hoverRect.Contains(cursorDip);
}
