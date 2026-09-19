using System.Globalization;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Media3D;
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

    /// <summary>The stack caption's own reserved height (its FontSize plus the margin under it),
    /// FIXED rather than left to measure its own content: BuildSeparator needs the exact same
    /// number to draw a line that spans the caption as well as the tiles under it, and a value
    /// only WPF's layout pass knows would leave the two free to disagree.</summary>
    private const double CaptionHeight = 13;

    /// <summary>Icon geometry duplicated from Resources/PlithIcons.xaml's IconDocument, not
    /// shared with it: see the header comment in ShelfSurface.xaml for why this project cannot
    /// reach that dictionary.</summary>
    private static readonly Geometry DocumentIcon =
        CreateIcon("M6.5,3.5 L14,3.5 L17.5,7 L17.5,20.5 L6.5,20.5 Z M14,3.5 L14,7 L17.5,7");

    /// <summary>Icon geometry duplicated from Resources/PlithIcons.xaml's IconFolder. Same
    /// caveat as <see cref="DocumentIcon"/>.</summary>
    private static readonly Geometry FolderIcon =
        CreateIcon("M3.5,6.5 L9.5,6.5 L11.5,9 L20.5,9 L20.5,18.5 L3.5,18.5 Z");

    /// <summary>Same coordinates as ClearButton's icon in ShelfSurface.xaml, duplicated for the
    /// per-tile remove affordance rather than shared: it is a different Path in a different part
    /// of the tree, built in code rather than declared, and this project keeps every icon shape
    /// as its own literal geometry for the reason the file header comment gives.</summary>
    private static readonly Geometry RemoveIcon = CreateIcon("M6,6 L18,18 M18,6 L6,18");

    /// <summary>
    /// The private clipboard format a tile drag carries its paths under, ALONGSIDE the ordinary
    /// <see cref="DataFormats.FileDrop"/> the same drag now also offers.
    ///
    /// It no longer means "this drag never leaves the window", because since Task 8 it can: one
    /// press and one <c>DoDragDrop</c> serve both a restack inside the shelf and a drag out into
    /// another application. What it means now is "this drag came from the shelf itself", which is
    /// what lets this control's own drop target tell a tile being restacked apart from a file
    /// arriving from Explorer. The destination is not known until the release, so there is no
    /// reliable signal to branch on at the source; both formats go out and each target's own
    /// DragOver decides which one it wants.
    ///
    /// Internal rather than private because <see cref="ShelfWindow"/> builds the single
    /// <see cref="DataObject"/> that carries both formats. See StartDrag there for why there is
    /// exactly one call and why it lives in that file.
    /// </summary>
    internal const string ShelfDragFormat = "Plith.Shelf.Paths";

    /// <summary>The dictionary Apply last installed, so a second call replaces it instead of
    /// merging on top of it. Without this, a theme change would leave the old brushes reachable
    /// underneath the new ones, invisible until the day something asks for a key both define
    /// and gets whichever happened to merge first.</summary>
    private ResourceDictionary? _paletteResources;

    /// <summary>The model Render last painted from. Not a source of truth (see the header
    /// comment), only kept so a press, a drag or a drop can ask it what a remove or a restack
    /// should act on: ShelfModel.DragPaths needs the current selection, and that lives here,
    /// not in anything the tile or the drop target itself remembers.</summary>
    private ShelfModel? _lastModel;

    /// <summary>The context menu a tile currently has open, or null. Tracked here rather than
    /// left to be inferred from ContextMenuOpening/Closing bubbling up from whatever tile owns
    /// it: Render tears every tile out of Columns and rebuilds them from scratch, and a bubbling
    /// routed event has nowhere to bubble THROUGH once its source has been removed from the
    /// tree. A menu open on a tile that a later Render destroys would then never raise anything
    /// this control could hear at all, which is exactly the bug this field exists to prevent -
    /// see Render's own comment for the other half of the fix.</summary>
    private ContextMenu? _openMenu;

    /// <summary>
    /// The column and row keyboard navigation is currently on, or null before the first arrow
    /// key press (and before any mouse press, which sets these too - see BuildTile). Indices
    /// rather than a remembered path, because what is at a given position can change under a
    /// live shelf (a remove closes a gap, a fresh SetStack can reorder entries) in a way an
    /// index survives and a captured path would not: every use re-resolves these against the
    /// CURRENT model through <see cref="ResolveFocus"/> rather than trusting what was true when
    /// they were set.
    /// </summary>
    private int? _focusColumn;

    /// <inheritdoc cref="_focusColumn"/>
    private int? _focusRow;

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

    /// <summary>Take these paths off the shelf: the tile's own selection, or the whole current
    /// selection if the tile removed belongs to it. Raised by the hover affordance and by the
    /// tile's context menu, both computing the same set from ShelfModel.DragPaths.</summary>
    public event Action<IReadOnlyList<string>>? RemoveRequested;

    /// <summary>A tile drag landed on stack <c>index</c>, carrying <c>paths</c>. An index equal
    /// to the current stack count means the drop missed every column, which is how landing past
    /// the last stack (or on an empty shelf) asks for a new one instead.</summary>
    public event Action<int, IReadOnlyList<string>>? RestackRequested;

    /// <summary>Open this file, from its context menu.</summary>
    public event Action<string>? OpenRequested;

    /// <summary>Show this file in the file manager, from its context menu.</summary>
    public event Action<string>? RevealRequested;

    /// <summary>
    /// A press on a tile has moved further than the system's drag threshold: the element the
    /// press landed on, and the paths that press drags.
    ///
    /// Raised SYNCHRONOUSLY from inside that tile's own mouse-move handler, and from nowhere
    /// else, which is the whole of its contract. The drag itself is started by
    /// <see cref="ShelfWindow"/>, which refuses any drag whose source element does not belong to
    /// its own window: <c>DoDragDrop</c> has been measured to fail, and once to hang outright,
    /// for a press it did not receive. Anything that raised this from a timer or a pipe message
    /// would be refused there rather than obeyed here.
    /// </summary>
    public event Action<DependencyObject, IReadOnlyList<string>>? DragOutRequested;

    /// <summary>Whether a tile's context menu is open, changed. True right after one opens,
    /// false right after one closes - including a close Render forces because the tile that
    /// owned the menu is about to be torn down. ShelfWindow reads this to know whether a menu
    /// is suspending its own dismissal; see Render's comment for why the flag cannot be derived
    /// from ContextMenuOpening/Closing instead.</summary>
    public event Action<bool>? MenuOpenChanged;

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
            // NOT the raw accent. Drawn as Accent, the selection ring measured 1.25:1 against the
            // panel for a near-white accent, where this product holds a non-text surface to 3:1.
            // Plith derives this with ContrastInk.RingOn, which keeps the accent unchanged when
            // it already clears 3:1 and only walks its lightness when it does not - unlike
            // ContrastInk.TrackOn, which NotchTrack uses and which ignores the accent's hue
            // entirely, flattening an already-good accent along with a broken one. Plith sends
            // the answer rather than the accent alone, so the ring cannot drift from the rest of
            // the product's contrast-derived colours.
            ["SelectionRing"] = Solid(palette.SelectionRing),
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
    /// <summary>
    /// A key that reached the shelf, forwarded here by <see cref="ShelfWindow"/> because the
    /// window, not this control, is what holds keyboard focus - see that class's own header
    /// comment on why the shelf must take activation at all. Returns whether the key meant
    /// something here, so a key this surface does not use is left for the window's own handling
    /// (today, only Escape) rather than silently swallowed.
    ///
    /// Arrow keys move the selection one tile at a time (the visible grid only: an entry folded
    /// into an overflow tile has no key that reaches it, the same limit the mouse has on it).
    /// Space adds or removes the current tile from the selection without moving it. Enter opens
    /// it. Delete removes the current selection, or just the current tile if nothing is
    /// selected. A surface that takes focus and then answers no key is worse than one that never
    /// took focus, which is the whole reason this exists.
    /// </summary>
    public bool HandleKey(Key key)
    {
        if (_lastModel is not { } model) return false;
        var stacks = model.Stacks;
        if (stacks.Count == 0) return false;

        return key switch
        {
            Key.Left => Move(model, stacks, columnDelta: -1, rowDelta: 0),
            Key.Right => Move(model, stacks, columnDelta: 1, rowDelta: 0),
            Key.Up => Move(model, stacks, columnDelta: 0, rowDelta: -1),
            Key.Down => Move(model, stacks, columnDelta: 0, rowDelta: 1),
            Key.Space => ToggleFocused(model, stacks),
            Key.Enter => OpenFocused(stacks),
            Key.Delete => RemoveFocused(model, stacks),
            _ => false,
        };
    }

    /// <summary>
    /// The tile keyboard input currently acts on: the last one an arrow key landed on (or a
    /// mouse press set - see BuildTile), clamped against what the shelf holds now, or the
    /// shelf's very first tile before anything has set a position at all. The clamped result is
    /// committed back to the fields, so a Space, Enter or Delete pressed before the first arrow
    /// key acts on the first tile AND leaves the next arrow key moving on from there, rather than
    /// from a position nothing was ever actually on.
    /// </summary>
    private (int Column, int Row, string Path)? ResolveFocus(IReadOnlyList<IReadOnlyList<ShelfEntry>> stacks)
    {
        var columns = Math.Min(VisibleColumns, stacks.Count);
        if (columns == 0) return null;

        var column = Math.Clamp(_focusColumn ?? 0, 0, columns - 1);
        var rows = VisibleRowsShown(stacks[column].Count);
        if (rows == 0) return null;
        var row = Math.Clamp(_focusRow ?? 0, 0, rows - 1);

        _focusColumn = column;
        _focusRow = row;
        return (column, row, stacks[column][row].Path);
    }

    private bool Move(ShelfModel model, IReadOnlyList<IReadOnlyList<ShelfEntry>> stacks, int columnDelta, int rowDelta)
    {
        // Whether anything has been on this grid before matters here and only here: the very
        // first arrow key press has to land ON the resolved default rather than move AWAY from
        // it, the same first-press behaviour any keyboard list gives a shelf nobody has
        // navigated yet. ResolveFocus itself cannot tell the two cases apart once it returns,
        // because it commits the default the moment it resolves one.
        var firstPress = _focusColumn is null;
        if (ResolveFocus(stacks) is not { } current) return false;

        if (!firstPress)
        {
            var columns = Math.Min(VisibleColumns, stacks.Count);
            var column = Math.Clamp(current.Column + columnDelta, 0, columns - 1);
            var rows = VisibleRowsShown(stacks[column].Count);
            var row = rows == 0 ? 0 : Math.Clamp(current.Row + rowDelta, 0, rows - 1);
            _focusColumn = column;
            _focusRow = row;
            current = (column, row, stacks[column][row].Path);
        }

        model.Select(current.Path, additive: false);
        Render(model);
        return true;
    }

    private bool ToggleFocused(ShelfModel model, IReadOnlyList<IReadOnlyList<ShelfEntry>> stacks)
    {
        if (ResolveFocus(stacks) is not { } current) return false;

        model.Select(current.Path, additive: true);
        Render(model);
        return true;
    }

    private bool OpenFocused(IReadOnlyList<IReadOnlyList<ShelfEntry>> stacks)
    {
        if (ResolveFocus(stacks) is not { } current) return false;

        OpenRequested?.Invoke(current.Path);
        return true;
    }

    private bool RemoveFocused(ShelfModel model, IReadOnlyList<IReadOnlyList<ShelfEntry>> stacks)
    {
        if (ResolveFocus(stacks) is not { } current) return false;

        IReadOnlyList<string> toRemove = model.Selection.Count > 0 ? [.. model.Selection] : [current.Path];
        RemoveRequested?.Invoke(toRemove);
        return true;
    }

    /// <summary>Entries a column actually shows as tiles, before the rest fold into a count
    /// tile - the same arithmetic BuildColumn uses to build them, factored out so keyboard
    /// navigation and construction cannot silently disagree about which rows are reachable.
    /// </summary>
    private static int VisibleRowsShown(int stackCount) =>
        stackCount > VisibleRows ? VisibleRows - 1 : Math.Min(stackCount, VisibleRows);

    public void Render(ShelfModel model)
    {
        // Recorded before anything else: a tile built below captures this in its own press and
        // drop handlers, and a handler built from a stale model would compute a remove or a
        // restack against a selection that is no longer the one on screen.
        _lastModel = model;

        // A menu opened on a PREVIOUS render's tile is about to have that tile torn out from
        // under it by the Children.Clear() below. Closed here, first: the menu is popup content,
        // not a descendant of the tile that opened it, so setting IsOpen false works and raises
        // Closed (see BuildTileMenu) whether or not that tile still exists by the time it does.
        // Measured: Closed does NOT fire synchronously with this assignment (the default
        // ContextMenu style animates its close), so _openMenu and MenuOpenChanged lag this line
        // by roughly one dispatcher tick, not zero. That is harmless here - everything downstream
        // (ShelfWindow's Dismiss) already tolerates _menuOpen being stale for a moment, because
        // it retries rather than deciding once - but it is exactly the kind of timing detail that
        // looks synchronous until measured, so it is written down rather than assumed.
        //
        // Measured the alternative too: leaving this out means a right-click on tile A followed
        // by ANY refresh (Plith re-sends the whole Items set after every mutating verb, so a
        // hover-remove on tile B is enough) tears tile A down mid-open, the menu's Closed then
        // has no live tile to have bubbled through even if THAT were what was being listened to,
        // and ShelfWindow's _menuOpen stays true forever, which means the shelf can never be
        // dismissed again.
        if (_openMenu is { IsOpen: true } openMenu) openMenu.IsOpen = false;

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

            // Nothing left for keyboard navigation to be on. Left set, the next non-empty
            // Render would resolve a stale index against an entirely unrelated delivery -
            // ResolveFocus clamps the NUMBERS back into range, but a clamped index still points
            // at whatever now happens to occupy that slot, not at anything the person actually
            // navigated to.
            _focusColumn = null;
            _focusRow = null;
            return;
        }

        var shown = Math.Min(VisibleColumns, stacks.Count);
        for (var i = 0; i < shown; i++)
        {
            if (i > 0) Columns.Children.Add(BuildSeparator());
            Columns.Children.Add(BuildColumn(i, stacks[i], model.Selection));
        }

        AutomationProperties.SetName(Columns, string.Create(CultureInfo.CurrentCulture,
            $"Shelf, {stacks.Count} stack{(stacks.Count == 1 ? "" : "s")}"));
    }

    private void OnNewStackClick(object sender, RoutedEventArgs e) => NewStackRequested?.Invoke();

    // Asks nothing first, on purpose: the shelf holds references, not the files themselves, so
    // clearing it deletes nothing on disk, and a confirmation dialog for a reversible action on
    // a surface this small is friction rather than safety. If a future change makes Clear do
    // something that is NOT trivially reversible, this is the line that stops being true.
    private void OnClearClick(object sender, RoutedEventArgs e) => ClearRequested?.Invoke();

    /// <summary>
    /// Copy, not Move, and the reason is at the source rather than here. The one
    /// <c>DoDragDrop</c> behind every tile drag offers <c>Copy | Link</c> and deliberately never
    /// Move, because the same gesture can end over Explorer, where Move means "delete the
    /// original once you have it". A target cannot ask for an effect the source never offered,
    /// so Move is not available to this drop target either, whatever it would have preferred.
    ///
    /// Copy rather than Link of the two that remain, so that the cursor does not change meaning
    /// when the pointer crosses the shelf's own edge in the middle of a drag: one gesture, one
    /// badge, wherever it happens to be hovering. Nothing about a restack reads the effect
    /// anyway - <see cref="OnColumnsDrop"/> raises <see cref="RestackRequested"/> on the paths,
    /// and the files on disk are not touched by either side of it.
    /// </summary>
    private void OnColumnsDragOver(object sender, DragEventArgs e)
    {
        e.Effects = e.Data.GetDataPresent(ShelfDragFormat) ? DragDropEffects.Copy : DragDropEffects.None;
        e.Handled = true;
    }

    private void OnColumnsDrop(object sender, DragEventArgs e)
    {
        if (e.Data.GetData(ShelfDragFormat) is not IReadOnlyList<string> paths) return;

        RestackRequested?.Invoke(TargetStackIndex(e.GetPosition(Columns)), paths);
        e.Handled = true;
    }

    /// <summary>
    /// Which stack a drop at <paramref name="position"/> belongs to, found by which built
    /// column's own bounds the point falls inside. A point that matches no column, including
    /// one past the last one or on a shelf with none at all, resolves to the current stack
    /// count: exactly the index Restack (and ShelfStore.Restack behind it) treats as "make a new
    /// one", which is what the brief calls dropping on the empty area past the last stack.
    /// </summary>
    private int TargetStackIndex(Point position)
    {
        // Border, not StackPanel: BuildColumn wraps its StackPanel in a Border so the whole
        // column has an automation peer to announce its name on (see that method's own comment),
        // and the Tag that used to live on the StackPanel now lives on the Border that replaced
        // it as this method's unit of "a column".
        foreach (var column in Columns.Children.OfType<Border>())
        {
            if (column.Tag is not int index) continue;

            var topLeft = column.TranslatePoint(new Point(0, 0), Columns);
            var bounds = new Rect(topLeft, column.RenderSize);
            if (position.X >= bounds.Left && position.X < bounds.Right) return index;
        }

        return _lastModel?.Stacks.Count ?? 0;
    }

    /// <summary>
    /// A drawn line between stacks, not a Border edge: the brief for this surface calls for the
    /// stack boundary to be drawn geometry the same as the two buttons above it, so a filled
    /// panel is not the only way this product marks a division.
    /// </summary>
    private Path BuildSeparator()
    {
        // Starts at the top of the CAPTION, not the top of the tiles, and is therefore taller by
        // CaptionHeight than the tile grid alone: BuildColumn's caption sits above the tiles, and
        // a separator that only spanned the tiles would visibly stop short of the column's own
        // top edge.
        var height = CaptionHeight + VisibleRows * TileSize + (VisibleRows - 1) * Gap;
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

    /// <summary>
    /// One stack, as a column of tiles under a small caption naming it, wrapped in a Border.
    ///
    /// The Border, not the StackPanel inside it, is what carries both the drop-target Tag
    /// TargetStackIndex reads back and the stack's own AutomationProperties.Name: a StackPanel
    /// gets no automation peer of its own, so a name set directly on one reaches nothing at all -
    /// the same defect class this slice's brief calls out, and this control's own version of the
    /// bug ShelfWidget's row already had to be fixed once. The Border adds no visible chrome of
    /// its own (no Background, no BorderBrush), so it changes nothing on screen; it exists only
    /// to have a peer.
    ///
    /// The caption text is new, in NotchInkMuted rather than NotchInk: it names the stack rather
    /// than being its content, the same distinction ShelfWidget's own hint line draws against its
    /// tile row. It is also what makes check-contrast.ps1's new
    /// OsdSurfaceBrush/NotchInkMuted pair for this file a real measurement of shipped code rather
    /// than a check written against a colour nothing draws.
    /// </summary>
    private Border BuildColumn(int index, IReadOnlyList<ShelfEntry> stack, IReadOnlyCollection<string> selection)
    {
        var column = new StackPanel { Width = TileSize, VerticalAlignment = VerticalAlignment.Top };

        var caption = new TextBlock
        {
            Text = string.Create(CultureInfo.CurrentCulture, $"Stack {index + 1}"),
            FontSize = 9,
            Height = CaptionHeight - 2,
            Foreground = (Brush)FindResource("NotchInkMuted"),
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 0, 0, 2),
        };
        column.Children.Add(caption);

        // One slot is spent on the count whenever a stack holds more than the column can show,
        // the same rule ShelfWidget uses for the row as a whole: a stack of four shows one file
        // and a "+3" rather than two files and a lie about how many are really there.
        var overflow = Math.Max(0, stack.Count - VisibleRows);
        var shown = VisibleRowsShown(stack.Count);

        for (var i = 0; i < shown; i++)
        {
            var last = overflow == 0 && i == shown - 1;
            column.Children.Add(BuildTile(stack[i], selection.Contains(stack[i].Path), last, index, i));
        }

        if (overflow > 0) column.Children.Add(BuildOverflowTile(stack.Count - shown));

        var wrapper = new Border { Tag = index, Child = column };
        AutomationProperties.SetName(wrapper, string.Create(CultureInfo.CurrentCulture,
            $"Stack {index + 1}, {stack.Count} item{(stack.Count == 1 ? "" : "s")}"));

        return wrapper;
    }

    /// <param name="stackIndex">Which column this tile sits in, and <paramref name="rowIndex"/>
    /// its row within it - carried only so a mouse press on this tile can set keyboard
    /// navigation's position to match (see the press handler below), so an arrow key pressed
    /// right after a click moves on from the tile that was actually clicked rather than from
    /// wherever a previous arrow key last left it.</param>
    private Border BuildTile(ShelfEntry entry, bool selected, bool last, int stackIndex, int rowIndex)
    {
        // A fixed-size host rather than the icon itself, so the later swap from the drawn
        // fallback to a real shell icon changes what fills this box without changing the box:
        // same 22x22 size, same centered position, so the row does not shift when the two are
        // mixed on the same shelf.
        var iconHost = new Grid
        {
            Width = 22,
            Height = 22,
            HorizontalAlignment = HorizontalAlignment.Center,
        };
        // Cache checked synchronously, on the UI thread, before anything is drawn: this is a
        // dictionary lookup, never an extraction, so it cannot stall. A hit paints the real icon
        // on the very first frame; without this check, every tile drew the fallback first and
        // raced a background swap into it on every repaint, including every selection change,
        // one frame of drawn geometry that a cache hit never needed to show at all.
        if (ShellIcons.TryGetCached(entry.Path, out var cachedIcon))
        {
            iconHost.Children.Add(BuildIconImage(cachedIcon));
        }
        else
        {
            iconHost.Children.Add(BuildFallbackIcon(entry));
            RequestShellIcon(entry.Path, iconHost);
        }

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
        content.Children.Add(iconHost);
        content.Children.Add(label);

        // A Grid rather than handing content straight to the Border, so the hover-only remove
        // affordance can sit on top of it without changing the tile's own layout: the overlay
        // occupies the same cell, drawn last, and never affects where the icon or label lands.
        var removeButton = BuildRemoveButton(entry);
        var overlay = new Grid();
        overlay.Children.Add(content);
        overlay.Children.Add(removeButton);

        var tile = new Border
        {
            Width = TileSize,
            Height = TileSize,
            CornerRadius = new CornerRadius(8),
            Margin = new Thickness(0, 0, 0, last ? 0 : Gap),
            // SelectionRing, not the raw accent: see Apply's own comment. Drawn at a thickness
            // that reads as a ring rather than a coincidental extra pixel.
            BorderBrush = selected ? (Brush)FindResource("SelectionRing") : Brushes.Transparent,
            BorderThickness = new Thickness(selected ? 1.5 : 0),
            Padding = new Thickness(4),
            Cursor = Cursors.Hand,
            Child = overlay,
            ToolTip = entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name,
        };

        AutomationProperties.SetName(tile, entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name);
        tile.ContextMenu = BuildTileMenu(entry);
        tile.MouseEnter += (_, _) => removeButton.Visibility = Visibility.Visible;
        tile.MouseLeave += (_, _) => removeButton.Visibility = Visibility.Collapsed;

        // Where the current press started, so PreviewMouseMove can tell a click from the
        // beginning of a drag. Null between gestures and while none is in progress.
        Point? pressStart = null;

        tile.PreviewMouseLeftButtonDown += (_, e) =>
        {
            // The remove button sits inside this tile's own visual tree, so a click on it
            // tunnels through here first. Left alone, every remove-button click would also
            // select the tile it is about to remove, which is at best a flicker and at worst a
            // selection change the person never asked for on a tile that is a moment from gone.
            if (IsDescendantOf(e.OriginalSource as DependencyObject, removeButton)) return;

            pressStart = e.GetPosition(tile);

            // Keyboard navigation's position follows the mouse, not just the other way round:
            // without this, clicking a tile and then pressing an arrow key would move relative to
            // wherever the LAST arrow key left off, which could be a tile nowhere near the one
            // just clicked.
            _focusColumn = stackIndex;
            _focusRow = rowIndex;

            EntryPressed?.Invoke(entry.Path, Keyboard.Modifiers.HasFlag(ModifierKeys.Control));
        };

        tile.PreviewMouseMove += (_, e) =>
        {
            if (pressStart is not { } start || e.LeftButton != MouseButtonState.Pressed) return;

            var current = e.GetPosition(tile);
            if (Math.Abs(current.X - start.X) < SystemParameters.MinimumHorizontalDragDistance &&
                Math.Abs(current.Y - start.Y) < SystemParameters.MinimumVerticalDragDistance) return;

            // Cleared before the drag starts, not after: ShelfWindow.StartDrag runs DoDragDrop
            // synchronously on this stack frame, that call pumps its own message loop until the
            // drag ends, and a MouseMove that reaches this handler again while it is still
            // running must not try to start a second drag on top of the first.
            pressStart = null;
            var paths = _lastModel?.DragPaths(entry.Path) ?? [entry.Path];

            // The tile, not this control and not the window: StartDrag's guard asks whether the
            // element the press actually landed on belongs to the window about to call
            // DoDragDrop, and only the pressed element can answer that.
            DragOutRequested?.Invoke(tile, paths);
        };

        tile.PreviewMouseLeftButtonUp += (_, _) => pressStart = null;

        return tile;
    }

    /// <summary>
    /// The hover-only "take this off the shelf" control. Not the only way to remove a tile, and
    /// deliberately not: its visibility depends on a mouse already hovering this exact tile,
    /// which a keyboard or touch user cannot do, so <see cref="BuildTileMenu"/> carries the same
    /// action somewhere that hover is not the price of admission.
    /// </summary>
    private Button BuildRemoveButton(ShelfEntry entry)
    {
        var icon = new Path
        {
            Data = RemoveIcon,
            Stretch = Stretch.Uniform,
            Stroke = (Brush)FindResource("NotchInk"),
            StrokeThickness = 1.6,
            StrokeStartLineCap = PenLineCap.Round,
            StrokeEndLineCap = PenLineCap.Round,
        };

        var button = new Button
        {
            Width = 16,
            Height = 16,
            Padding = new Thickness(4),
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            Cursor = Cursors.Hand,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Visibility = Visibility.Collapsed,
            Content = icon,
        };

        AutomationProperties.SetName(button, string.Create(CultureInfo.CurrentCulture,
            $"Remove {entry.Name} from the shelf"));

        button.Click += (_, e) =>
        {
            e.Handled = true;
            RemoveRequested?.Invoke(_lastModel?.DragPaths(entry.Path) ?? [entry.Path]);
        };

        return button;
    }

    /// <summary>
    /// Open, show in file manager, remove: the same three actions a mouse reaches through
    /// hovering plus a click (open and reveal have no other way in at all), always present
    /// regardless of hover, which is what makes the whole set reachable without one.
    /// </summary>
    private ContextMenu BuildTileMenu(ShelfEntry entry)
    {
        var open = new MenuItem { Header = "Open" };
        open.Click += (_, _) => OpenRequested?.Invoke(entry.Path);

        var reveal = new MenuItem { Header = "Show in file manager" };
        reveal.Click += (_, _) => RevealRequested?.Invoke(entry.Path);

        var remove = new MenuItem { Header = "Remove" };
        remove.Click += (_, _) => RemoveRequested?.Invoke(_lastModel?.DragPaths(entry.Path) ?? [entry.Path]);

        var menu = new ContextMenu();
        menu.Items.Add(open);
        menu.Items.Add(reveal);
        menu.Items.Add(new Separator());
        menu.Items.Add(remove);

        // Opened/Closed, on the menu itself, not ContextMenuOpening/Closing bubbling up from the
        // tile: a ContextMenu is popup content, hosted outside the tile's own visual subtree, so
        // these fire correctly whether the tile that opened the menu still exists or not by the
        // time it closes. That is what makes _openMenu (and Render's force-close of it) reliable
        // in the one case that broke the previous design: a re-render destroying the tile while
        // its menu is still up.
        menu.Opened += (_, _) => { _openMenu = menu; MenuOpenChanged?.Invoke(true); };
        menu.Closed += (_, _) =>
        {
            // Guards against stomping a DIFFERENT, newer menu: this fires both for an ordinary
            // close (Esc, a click, losing focus) and for the force-close Render performs above,
            // and either way _openMenu must already be (or be about to become) this same menu.
            if (!ReferenceEquals(_openMenu, menu)) return;
            _openMenu = null;
            MenuOpenChanged?.Invoke(false);
        };

        return menu;
    }

    /// <summary>Whether <paramref name="source"/> is <paramref name="ancestor"/> or sits inside
    /// it, walking up the visual tree. Used only to tell a click on the remove button apart from
    /// a click on the rest of the tile it sits on top of: see the comment beside its one call
    /// site.</summary>
    private static bool IsDescendantOf(DependencyObject? source, DependencyObject ancestor)
    {
        // VisualTreeHelper.GetParent throws on anything that is not a Visual or a Visual3D, and
        // MouseButtonEventArgs.OriginalSource carries no such guarantee. The loop condition
        // checks that before every step rather than once, since the walk itself can only produce
        // more Visuals from here, but the very first value handed in has not been checked yet.
        while (source is Visual or Visual3D)
        {
            if (ReferenceEquals(source, ancestor)) return true;
            source = VisualTreeHelper.GetParent(source);
        }

        return false;
    }

    /// <summary>
    /// The drawn document or folder geometry, shown until (and unless) a real shell icon
    /// replaces it. Also the permanent answer for anything ShellIcons could not resolve: a path
    /// that no longer exists with no recognisable extension, or a network path that never
    /// answers. A shelf of empty tiles is worse than a shelf of generic ones, so this is what
    /// stays on screen rather than a blank box.
    /// </summary>
    private Path BuildFallbackIcon(ShelfEntry entry) => new()
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

    /// <summary>
    /// Asks ShellIcons for the real icon off the UI thread (a cache miss can be as slow as the
    /// shell, or a network share that never answers, takes to reply), and swaps it into
    /// <paramref name="host"/> only if one comes back. <paramref name="host"/> keeps the drawn
    /// fallback otherwise, which is the point: a resolution failure must never leave a blank
    /// tile behind.
    /// </summary>
    private void RequestShellIcon(string path, Grid host)
    {
        Task.Run(() =>
        {
            if (!ShellIcons.TryGet(path, out var icon)) return;

            Dispatcher.BeginInvoke(() =>
            {
                host.Children.Clear();
                host.Children.Add(BuildIconImage(icon));
            });
        });
    }

    /// <summary>Same 22x22 box as <see cref="BuildFallbackIcon"/>, so a real icon never shifts
    /// the row whether it arrives on the first frame (a cache hit) or a moment later (a swap
    /// after extraction).</summary>
    private static Image BuildIconImage(ImageSource icon) => new()
    {
        Source = icon,
        Width = 22,
        Height = 22,
        Stretch = Stretch.Uniform,
    };

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
