using System.Globalization;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using Plith.Services.Shelf;

namespace Plith.DropCatcher.Shelf;

/// <summary>
/// The shelf page's visuals: a grid of stacks, each a column of the files dropped into it, with
/// the shelf's two whole-shelf actions above them.
///
/// A UserControl rather than a Window on purpose: see the header comment in ShelfSurface.xaml.
/// It never touches Plith's own resources, only its own <see cref="Resources"/>, filled in by
/// <see cref="Apply"/>; and it holds no reference to <see cref="ShelfModel"/> as a source of
/// truth, only as the last thing it was asked to paint. The model is a VIEW of Plith's shelf,
/// and this control is a view of the model.
///
/// The grid geometry below (five columns, two rows, a 64 DIP tile, an 8 DIP gap) is what a
/// render showed was needed, not what arithmetic predicted. See the comment beside each
/// constant for the render that set it.
/// </summary>
public partial class ShelfSurface : UserControl
{
    /// <summary>Stacks shown side by side before the row would need to scroll or page. The
    /// arithmetic guess was right: rendered at a no-overflow, five-stack model (ten tiles, two
    /// per column) the last column's icon and full file name still clear the rounded corner at
    /// 384 DIP wide, with none of the four constants changed from the guess.</summary>
    private const int VisibleColumns = 5;

    /// <summary>Entries shown per stack before the rest collapse into a count tile. Confirmed by
    /// the same five-stack render as <see cref="VisibleColumns"/>: two full rows of 64 DIP tiles
    /// sit comfortably under the header with room to spare before the frame's true measured need
    /// (210 DIP, see render-widgets.ps1's shelf-surface section) rather than the 264 DIP the
    /// original arithmetic guessed for the page.</summary>
    private const int VisibleRows = 2;

    /// <summary>Confirmed by the render: a two-line file name and a 22 DIP icon both sit inside
    /// the tile with room to spare, in the seven-entry/two-stack render and in the five-stack,
    /// no-overflow render used to check the column count.</summary>
    private const double TileSize = 64;

    /// <summary>Confirmed by the render: 8 DIP reads as a stack boundary without the row looking
    /// sparse, the same value ShelfWidget settled on for the same purpose.</summary>
    private const double Gap = 8;

    /// <summary>Icon geometry duplicated from Resources/PlithIcons.xaml's IconDocument, not
    /// shared with it: see the header comment in ShelfSurface.xaml for why this project cannot
    /// reach that dictionary.</summary>
    private static readonly Geometry DocumentIcon =
        CreateIcon("M6.5,3.5 L14,3.5 L17.5,7 L17.5,20.5 L6.5,20.5 Z M14,3.5 L14,7 L17.5,7");

    /// <summary>Icon geometry duplicated from Resources/PlithIcons.xaml's IconFolder. Same
    /// caveat as <see cref="DocumentIcon"/>.</summary>
    private static readonly Geometry FolderIcon =
        CreateIcon("M3.5,6.5 L9.5,6.5 L11.5,9 L20.5,9 L20.5,18.5 L3.5,18.5 Z");

    /// <summary>The dictionary Apply last installed, so a second call replaces it instead of
    /// merging on top of it. Without this, a theme change would leave the old brushes reachable
    /// underneath the new ones, invisible until the day something asks for a key both define
    /// and gets whichever happened to merge first.</summary>
    private ResourceDictionary? _paletteResources;

    public ShelfSurface()
    {
        InitializeComponent();
    }

    /// <summary>A tile was pressed: the path, and whether the press was additive (Ctrl held).
    /// The surface does not decide what a press means: ShelfModel.DragPaths and Select do that.
    /// It only reports what happened, on the same instance it was told to paint.</summary>
    public event Action<string, bool>? EntryPressed;

    /// <summary>The "clear the shelf" control was pressed.</summary>
    public event Action? ClearRequested;

    /// <summary>The "start a new stack" control was pressed.</summary>
    public event Action? NewStackRequested;

