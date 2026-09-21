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
    /// Two earlier passes let the PAGE drive the size and both were rejected on the mockup.
    /// Varying both dimensions made the notch itself appear to jump around while swiping;
    /// varying only the height still moved the bottom edge on every page turn. A single frame
    /// that content moves through is the only version that reads as one object, and that rule
    /// still holds: this is one frame, one size, for every page.
    ///
    /// The height is set by the FULLEST page rather than chosen, and on 2026-09-21 the fullest
    /// page changed. It was 116, sized for the weather page's ~73 DIP of content. The media page
    /// was then rebuilt after Alcove and asked for more: the user looked at the result, called
    /// the text cramped, and asked for the whole notch to grow rather than for that one page to
    /// be squeezed further. So 164, which is 121 DIP of content, 14 of top padding, and the 29
    /// below that the page rail's lane lives in.
    ///
    /// 356 x 164 is 2.17:1. Alcove's own media panel, measured from its press screenshot, is
    /// about 2.16:1, and matching that is the point: the layout this frame now holds is Alcove's
    /// stacking rather than a rail squeezed in beside it.
    ///
    /// What the extra 48 DIP bought, and why it was not spent on the text column directly: the
    /// transport moved OUT of a right-hand rail and onto its own centred row, which hands the
    /// whole width back to the title. The text column went from 116 DIP to 244. Widening the
    /// frame instead would have solved the same complaint while moving further from Alcove's
    /// proportion, and it was offered and not taken.
    ///
    /// The clock page carries more empty space as a result. Centred in a steady frame, that
    /// reads as deliberate.
    /// </summary>
    public static readonly Size OpenFrameDip = new(356, 164);

    /// <summary>Tiles in one row of the shelf grid.</summary>
    public const int ShelfTilesPerRow = 5;

    /// <summary>
    /// Rows the shelf draws. Every row is drawn; nothing folds and nothing scrolls.
    ///
    /// TWO, not three, because the shelf is the notch's own page now rather than a pane of its
    /// own. Three rows needed 384 x 290 and that surface is what a person reported as a separate
    /// shelf; two rows fit the 121 DIP band the open frame gives a page, with slack. The cost is
    /// ten files instead of fifteen, and it is the right trade: a cap the surface cannot draw is
    /// the defect this constant exists to prevent.
    /// </summary>
    public const int ShelfRowCount = 2;

    /// <summary>
    /// The most files the shelf holds, defined as exactly what the grid can draw.
    ///
    /// This is the whole guarantee of the flat shelf: a file that is on the shelf is on the
    /// screen, in the UIA tree, and reachable by a key. The stack model had a cap of 20 against a
    /// surface that could draw 10, and the other ten sat behind count chips that no screen reader
    /// could see. Defining the cap as the product rather than typing 15 is what keeps the two
    /// from drifting apart across the process boundary between ShelfStore and ShelfSurface.
    /// </summary>
    public const int ShelfCapacity = ShelfTilesPerRow * ShelfRowCount;

    /// <summary>
    /// One tile, in DIP. NOT square any more, and both numbers come from the frame rather than
    /// from taste.
    ///
    /// Width: five tiles must fit the content band, which is 356 less the page inset's 18 a side,
    /// so 320. A tile occupies width + gap, so 5 * (56 + 8) = 320 exactly.
    ///
    /// Height: two rows and one gap must fit the vertical band, which is 164 less the inset's 14
    /// top and 29 bottom (the bottom already clears the page rail), so 121. At 52 the pair comes
    /// to 112 and leaves slack; at 56 it would be 120 and leave one DIP, which is the kind of
    /// margin that disappears the next time a font metric moves.
    ///
    /// A tile holds a 24 DIP icon over ONE line of name at 52. Two lines do not fit, and the pane
    /// that used to carry them is gone, so a long name survives only in the tooltip. That is a
    /// real loss, recorded in the spec rather than discovered later.
    /// </summary>
    public const double ShelfTileWidth = 56;

    /// <summary>See <see cref="ShelfTileWidth"/> for where 52 comes from.</summary>
    public const double ShelfTileHeight = 52;

    /// <summary>Columns in the output picker's grid.</summary>
    public const int OutputPickerColumns = 2;

    /// <summary>Rows in the output picker's grid.</summary>
    public const int OutputPickerRows = 3;

    /// <summary>
    /// How many cells the output picker draws, which is also how many outputs it can show.
    ///
    /// The product of the grid rather than a literal, exactly like <see cref="ShelfCapacity"/>:
    /// the number that decides whether a device appears at all must not be free to drift from
    /// the grid that draws it. With more endpoints than this, the last cell becomes a door to
    /// Windows' own sound settings rather than a silent fold, because a folded cell is in no UIA
    /// tree and so is invisible to a screen reader and reachable by no key.
    /// </summary>
    public const int OutputPickerCapacity = OutputPickerColumns * OutputPickerRows;

    /// <summary>
    /// One output cell's height, in DIP. Three of them plus two gaps and the picker's header fill
    /// the same 121 DIP content band every other page gets: 14 + 3 + (3 * 32) + (2 * 4) = 121.
    ///
    /// It was 16, and 16 was recorded in the spec as "the thinnest number in this design and the
    /// one most likely to need correcting on a render". The frame growing to 164 corrected it
    /// without anything else changing: the same grid now has 104 DIP to put three rows in
    /// instead of 56.
    /// </summary>
    public const double OutputPickerCellHeight = 32;

    /// <summary>The gap between output cells, in DIP, horizontally and vertically.</summary>
    public const double OutputPickerCellGap = 4;

    /// <summary>
    /// The page's content box: the open frame less the inset that positions content inside it.
    ///
    /// Derived rather than typed, and it exists because a page that draws NOTHING still has to
    /// claim its space: ShelfWidget became a blank page when the shelf moved into the catcher's
    /// window, and a page measuring zero makes Reposition early-return and leaves WidgetFrame
    /// sizing its track from nothing.
    /// </summary>
    public static readonly double PageContentWidthDip =
        OpenFrameDip.Width - PageInsetDip.Left - PageInsetDip.Right;

    /// <inheritdoc cref="PageContentWidthDip"/>
    public static readonly double PageContentHeightDip =
        OpenFrameDip.Height - PageInsetDip.Top - PageInsetDip.Bottom;

    /// <summary>
    /// The band along the top of the shelf page where the hovered file's name is drawn.
    ///
    /// It is the page inset's own top margin, which is 14 and is otherwise empty, so the name
    /// costs the tiles nothing. Named rather than repeated as a literal because the shelf surface
    /// is in the other project and a 14 typed there would stop following this one.
    /// </summary>
    public const double ShelfNameRowDip = 14;

    /// <summary>The gap between tiles, in DIP, horizontally and vertically.</summary>
    public const double ShelfGap = 8;

    /// <summary>
    /// Everything in the shelf frame that is not the tile grid, as the SUM OF THE PARTS THAT
    /// DECLARE IT rather than a number derived from a frame that no longer exists.
    ///
    /// It said 61 + 14, and the comment that carried it decomposed a 384 x 224 frame whose tile
    /// columns included a 13 DIP stack caption. Stacks are gone, the caption with them, and the
    /// leftover was 7 DIP short of what the surface measures at EVERY row count, which is the
    /// second half of the shelf being cut: the growth clamp in GrowthStart stops the page hanging
    /// outside its window, and this stops the window being smaller than the page in the first
    /// place. Measured on 2026-09-21, one to fifteen files: the surface wanted 146, 218 and 290
    /// where the frame gave 139, 211 and 283.
    ///
    /// The parts, each traceable to a declaration in ShelfSurface.xaml:
    ///   2  the surface's own border, BorderThickness="1" top and bottom
    ///   32 the content grid's Margin="16", top and bottom
    ///   40 the header row: a 28 DIP close box (the taller of the two controls) plus its 12 DIP
    ///      bottom margin
    ///
    /// scripts/render-widgets.ps1 asserts this against the surface's measured DesiredSize for
    /// every shelf size, which is the only check that can see the two disagree: the surface lives
    /// in the other project and this file cannot measure it.
    /// </summary>
    private const double ShelfChromeDip = 2 + 32 + 40;

    /// <summary>
    /// The rail's row at the bottom of the frame, which every page keeps its content clear of.
    ///
    /// FIXED rather than sized to its content, which is the rule the dots lane it replaced had
    /// for a reason worth keeping: sized to content, a page measuring one pixel taller than its
    /// share pushes the chrome past the panel's padding and onto the frame's edge, which is what
    /// the weather page did on the mockup. A fixed row over a page that cannot exceed its track
    /// means an over-tall page clips inside itself instead of displacing the chrome.
    ///
    /// It replaces DotsLaneDip, which said 14, was used by nothing at all, and disagreed with
    /// both the frame's actual row (23) and the media page's own comment (20). Three numbers for
    /// one measurement, two of them wrong, none of them load-bearing.
    /// </summary>
    public const double PageRailRowDip = 23;

    /// <summary>The same number as a GridLength, so WidgetFrame.xaml's row can take it from here
    /// rather than repeating it.</summary>
    public static readonly GridLength PageRailRow = new(PageRailRowDip);

    /// <summary>
    /// The one content inset every widget page lays itself out in.
    ///
    /// It was four different insets: the media page 18,14,18,29, the clock 20,14,20,26, the shelf
    /// 20,12,20,22 and the weather readout 18,0,18,26. Paging between them moved the left edge by
    /// two DIP and put three different floors under the content, which is visible as a sideways
    /// jump on every page turn, and the shelf's 22 reached one DIP INTO the rail's row: its own
    /// comment worried about exactly that collision.
    ///
    /// The bottom is the rail's row plus six, so the row is cleared rather than shared.
    /// </summary>
    public static readonly Thickness PageInsetDip = new(18, 14, 18, PageRailRowDip + 6);

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
    /// The room the panel leaves around itself for its shadow.
    ///
    /// Plith's own window is the panel plus this on the left, the right and the BOTTOM, and
    /// nothing at the top: the notch is flush with the screen's top edge, where a shadow has
    /// nothing to fall on. That is why the notch window measures 384 by 178 for a 356 by 164
    /// panel.
    ///
    /// Named here rather than left as the literal 14 in OsdContent.xaml because the catcher's
    /// shelf window has to match it. Without the match the handover is visible: the catcher's
    /// window was exactly the panel, so at the swap the shadow vanished and the panel's edges
    /// moved by 14 DIP, which a person reported as the notch closing and reopening in a fraction
    /// of a second.
    /// </summary>
    public const double PanelShadowMarginDip = 14;

    /// <summary>The same margin as a Thickness, so ShelfWindow.xaml can inset its shape by it
    /// rather than repeating the number. Nothing at the top: the notch is flush with the screen's
    /// edge.</summary>
    public static readonly Thickness ShelfShapeMargin =
        new(PanelShadowMarginDip, 0, PanelShadowMarginDip, PanelShadowMarginDip);

    /// <summary>
    /// The WINDOW the catcher opens for the shelf: the page rect grown by the shadow margin, so
    /// it is the same rectangle Plith's own window occupies.
    ///
    /// <see cref="ShelfPageRect"/> is the panel; this is the window around it. The catcher draws
    /// its shape inset by the same margin and casts the same shadow, so the two surfaces are
    /// comparable pixel for pixel and the swap has nothing to show.
    /// </summary>
    public static Rect ShelfWindowRect(Rect hoverRect)
    {
        var page = ShelfPageRect(hoverRect);
        return new Rect(page.Left - PanelShadowMarginDip,
                        page.Top,
                        page.Width + (PanelShadowMarginDip * 2),
                        page.Height + PanelShadowMarginDip);
    }

    /// <summary>
    /// Where the shelf stands: the notch's open frame, exactly.
    ///
    /// It IS <see cref="DropTargetRect"/>, and this method exists anyway, for two reasons. The
    /// name says what the rectangle is for, so a reader of ShelfSession does not have to know
    /// that the shelf and the drop target happen to be the same box. And a second caller
    /// computing the open frame itself is precisely how the two would drift: the previous version
    /// of this method returned a LARGER rectangle, 384 wide by up to 290 tall, and that
    /// difference is what a person reported as the shelf being a separate window rather than the
    /// notch.
    ///
    /// The shelf no longer depends on how many files are on it. There is nothing to hug: the page
    /// is the size of every other page.
    /// </summary>
    public static Rect ShelfPageRect(Rect hoverRect) => DropTargetRect(hoverRect);


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
