using System.Globalization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;
using Plith.Services.Shelf;

namespace Plith.Views.Widgets;

/// <summary>
/// The shelf page: what has been dropped on the notch, newest first.
///
/// Repainted from <see cref="ShelfStore.Changed"/> rather than polled, and only while on screen
/// — the same rule every page here follows, because in notch mode the OSD's window is never
/// hidden and anything left running is a permanent cost on an invisible overlay.
///
/// Every size below came from rendering the page rather than from arithmetic about it. The first
/// version overflowed the frame on both axes at once — five tiles at 64 DIP of pitch in a 316 DIP
/// page, and a 70 DIP tile under a 25 DIP heading in a page 76 DIP tall — and both were obvious
/// in the render and invisible to a green build.
/// </summary>
public partial class ShelfWidget : UserControl
{
    /// <summary>
    /// Slots in the row, INCLUDING the overflow tile. The page is 316 DIP wide once its margins
    /// are taken, and five slots is what fits with a name still readable under the icon.
    /// </summary>
    public const int Slots = 5;

    private const double TileWidth = 58;
    private const double TileGap = 5;

    /// <summary>
    /// Fixed rather than measured, so the row cannot grow past what the frame gives it.
    ///
    /// 68 rather than the 76 it was, and the arithmetic is worth writing down because the first
    /// attempt at it was a DIP and a half out.
    ///
    /// The page is 116 DIP. Its root grid takes 12 off the top and 22 off the bottom, leaving 82.
    /// The hint line under the row measures about 13.3 (a 10 DIP font at its default line
    /// height), and its track is Auto, so the row's star track is about 68.7. At 70 the row
    /// overhung its own track by more than a DIP and drew into the line below it; nothing
    /// clipped, which is exactly why it would have stayed. At 68 it fits.
    ///
    /// The tile's own contents are 52 (a 22 icon, 6 of margin, two 12 DIP lines of name) plus 8
    /// of padding, so 68 still leaves slack rather than clipping.
    /// </summary>
    private const double TileHeight = 68;

    /// <summary>
    /// How long a "the shelf cannot open" sentence stays on the page.
    ///
    /// It has to go away on its own: the notch closes itself and comes back showing whatever was
    /// last rendered, so a sentence left in place would still be there the next time the page is
    /// opened, describing a helper process that has since started perfectly well.
    /// </summary>
    private static readonly TimeSpan UnavailableFor = TimeSpan.FromSeconds(6);

    /// <summary>Kept beside the timer that replaces it, so the sentence and the thing that
    /// restores it cannot drift. Declared in XAML too, which is where it is first shown.</summary>
    private const string OpenHint = "Click to open the shelf";

    private readonly ShelfStore _shelf;
    private DispatcherTimer? _unavailable;
    private bool _pressedHere;

    public ShelfWidget(ShelfStore shelf)
    {
        InitializeComponent();
        _shelf = shelf;

        // Button UP, never down, and this is the hand-over the whole slice turns on.
        //
        // The press and the release go to different processes: the press lands on Plith's
        // window, and by the time the button comes up the notch is on its way down and the
        // catcher is taking its place. Task 8 measured what a press split across two processes
        // costs: DoDragDrop will not deliver a drag for a press that happened somewhere else,
        // and one of the three runs did not return for seventeen seconds. On release there is no
        // press in flight to be split. See docs/superpowers/plans/2026-09-17-shelf-drop-catcher.md.
        //
        // It arrives here at all only because the root carries Background="Transparent". WPF hit
        // tests a panel with no background straight through to whatever is behind it, which here
        // is OsdContent's SlidingRoot: without the brush this handler would fire on the file
        // names and the icons and nowhere else on the page.
        // And only for a release whose PRESS was on this page.
        //
        // WidgetFrame's rail overlays the bottom of the page rather than sitting beside it, and
        // it marks only the button DOWN as handled. A press on the rail that drifts up onto the
        // page before release would otherwise deliver the up here and open the shelf, which is a
        // page change answered by leaving the page.
        //
        // The flag is cleared on the way out as well as on the way in. Without that, a press that
        // began here and ended somewhere else would leave it set, and the next stray release over
        // the page would be taken as a click that never happened.
        PreviewMouseLeftButtonDown += (_, _) => _pressedHere = true;
        MouseLeave += (_, _) => _pressedHere = false;
        MouseLeftButtonUp += (_, _) =>
        {
            if (!_pressedHere) return;
            _pressedHere = false;
            OpenRequested?.Invoke();
        };

        _shelf.Changed += OnShelfChanged;

        // IsVisibleChanged rather than Loaded: the frame keeps pages loaded between opens, so a
        // repaint bound to Loaded would fire once and then never again.
        IsVisibleChanged += (_, e) => { if ((bool)e.NewValue) Render(); };

        Render();
    }

