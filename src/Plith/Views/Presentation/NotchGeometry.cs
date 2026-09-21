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

    /// <summary>Rows the shelf draws. Every row is drawn; nothing folds and nothing scrolls.
    /// </summary>
    public const int ShelfRowCount = 3;

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

    /// <summary>One tile, square, in DIP.</summary>
    public const double ShelfTileSize = 64;

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

    /// <summary>The gap between tiles, in DIP, horizontally and vertically.</summary>
    public const double ShelfGap = 8;

    /// <summary>
    /// Everything in the shelf frame that is not the tile grid: the header with its clear
    /// control, the surface's own padding, and the window margin.
    ///
    /// DERIVED, but from two measured parts rather than from a guess. The frame was 384 x 224,
    /// and the comment that number carried decomposed it: the surface wants 210 DIP of content
    /// plus 14 DIP of margin. Of that 210, the tile columns were 149 (a 13 DIP stack caption, two
    /// 64 DIP rows and one 8 DIP gap), which leaves 61 for the header and padding. So 61 + 14.
    ///
    /// It is named rather than folded into a literal height so that changing ShelfRowCount moves
    /// the frame with it.
    /// </summary>
    private const double ShelfChromeDip = 61 + 14;

    /// <summary>
    /// The shelf surface, which is a second process's window and still belongs here.
    ///
    /// It lives in NotchGeometry because both ends of the wire need it and this file is already
    /// linked into the catcher: Plith computes the rectangle it hands over, the catcher's
    /// ShelfWindow.xaml declares the same numbers as its design size, and a third copy in a
    /// service on Plith's side would be the one free to drift.
    ///
    /// The height is an EXPRESSION over the grid above, not a literal. A literal is free to stop
    /// matching the number of rows beside it, silently, and the result is a row drawn outside the
    /// window. See scripts/render-widgets.ps1, the shelf-surface section, which renders the
    /// surface at exactly this size.
    /// </summary>
    public static readonly Size ShelfFrameDip = ShelfFrameFor(ShelfCapacity);

    /// <summary>
    /// Rows a shelf of <paramref name="itemCount"/> files needs.
    ///
    /// AN EMPTY SHELF GETS TWO, which is more than any single row of files needs and is the one
    /// deliberate asymmetry here. The empty state is the only thing the shelf draws that is not a
    /// tile: it carries an icon over a line of text, and in a single row the box around it is 352
    /// by 64. That is the aspect ratio of a text input, and it read as a field to type in rather
    /// than a place to drop onto, reported that way from the running build.
    ///
    /// Nobody watches it shrink, because the shelf cannot gain items while it is open (see
    /// <see cref="ShelfFrameFor"/>); the next open is simply the right size for what is on it.
    ///
    /// The ceiling at the top because the count arrives from a caller: ShelfStore enforces the
    /// cap, but this is the arithmetic that keeps the frame inside the design even if it did not.
    /// </summary>
    public static int ShelfRowsFor(int itemCount)
        => itemCount <= 0
            ? Math.Min(2, ShelfRowCount)
            : Math.Clamp((int)Math.Ceiling(itemCount / (double)ShelfTilesPerRow), 1, ShelfRowCount);

    /// <summary>
    /// The frame a shelf of <paramref name="itemCount"/> files opens at.
    ///
    /// THE SHELF HUGS WHAT IT HOLDS, and it decides once. Two files do not open a pane sized for
    /// fifteen, which is what a fixed three-row frame gave: a render of a seven-file shelf had a
    /// whole empty row under it.
    ///
    /// Chosen at OPEN and never changed while the shelf is up. The window is sized by Plith and
    /// applied by the catcher over the pipe, so a resize per item would spread an animation
    /// across two processes, and every cross-process coordination on this branch has cost
    /// several runs to get right. It is safe to decide once because while the shelf is open the
    /// only verbs that touch the store are RemoveItems and ClearShelf, which can only make it
    /// smaller, and the shelf window accepts no drops of its own. Removing a file reflows the
    /// content and leaves the window alone.
    /// </summary>
    public static Size ShelfFrameFor(int itemCount)
    {
        var rows = ShelfRowsFor(itemCount);
        return new Size(384, rows * ShelfTileSize + (rows - 1) * ShelfGap + ShelfChromeDip);
    }

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
    /// Where the shelf stands: the same top edge and the same centre as the open frame, in the
    /// shelf's own larger size.
    ///
    /// The centre has to match <see cref="DropTargetRect"/> exactly, because the shelf grows OUT
    /// of the open frame: the catcher animates from the open frame to this rectangle, and a centre
    /// that moved by even a few DIP would read as the shape sliding sideways while it opened
    /// rather than as the notch continuing into something larger.
    /// </summary>
    public static Rect ShelfRect(Rect hoverRect, int itemCount)
    {
        var frame = ShelfFrameFor(itemCount);
        return new Rect(hoverRect.Left + (hoverRect.Width - frame.Width) / 2, hoverRect.Top,
                        frame.Width, frame.Height);
    }

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
