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
    /// The empty page's glyph: an arrow coming down onto a line, the same one the shelf surface
    /// uses. A dashed box with only a sentence in it reads as a field to type in; an arrow
    /// landing on a surface says the one thing the sentence has to spell out.
    ///
    /// INLINE here, beside the text, where the shelf surface stacks it above. The arrangement
    /// differs because the proportions do: this box is 300 by 68 and its height is fixed on
    /// purpose, so that an empty page occupies exactly what a full one does and the frame does
    /// not resize between them. A stacked glyph does not fit in 68 and buying the room would
    /// mean moving a frame shared by every widget page.
    /// </summary>
    private static readonly Geometry DropHereIcon =
        Geometry.Parse("M12,4 L12,14 M8,10.5 L12,14.5 L16,10.5 M5,18.5 L19,18.5");

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
        // The tooltip on the tile this is about to destroy, closed FIRST.
        //
        // The same defect as ShelfSurface's, in the same shape, in the other process: a tooltip
        // is popup content rather than a child of the tile, so clearing the row below leaves an
        // open one on screen with a file name in it, for WPF's five-second ShowDuration. This
        // page repaints on every store change, so a drop or a remove is enough to reach it while
        // the pointer rests on a tile - and this page is hovered by design, since a click on it
        // is what opens the shelf.
        //
        // Fixed here as well as there rather than waiting for a report, because the reported one
        // cost an afternoon of measurement in the surface that happened to be looked at.
        //
        // The tiles are asked directly rather than an open tooltip being tracked as it opens: a
        // tooltip opened by anything that does not raise ToolTipOpening would be open and
        // untracked, which a probe caught on the first try in the other surface.
        foreach (var child in Tiles.Children)
        {
            if (child is FrameworkElement { ToolTip: ToolTip { IsOpen: true } open }) open.IsOpen = false;
        }

        Tiles.Children.Clear();

        // Both hosts reset on every render, so the empty state cannot outlive the empty shelf.
        // Setting them only inside the empty branch made the transition one-way: a file arriving
        // on an empty shelf would have left the dashed box up and the tiles hidden behind it.
        EmptyHost.Visibility = Visibility.Collapsed;
        FilledBlock.Visibility = Visibility.Visible;

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
            //
            // And a SHAPE around the sentence, not the sentence alone. Told on hardware that the
            // empty page reads as bare, and the reason it does is that it instructs without
            // showing: "drop files here" with no "here" drawn anywhere. A dashed outline is the
            // here. It is not a drop target of its own and does not need to be, since the drop
            // lands on the notch itself and never on this page.
            var message = new TextBlock
            {
                Text = "Drop files on the notch to keep them here",
                FontSize = 12,
                TextWrapping = TextWrapping.Wrap,
                TextAlignment = TextAlignment.Center,
                Foreground = (Brush)FindResource("NotchInkMuted"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                MaxWidth = 250,
            };

            var outline = new System.Windows.Shapes.Rectangle
            {
                RadiusX = 10,
                RadiusY = 10,
                Stroke = (Brush)FindResource("NotchTrack"),
                StrokeThickness = 1,
                StrokeDashArray = [4, 4],
                Fill = Brushes.Transparent,
            };

            var empty = new Grid
            {
                // Stretched, not sized. An empty shelf page IS a drop target, so the box that
                // says so should occupy the page rather than float in the middle of it, and
                // stretching means it follows the frame instead of carrying a number derived
                // from one version of it. See EmptyHost in the XAML.
                HorizontalAlignment = HorizontalAlignment.Stretch,
                VerticalAlignment = VerticalAlignment.Stretch,
                
            };
            // The glyph and the line as one centred row.
            var glyph = new System.Windows.Shapes.Path
            {
                Data = DropHereIcon,
                Width = 20,
                Height = 20,
                Stretch = Stretch.Uniform,
                Stroke = (Brush)FindResource("NotchInkMuted"),
                StrokeThickness = 1.4,
                StrokeStartLineCap = PenLineCap.Round,
                StrokeEndLineCap = PenLineCap.Round,
                StrokeLineJoin = PenLineJoin.Round,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, 10, 0),
            };

            var row = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            };
            row.Children.Add(glyph);
            row.Children.Add(message);

            empty.Children.Add(outline);
            empty.Children.Add(row);

            // Into its own host rather than into the tile row: a horizontal StackPanel gives its
            // children their desired size and never stretches them, so a box added there could
            // only ever be a fixed height.
            EmptyHost.Children.Clear();
            EmptyHost.Children.Add(empty);
            EmptyHost.Visibility = Visibility.Visible;
            FilledBlock.Visibility = Visibility.Collapsed;

            // On THIS control, not on Tiles. See the populated branch below for the whole of it.
            AutomationProperties.SetName(this, "Shelf, empty. Drop files on the notch to keep them here.");
            return;
        }

        // One slot is spent on the count whenever there is anything to count, so a shelf of six
        // shows five files and a "+1" rather than six files and a lie.
        var overflow = Math.Max(0, items.Count - Slots);
        var shown = overflow > 0 ? Slots - 1 : Math.Min(items.Count, Slots);

        for (var i = 0; i < shown; i++)
            Tiles.Children.Add(BuildTile(items[i], last: overflow == 0 && i == shown - 1));

        if (overflow > 0) Tiles.Children.Add(BuildOverflowTile(items.Count - shown));

        // ON THIS CONTROL, NOT ON Tiles, and the comment this replaces is the best evidence of
        // why that matters: it read "announced on the row, because a StackPanel carries no
        // automation peer of its own and a name set on one would reach nothing", and then set the
        // name on Tiles, which IS a StackPanel. The reason was written down correctly and then
        // contradicted by the line under it.
        //
        // Confirmed against the LIVE UIA tree on 2026-09-19, not inferred: with five files on the
        // shelf page the names present were the five file names, their type chips and the open
        // hint. "Shelf, 5 items" appeared nowhere, so no screen reader had ever heard the count.
        // Filed then as a known gap in docs/SHELF-VERIFICATION.md section 5.4; fixed here.
        //
        // The UserControl root has a peer, which is the same one-level-up fix ShelfSurface,
        // AmbientCardView, AudioCardView and MediaCardView all use for the same reason.
        //
        // The count first: a screen reader user needs to know how much is here before hearing a
        // list of file names.
        AutomationProperties.SetName(this, string.Create(CultureInfo.CurrentCulture,
            $"Shelf, {items.Count} item{(items.Count == 1 ? "" : "s")}"));
    }

    private Border BuildTile(ShelfItem item, bool last)
    {
        var glyph = new System.Windows.Shapes.Path
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

        // The icon host, so the extension can sit inside the page of the document glyph.
        //
        // Why this page does not simply show the shell's own icon, the way the catcher's surface
        // does: extracting one loads the icon handler the extension is registered to, which is
        // third-party code chosen by the shell. Plith runs UIAccess-signed at High integrity and
        // is precisely the process that must not load an arbitrary shell extension. ShellIcons'
        // own header states this, and it is the reason the two surfaces differ at all.
        //
        // So the difference is closed the other way. Four identical document outlines told a
        // person nothing about which file was which, and the names beside them are trimmed
        // ("invoice-2...") exactly when the extension would have been most useful. Drawing the
        // extension inside the glyph recovers the information the trim took away, and it does it
        // with the real type rather than a category guessed from it.
        var icon = new Grid { Width = 22, Height = 22, HorizontalAlignment = HorizontalAlignment.Center };
        icon.Children.Add(glyph);

        if (!item.IsDirectory && ExtensionTag(item.Name) is { } tag)
        {
            var text = new TextBlock
            {
                Text = tag,
                // Muted, so the tag reads as a property of the icon rather than as a second
                // label competing with the file name under it.
                FontSize = 6.5,
                FontWeight = FontWeights.SemiBold,
                Foreground = (Brush)FindResource("NotchInkMuted"),
            };

            // A Viewbox rather than a font size chosen per tag length, because the first attempt
            // WAS a fixed size and the render showed "XLSX" hanging outside the document outline
            // on both sides. DownOnly means a three-character tag keeps its natural 6.5 and only
            // a tag too wide for the page shrinks; the alternative was a table of sizes by length,
            // which is the same magic number written four times.
            icon.Children.Add(new Viewbox
            {
                Child = text,
                // 10.5, and the number is measured rather than chosen. The document glyph is
                // drawn Uniform into 22 x 22 and a document is taller than it is wide, so its
                // body comes out about 15 DIP across; two 1.4 DIP strokes leave about 12 inside,
                // and a tag wants a little air on each side of that. 13 was the first value and a
                // magnified crop of the render showed "XLSX" cut by the outline on both edges.
                Width = 10.5,
                Stretch = Stretch.Uniform,
                StretchDirection = StretchDirection.DownOnly,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Bottom,
                Margin = new Thickness(0, 0, 0, 3.5),
            });
        }

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
    /// <summary>
    /// The count of what the row is not showing, weighted as a count rather than as a file.
    ///
    /// It was "+4" at 20 point over the word "more", both in full ink, inside a tile the same
    /// size as the ones holding real files. Looked at on hardware for the first time, the eye
    /// takes that for a fifth file. It is the opposite: the statement that files exist which this
    /// row does NOT draw.
    ///
    /// One muted line now, and the SLOT keeps its full width so the four tiles beside it do not
    /// move. The full sentence stays in the tooltip and in the automation name, so nothing is
    /// lost to a screen reader by the word "more" leaving the screen.
    ///
    /// The old comment here argued for full ink because the muted brush was the faintest thing on
    /// a lighter TRACK tile. That tile is gone (see Tile, which no longer fills a chip), so the
    /// muted brush now sits on the panel it was calibrated against.
    /// </summary>
    private Border BuildOverflowTile(int hidden)
    {
        var count = new TextBlock
        {
            Text = string.Create(CultureInfo.CurrentCulture, $"+{hidden}"),
            FontSize = 13,
            FontWeight = FontWeights.SemiBold,
            Foreground = (Brush)FindResource("NotchInkMuted"),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var announced = string.Create(CultureInfo.CurrentCulture, $"{hidden} more item{(hidden == 1 ? "" : "s")}");
        return Tile([count], last: true, tooltip: announced, announced: announced);
    }

    /// <summary>
    /// The extension to draw inside a document glyph, or null when there is nothing worth
    /// drawing.
    ///
    /// Null rather than an empty string for three cases that all mean "no useful tag": a name
    /// with no dot at all, a dotfile whose whole name is its suffix (".gitignore" has no
    /// extension, it has a name), and anything longer than four characters, which would not fit
    /// inside the glyph and would be trimmed into a lie.
    /// </summary>
    private static string? ExtensionTag(string name)
    {
        // Fully qualified: this file draws with System.Windows.Shapes.Path, so a bare `Path`
        // here would mean the shape, not the path helper.
        var extension = System.IO.Path.GetExtension(name);
        if (extension.Length is < 2 or > 5) return null;                 // ".a" is 2, ".webp" is 5
        if (System.IO.Path.GetFileNameWithoutExtension(name).Length == 0) return null;

        return extension[1..].ToUpperInvariant();
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
        };

        // An explicit ToolTip rather than the string this used to be: a string tooltip is wrapped
        // by WPF in an object it hands nobody, so it cannot be closed once its tile is gone. See
        // Render.
        tile.ToolTip = new ToolTip { Content = tooltip };

        AutomationProperties.SetName(tile, announced);
        return tile;
    }
}