    /// <summary>
    /// A click on the page asked for the shelf.
    ///
    /// The page is a glance and the shelf is the place: five slots cannot show stacking, cannot
    /// be selected from and cannot be dragged out of, so the row's job is to say there is
    /// something here and to be the way to the surface that can do all three.
    /// </summary>
    public event Action? OpenRequested;

    /// <summary>Unsubscribe when the page is discarded. The store outlives every page built from
    /// it, so a page that never let go would be kept alive by the store's event for the life of
    /// the process — and would keep repainting a visual tree nothing is showing.</summary>
    public void Detach()
    {
        _shelf.Changed -= OnShelfChanged;
        _unavailable?.Stop();
    }

    /// <summary>
    /// Say why the shelf did not open, in place of the hint that says it does.
    ///
    /// In the hint's own line rather than over the tiles, because the sentence answers the click
    /// and the click was on the page as a whole. Replacing the row would also throw away the
    /// only thing on screen that is still true.
    /// </summary>
    public void ShowUnavailable(string why)
    {
        Hint.Text = why;
        Hint.Visibility = Visibility.Visible;

        if (_unavailable is null)
        {
            _unavailable = new DispatcherTimer(DispatcherPriority.Normal, Dispatcher) { Interval = UnavailableFor };
            _unavailable.Tick += (_, _) => { _unavailable!.Stop(); Render(); };
        }

        // Stopped before started, so a second click restarts the clock rather than leaving the
        // sentence to disappear on the first click's schedule.
        _unavailable.Stop();
        _unavailable.Start();
    }