    /// <summary>
    /// Resolves <paramref name="palette"/> onto this control's OWN Resources, under the keys
    /// Plith uses for the same things. Not Application.Resources: this process has no
    /// application-level theme, and the render harness proved once already, on ShelfWidget, that
    /// a colour reached through Application renders in whatever WPF's defaults are rather than
    /// anything the product ships.
    /// </summary>
    public void Apply(ShelfPalette palette)
    {
        var resources = new ResourceDictionary
        {
            ["NotchInk"] = Solid(palette.Ink),
            ["NotchInkMuted"] = Solid(palette.InkMuted),
            ["NotchTrack"] = Solid(palette.Track),
            ["AccentBrush"] = Solid(palette.Accent),
            // Two stops, not one: OsdSurfaceBrush is a LinearGradientBrush in Plith, and a flat
            // stand-in here would be visibly not the surface the shelf sits on everywhere else.
            ["OsdSurfaceBrush"] = Gradient(palette.SurfaceStart, palette.SurfaceEnd),
        };

        if (_paletteResources is not null) Resources.MergedDictionaries.Remove(_paletteResources);
        Resources.MergedDictionaries.Add(resources);
        _paletteResources = resources;
    }

    /// <summary>
    /// Paints the shelf from <paramref name="model"/>. Call this after <see cref="Apply"/> has
    /// run at least once: every brush below is looked up by key, and a key nothing has defined
    /// yet throws rather than silently drawing in WPF's black-on-black defaults, which is the
    /// failure mode this control exists to make loud instead of invisible.
    /// </summary>
    public void Render(ShelfModel model)
    {
        Columns.Children.Clear();

        var stacks = model.Stacks;
        if (stacks.Count == 0)
        {
            // The page is reachable even when nothing has ever been dropped, so the empty state
            // has to earn its space rather than leave a blank card behind the header.
            Columns.Children.Add(new TextBlock
            {
                Text = "Drop files on the notch to keep them here",
                FontSize = 12,
                Width = VisibleColumns * TileSize + (VisibleColumns - 1) * Gap,
                TextWrapping = TextWrapping.Wrap,
                TextAlignment = TextAlignment.Center,
                Foreground = (Brush)FindResource("NotchInk"),
                Margin = new Thickness(0, 36, 0, 0),
            });
            AutomationProperties.SetName(Columns, "Shelf, empty. Drop files on the notch to keep them here.");
            return;
        }

        var shown = Math.Min(VisibleColumns, stacks.Count);
        for (var i = 0; i < shown; i++)
        {
            if (i > 0) Columns.Children.Add(BuildSeparator());
            Columns.Children.Add(BuildColumn(stacks[i], model.Selection));
        }

        AutomationProperties.SetName(Columns, string.Create(CultureInfo.CurrentCulture,
            $"Shelf, {stacks.Count} stack{(stacks.Count == 1 ? "" : "s")}"));
    }

    private void OnNewStackClick(object sender, RoutedEventArgs e) => NewStackRequested?.Invoke();

    private void OnClearClick(object sender, RoutedEventArgs e) => ClearRequested?.Invoke();

    /// <summary>
    /// A drawn line between stacks, not a Border edge: the brief for this surface calls for the
    /// stack boundary to be drawn geometry the same as the two buttons above it, so a filled
    /// panel is not the only way this product marks a division.
    /// </summary>
    private Path BuildSeparator()
    {
        var height = VisibleRows * TileSize + (VisibleRows - 1) * Gap;
        return new Path
        {
            Data = new LineGeometry(new Point(0, 0), new Point(0, height)),
            Stroke = (Brush)FindResource("NotchTrack"),
            StrokeThickness = 1,
            Width = Gap,
            Height = height,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Top,
        };
    }

    private StackPanel BuildColumn(IReadOnlyList<ShelfEntry> stack, IReadOnlyCollection<string> selection)
    {
        var column = new StackPanel { Width = TileSize, VerticalAlignment = VerticalAlignment.Top };

        // One slot is spent on the count whenever a stack holds more than the column can show,
        // the same rule ShelfWidget uses for the row as a whole: a stack of four shows one file
        // and a "+3" rather than two files and a lie about how many are really there.
        var overflow = Math.Max(0, stack.Count - VisibleRows);
        var shown = overflow > 0 ? VisibleRows - 1 : Math.Min(stack.Count, VisibleRows);

        for (var i = 0; i < shown; i++)
        {
            var last = overflow == 0 && i == shown - 1;
            column.Children.Add(BuildTile(stack[i], selection.Contains(stack[i].Path), last));
        }

        if (overflow > 0) column.Children.Add(BuildOverflowTile(stack.Count - shown));

        return column;
    }

