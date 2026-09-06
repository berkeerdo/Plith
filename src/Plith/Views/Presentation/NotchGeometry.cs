using System.Windows;

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
/// </summary>
public static class NotchGeometry
{
    /// <summary>Content offset when the notch is fully descended.</summary>
    public const double DescendedOffset = 0.0;

    /// <summary>
    /// Y offset applied to the OSD content while the notch is parked: negative, pushing all
    /// but the strip above the top of the window.
    ///
    /// <paramref name="contentInset"/> is the drop-shadow margin on OsdContent's outer Grid
    /// (14 DIP today). The card's visible border starts that far below the content origin,
    /// so it has to be subtracted or the strip renders that much too short.
    ///
    /// Clamped at zero: during the first layout pass DesiredSize is still zero, and a
    /// positive offset there would drop the card into mid-screen for a frame.
    /// </summary>
    public static double RestingOffset(double contentHeight, double stripHeight, double contentInset)
        => -Math.Max(0, contentHeight - stripHeight - contentInset);

    /// <summary>
    /// Screen rectangle of the visible strip, in DIP. <paramref name="windowLeft"/> and
    /// <paramref name="windowTop"/> are the values Reposition() computed, which are derived
    /// from Screen.WorkingArea — so a taskbar docked to the top moves this rectangle down
    /// with it and needs no special case here.
    /// </summary>
    public static Rect StripRect(
        double windowLeft, double windowTop, double contentWidth, double stripHeight, double contentInset)
    {
        var left = windowLeft + contentInset;
        var width = Math.Max(0, contentWidth - contentInset * 2);
        return new Rect(left, windowTop, width, Math.Max(0, stripHeight));
    }

    /// <summary>
    /// Convert a GetCursorPos result (physical pixels) into DIP.
    ///
    /// A non-positive scale means the DPI query failed; treat it as 1:1 rather than
    /// dividing by zero, which would send the cursor to infinity and make the strip
    /// permanently unhoverable.
    /// </summary>
    public static Point PhysicalToDip(int physicalX, int physicalY, double dpiScale)
    {
        if (dpiScale <= 0) return new Point(physicalX, physicalY);
        return new Point(physicalX / dpiScale, physicalY / dpiScale);
    }

    /// <summary>Hit test, both operands in DIP. Rect.Contains is inclusive on left/top and
    /// exclusive on right/bottom, which is the behaviour we want at the screen edge.</summary>
    public static bool IsInsideStrip(Rect stripRect, Point cursorDip) => stripRect.Contains(cursorDip);
}
