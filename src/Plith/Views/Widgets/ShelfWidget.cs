using System.Globalization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
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
    /// Fixed rather than measured, so the row cannot grow past what the frame gives it. The page
    /// has 80 DIP; this is what a tile needs and leaves the difference as slack rather than as
    /// clipping.
    /// </summary>
    private const double TileHeight = 76;

    private readonly ShelfStore _shelf;

    public ShelfWidget(ShelfStore shelf)
    {
        InitializeComponent();
        _shelf = shelf;

        _shelf.Changed += OnShelfChanged;

        // IsVisibleChanged rather than Loaded: the frame keeps pages loaded between opens, so a
        // repaint bound to Loaded would fire once and then never again.
        IsVisibleChanged += (_, e) => { if ((bool)e.NewValue) Render(); };

        Render();
    }

    /// <summary>Unsubscribe when the page is discarded. The store outlives every page built from
    /// it, so a page that never let go would be kept alive by the store's event for the life of
    /// the process — and would keep repainting a visual tree nothing is showing.</summary>
    public void Detach() => _shelf.Changed -= OnShelfChanged;

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
        AutomationProperties.SetName(Tiles, items.Count == 0
            ? "Shelf, empty"
            : string.Create(CultureInfo.CurrentCulture,
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
            TextWrapping = TextWrapping.Wrap,
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
    /// An instance method, not a static one, and deliberately: the palette reaches these controls
    /// through the element tree, and the render harness puts it on the HOST element rather than
    /// on Application.Resources. A static version resolving through Application would find
    /// nothing there and fall back to a default — rendering a page in colours the product never
    /// shows, which is the exact failure the harness exists to prevent.
    /// </summary>
    private Border Tile(IEnumerable<UIElement> children, bool last, string tooltip, string announced)
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