    private Border BuildTile(ShelfEntry entry, bool selected, bool last)
    {
        var icon = new Path
        {
            Data = entry.IsDirectory ? FolderIcon : DocumentIcon,
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
            Text = entry.Name,
            FontSize = 10,
            LineHeight = 12,
            LineStackingStrategy = LineStackingStrategy.BlockLineHeight,
            Margin = new Thickness(0, 4, 0, 0),
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
            TextAlignment = TextAlignment.Center,
            // WrapWithOverflow, not Wrap. ShelfWidget settled on the surrounding shape here (two
            // fixed lines, ellipsis-trimmed) and that shape is kept, but Wrap itself is the wrong
            // half of the choice: it breaks a run with no space in it wherever it has to, so a
            // rendered probe of this exact tile showed "screenshot.p" over "ng" and "invoice-202"
            // over "6-09.xlsx", splitting a filename and a date mid-character. WrapWithOverflow
            // still wraps at real word and hyphen boundaries (confirmed by rendering "Project
            // assets" and "one-more.txt" beside it, unchanged), but lets an unbreakable run
            // overflow its line instead of tearing it apart, where CharacterEllipsis then trims
            // it. Verified by rendering both modes on the same two names side by side.
            TextWrapping = TextWrapping.WrapWithOverflow,
            // A FIXED height, not a maximum: ShelfWidget's tiles found this the hard way. With a
            // maximum, a one-line name makes a shorter stack than a two-line one and neighbouring
            // tiles' icons land at different heights, visible in the render as a row that does
            // not line up.
            Height = 24,
            VerticalAlignment = VerticalAlignment.Top,
            TextTrimming = TextTrimming.CharacterEllipsis,
            MaxWidth = TileSize - 8,
        };

        var content = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(icon);
        content.Children.Add(label);

        var tile = new Border
        {
            Width = TileSize,
            Height = TileSize,
            CornerRadius = new CornerRadius(8),
            Margin = new Thickness(0, 0, 0, last ? 0 : Gap),
            // The selection ring is the accent, at a thickness that reads as a ring rather than
            // a coincidental extra pixel. Nothing else on this surface uses the accent, so
            // there is no other pair for check-contrast.ps1 to measure it against and this one
            // is judged in the render instead, the same way the media transport chip is.
            BorderBrush = selected ? (Brush)FindResource("AccentBrush") : Brushes.Transparent,
            BorderThickness = new Thickness(selected ? 1.5 : 0),
            Padding = new Thickness(4),
            Cursor = Cursors.Hand,
            Child = content,
            ToolTip = entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name,
        };

        AutomationProperties.SetName(tile, entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name);
        tile.PreviewMouseLeftButtonDown += (_, _) =>
            EntryPressed?.Invoke(entry.Path, Keyboard.Modifiers.HasFlag(ModifierKeys.Control));

        return tile;
    }

    /// <summary>
    /// The last slot in a column, when its stack holds more than the column can show. Not
    /// interactive: pressing it would need to mean something for the whole rest of the stack at
    /// once, and ShelfModel has no such operation. Same reason ShelfWidget's own overflow
    /// tile carries no press handler either.
    /// </summary>
    private Border BuildOverflowTile(int hidden)
    {
        var count = new TextBlock
        {
            Text = string.Create(CultureInfo.CurrentCulture, $"+{hidden}"),
            FontSize = 18,
            FontWeight = FontWeights.SemiBold,
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var label = new TextBlock
        {
            Text = "more",
            FontSize = 10,
            Margin = new Thickness(0, 2, 0, 0),
            Foreground = (Brush)FindResource("NotchInk"),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var content = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(count);
        content.Children.Add(label);

        var announced = string.Create(CultureInfo.CurrentCulture,
            $"{hidden} more item{(hidden == 1 ? "" : "s")} in this stack");

        var tile = new Border
        {
            Width = TileSize,
            Height = TileSize,
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(4),
            Child = content,
            ToolTip = announced,
        };
        AutomationProperties.SetName(tile, announced);

        return tile;
    }

    private static SolidColorBrush Solid(Color color)
    {
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        return brush;
    }

    private static LinearGradientBrush Gradient(Color start, Color end)
    {
        var brush = new LinearGradientBrush(start, end, new Point(0, 0), new Point(0, 1));
        brush.Freeze();
        return brush;
    }

    private static Geometry CreateIcon(string data)
    {
        var geometry = Geometry.Parse(data);
        geometry.Freeze();
        return geometry;
    }
}