    private void OnShelfChanged()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(Render));
            return;
        }

        Render();
    }

    private void Render()
    {
        Tiles.Children.Clear();

        var items = _shelf.Items;

        // Restored here rather than only on the timer, so a shelf that changes while a failure
        // sentence is up goes back to the hint immediately. A drop landing is exactly that case:
        // the catcher that could not be reached a moment ago is plainly reachable now.
        Hint.Text = OpenHint;

        // The hint only where the empty state is not. The empty state already carries the one
        // sentence this page owes a person who has never used it, and two lines of instruction
        // in an 82 DIP page is a page that reads as a form.
        Hint.Visibility = items.Count == 0 ? Visibility.Collapsed : Visibility.Visible;

        if (items.Count == 0)
        {
            // The page is present even when empty, so the empty state has to earn the space. A
            // sentence rather than a blank: this is the only place the product ever says that a
            // file can be dropped on the notch, and nobody discovers that on their own.
            Tiles.Children.Add(new TextBlock
            {
                Text = "Drop files on the notch to keep them here",
                FontSize = 12,
                Width = 300,
                TextWrapping = TextWrapping.Wrap,
                TextAlignment = TextAlignment.Center,
                Foreground = (Brush)FindResource("NotchInk"),
                VerticalAlignment = VerticalAlignment.Center,
                Height = TileHeight,
                // The top padding is what centres two wrapped lines in a fixed-height block;
                // TextBlock has no vertical alignment of its own content. It tracks TileHeight
                // because it is the tile row's height that this stands in for.
                Padding = new Thickness(0, 22, 0, 0),
            });
            AutomationProperties.SetName(Tiles, "Shelf, empty. Drop files on the notch to keep them here.");
            return;
        }

        // One slot is spent on the count whenever there is anything to count, so a shelf of six
        // shows five files and a "+1" rather than six files and a lie.
        var overflow = Math.Max(0, items.Count - Slots);
        var shown = overflow > 0 ? Slots - 1 : Math.Min(items.Count, Slots);

        for (var i = 0; i < shown; i++)
            Tiles.Children.Add(BuildTile(items[i], last: overflow == 0 && i == shown - 1));

        if (overflow > 0) Tiles.Children.Add(BuildOverflowTile(items.Count - shown));

        // Announced on the row, because a StackPanel carries no automation peer of its own and a
        // name set on one would reach nothing. The count first: a screen reader user needs to
        // know how much is here before hearing a list of file names.
        AutomationProperties.SetName(Tiles, string.Create(CultureInfo.CurrentCulture,
            $"Shelf, {items.Count} item{(items.Count == 1 ? "" : "s")}"));
    }

    private Border BuildTile(ShelfItem item, bool last)
    {
        var icon = new System.Windows.Shapes.Path
        {
            Data = (Geometry)FindResource(item.IsDirectory ? "IconFolder" : "IconDocument"),
            Stroke = (Brush)FindResource("NotchInk"),
            StrokeThickness = 1.4,
            StrokeLineJoin = PenLineJoin.Round,
            Stretch = Stretch.Uniform,
            Width = 22,
            Height = 22,
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var label = new TextBlock
        {
            Text = item.Name,
            FontSize = 10,
            LineHeight = 12,
            LineStackingStrategy = LineStackingStrategy.BlockLineHeight,
            Margin = new Thickness(0, 6, 0, 0),
            // Full ink, not the muted one. Muted is calibrated against the panel; these sit on
            // the track tile, which is lighter, and the name is the row's content rather than
            // its chrome.
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
            TextAlignment = TextAlignment.Center,
            // Two lines and then trimmed. One line reduces most real file names to a syllable;
            // three does not fit under the icon in a 116 DIP frame.
            //
            // WrapWithOverflow, not Wrap, and it took a render to see why. Wrap breaks a run with
            // no space in it wherever it has to, so this page showed "invoice-20" over
            // "26-09.xlsx": a filename torn mid-token. WrapWithOverflow still wraps at real word
            // and hyphen boundaries and lets an unbreakable run overflow its line instead, where
            // CharacterEllipsis then trims it.
            //
            // The catcher's ShelfSurface settled this in Task 3 and this page did not follow, so
            // the product had two shelf surfaces breaking names differently, and the one a person
            // meets FIRST was the worse of the two. The empty-state sentence above keeps plain
            // Wrap on purpose: a sentence is words, and words are what Wrap is right for.
            TextWrapping = TextWrapping.WrapWithOverflow,
            // A FIXED height, not a maximum. With a maximum the stack is as tall as its label,
            // so a one-line name makes a shorter tile contents than a two-line one and the two
            // icons sit at different heights — visible in the render as a row that does not line
            // up. Two lines of space, always, whether or not the second is used.
            Height = 24,
            VerticalAlignment = VerticalAlignment.Top,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        return Tile([icon, label], last, item.Path,
                    item.IsDirectory ? $"Folder {item.Name}" : item.Name);
    }

    /// <summary>
    /// The last slot, when the shelf holds more than the row can show.
    ///
    /// A tile rather than a corner label, so the row itself says there is more of it. It is also
    /// the one place the shelf's cap becomes visible: the store keeps twenty and this is what
    /// admits to the other fifteen.
    /// </summary>
    private Border BuildOverflowTile(int hidden)
    {
        var count = new TextBlock
        {
            Text = string.Create(CultureInfo.CurrentCulture, $"+{hidden}"),
            FontSize = 20,
            FontWeight = FontWeights.SemiBold,
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var label = new TextBlock
        {
            Text = "more",
            FontSize = 10,
            Margin = new Thickness(0, 4, 0, 0),
            // Full ink, like the file names beside it and for the same reason: the muted brush is
            // calibrated against the panel, and on the lighter track tile it was the faintest
            // thing on the page in both themes.
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var announced = string.Create(CultureInfo.CurrentCulture, $"{hidden} more item{(hidden == 1 ? "" : "s")}");
        return Tile([count, label], last: true, tooltip: announced, announced: announced);
    }

    /// <summary>
    /// Layout only. It resolved a brush until the tiles lost their filled chip, and now touches
    /// no palette at all — every colour is resolved by the callers, on the instance, because the
    /// render harness puts the palette on the HOST element rather than on Application.Resources
    /// and anything resolving through Application would silently render in colours the product
    /// never shows.
    /// </summary>
    private static Border Tile(IEnumerable<UIElement> children, bool last, string tooltip, string announced)
    {
        // Top, not centre: every tile's contents are now the same height, so centring would only
        // reintroduce a dependency on content that the fixed label height just removed.
        var stack = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        foreach (var child in children) stack.Children.Add(child);

        var tile = new Border
        {
            Width = TileWidth,
            Height = TileHeight,
            // No gap after the last one, which is where the first version lost its fifth tile off
            // the right edge: five tiles at 58 plus five gaps of 6 is 320 in a 316 DIP page.
            Margin = new Thickness(0, 0, last ? 0 : TileGap, 0),
            // No filled chip behind the tile, and the contrast check is why. NotchTrack is built
            // by ContrastInk.TrackOn, which targets 3:1 — it is a non-text surface by
            // construction, for grooves and thumbnails. NotchInk is derived against the PANEL,
            // and on a track tile it measured 4.0:1 with a white accent, under the 4.5 body text
            // needs. Separation comes from the spacing instead, which is the notch's own idiom.
            Padding = new Thickness(4),
            Child = stack,
            ToolTip = tooltip,
        };

        AutomationProperties.SetName(tile, announced);
        return tile;
    }
}
