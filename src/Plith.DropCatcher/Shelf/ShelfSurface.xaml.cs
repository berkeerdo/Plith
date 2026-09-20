using System.Globalization;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Media3D;
using System.Windows.Shapes;
using Plith.Services.Shelf;
using Plith.Views.Presentation;

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
    /// <summary>
    /// The grid, read from the one place both processes compile.
    ///
    /// These were four private constants here, and the cap on how many files the shelf held was
    /// a fifth number in ShelfStore, on the other side of a process boundary. The two disagreed:
    /// the store kept 20 and this surface could draw 10, so half a full shelf sat behind count
    /// chips that are in no UIA tree. NotchGeometry.ShelfCapacity is now defined as the product
    /// of these two, which is what makes "every file on the shelf is on the screen" a fact about
    /// the code rather than a thing to remember.
    /// </summary>
    private const double TileSize = NotchGeometry.ShelfTileSize;

    /// <inheritdoc cref="TileSize"/>
    private const double Gap = NotchGeometry.ShelfGap;

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
    /// live shelf (a remove closes a gap, a fresh delivery can reorder entries) in a way an
    /// index survives and a captured path would not: every use re-resolves it against the
    /// CURRENT model through <see cref="ResolveFocus"/> rather than trusting what was true when
    /// it was set.
    /// </summary>
    private int? _focusIndex;

    /// <summary>
    /// The press a drag could still grow out of: which PATH was pressed, and where the pointer
    /// was when it happened, in THIS control's coordinates. Null between gestures.
    ///
    /// On this control, keyed by path, and both halves of that are the fix for a defect a green
    /// build could not see. See <see cref="BeginPress"/>.
    /// </summary>
    private (string Path, Point Start)? _press;

    public ShelfSurface()
    {
        InitializeComponent();

        // A button-DOWN anywhere on this control ends the previous press before the new one is
        // recorded. Tunnelling reaches this root first, so a down that goes on to run BeginPress
        // re-records immediately afterwards and loses nothing; a down that does not (the header,
        // the Clear or new-stack buttons, the gap between two tiles) leaves nothing behind.
        //
        // THIS IS THE ORDINARY PATH, NOT A CORNER, and that is the whole reason it is here. No
        // mouse capture is taken, so a release outside this window raises no event on it at all
        // and the button-up below never runs - and a drag OUT ends outside this window BY
        // DEFINITION, since landing a file somewhere else is what it is for. So every successful
        // drag out leaves a press recorded. Without this line the next down on anything that is
        // not a tile, followed by a move onto one, would start a drag from a press that never
        // landed on a tile, measured against an origin from the gesture before it. A drag from a
        // press that did not land is the exact failure family this whole branch exists because
        // of: see ShelfWindow.StartDrag, whose guard was written after DoDragDrop was measured
        // failing, and once hanging for seventeen seconds, for a press it did not receive.
        PreviewMouseLeftButtonDown += (_, _) => EndPress();

        // A button-up ANYWHERE on this control ends the press, not only one on the tile that
        // took it. A preview event tunnels through this root whatever the pointer is over, so a
        // release in the gap between two tiles, or on the header, is caught here rather than
        // leaving a press outstanding with no gesture behind it. It cannot catch a release
        // OUTSIDE the window, which is why the button-down above exists as well.
        PreviewMouseLeftButtonUp += (_, _) => EndPress();
    }

    /// <summary>A tile was pressed: the path, and whether the press was additive (Ctrl held).
    /// The surface does not decide what a press means: ShelfModel.DragPaths and Select do that.
    /// It only reports what happened, on the same instance it was told to paint.</summary>
    public event Action<string, bool>? EntryPressed;

    /// <summary>The "clear the shelf" control was pressed.</summary>
    public event Action? ClearRequested;

    /// <summary>Take these paths off the shelf: the tile's own selection, or the whole current
    /// selection if the tile removed belongs to it. Raised by the hover affordance and by the
    /// tile's context menu, both computing the same set from ShelfModel.DragPaths.</summary>
    public event Action<IReadOnlyList<string>>? RemoveRequested;

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
        var items = model.Items;
        if (items.Count == 0) return false;

        // A flat grid, so Left and Right are one step and Up and Down are one ROW. The stack
        // build moved by column and row separately and had to ask how many rows the column under
        // the cursor happened to draw, which is the arithmetic VisibleRowsShown existed to keep
        // in step with construction. A wrapping row of a known width needs neither.
        return key switch
        {
            Key.Left => Move(model, items, -1),
            Key.Right => Move(model, items, 1),
            Key.Up => Move(model, items, -NotchGeometry.ShelfTilesPerRow),
            Key.Down => Move(model, items, NotchGeometry.ShelfTilesPerRow),
            Key.Space => ToggleFocused(model, items),
            Key.Enter => OpenFocused(items),
            Key.Delete => RemoveFocused(model, items),
            _ => false,
        };
    }

    /// <summary>
    /// The tile keyboard input currently acts on: the last one an arrow key landed on (or a
    /// mouse press set, see BuildTile), clamped against what the shelf holds now, or the shelf's
    /// very first tile before anything has set a position at all. The clamped result is committed
    /// back to the field, so a Space, Enter or Delete pressed before the first arrow key acts on
    /// the first tile AND leaves the next arrow key moving on from there, rather than from a
    /// position nothing was ever actually on.
    /// </summary>
    private (int Index, string Path)? ResolveFocus(IReadOnlyList<ShelfEntry> items)
    {
        if (items.Count == 0) return null;

        var index = Math.Clamp(_focusIndex ?? 0, 0, items.Count - 1);
        _focusIndex = index;
        return (index, items[index].Path);
    }

    private bool Move(ShelfModel model, IReadOnlyList<ShelfEntry> items, int delta)
    {
        // Whether anything has been on this grid before matters here and only here: the very
        // first arrow key press has to land ON the resolved default rather than move AWAY from
        // it, the same first-press behaviour any keyboard list gives a shelf nobody has
        // navigated yet. ResolveFocus itself cannot tell the two cases apart once it returns,
        // because it commits the default the moment it resolves one.
        var firstPress = _focusIndex is null;
        if (ResolveFocus(items) is not { } current) return false;

        if (!firstPress)
        {
            // CLAMPED rather than wrapped. A Right on the last tile staying put is what a grid
            // does; jumping to the first tile of the first row is a different gesture and one
            // nobody asked for here.
            var index = Math.Clamp(current.Index + delta, 0, items.Count - 1);
            _focusIndex = index;
            current = (index, items[index].Path);
        }

        model.Select(current.Path, additive: false);
        Render(model);
        return true;
    }

    private bool ToggleFocused(ShelfModel model, IReadOnlyList<ShelfEntry> items)
    {
        if (ResolveFocus(items) is not { } current) return false;

        model.Select(current.Path, additive: true);
        Render(model);
        return true;
    }

    private bool OpenFocused(IReadOnlyList<ShelfEntry> items)
    {
        if (ResolveFocus(items) is not { } current) return false;

        OpenRequested?.Invoke(current.Path);
        return true;
    }

    private bool RemoveFocused(ShelfModel model, IReadOnlyList<ShelfEntry> items)
    {
        if (ResolveFocus(items) is not { } current) return false;

        IReadOnlyList<string> toRemove = model.Selection.Count > 0 ? [.. model.Selection] : [current.Path];
        RemoveRequested?.Invoke(toRemove);
        return true;
    }

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

        var items = model.Items;
        if (items.Count == 0)
        {
            // The page is reachable even when nothing has ever been dropped, so the empty state
            // has to earn its space rather than leave a blank card behind the header.
            //
            // The sentence inside a dashed outline, matching ShelfWidget's empty page on the
            // notch: the two are the same product and a person meets them minutes apart. A
            // sentence with no shape around it instructs without showing where, which is what
            // both of these did and what made the empty shelf read as unfinished.
            var message = new TextBlock
            {
                Text = "Drop files on the notch to keep them here",
                FontSize = 12,
                TextWrapping = TextWrapping.Wrap,
                TextAlignment = TextAlignment.Center,
                Foreground = (Brush)FindResource("NotchInkMuted"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                MaxWidth = 240,
            };

            var outline = new Rectangle
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
                Width = NotchGeometry.ShelfTilesPerRow * TileSize
                      + (NotchGeometry.ShelfTilesPerRow - 1) * Gap,
                // ONE ROW tall, which is what an empty shelf's window is given.
                //
                // It used to be the full grid height, on the reasoning that the card should be
                // the same size as a full one so the shape does not jump when the first file
                // lands. That reasoning belonged to a fixed frame. The frame now hugs its
                // contents (NotchGeometry.ShelfFrameFor), so an empty shelf opens one row tall
                // and a card built for three rows would simply be clipped by it.
                Height = TileSize,
            };
            empty.Children.Add(outline);
            empty.Children.Add(message);

            Columns.Children.Add(empty);
            // Named on THIS control, not on Columns: Columns is a StackPanel, and WPF gives a
            // StackPanel no automation peer at all, so a name set on one reaches nothing (see
            // NamedBorder's own header comment for the review that caught this same defect on
            // every Border below). The UserControl root does have a peer - the same fix
            // AmbientCardView/AudioCardView/MediaCardView already use for the same reason, on the
            // same principle: a Grid or StackPanel that composes several peerless children names
            // the WHOLE control instead, one level up from where the name was tried first.
            AutomationProperties.SetName(this, "Shelf, empty. Drop files on the notch to keep them here.");

            // Nothing left for keyboard navigation to be on. Left set, the next non-empty
            // Render would resolve a stale index against an entirely unrelated delivery -
            // ResolveFocus clamps the NUMBERS back into range, but a clamped index still points
            // at whatever now happens to occupy that slot, not at anything the person actually
            // navigated to.
            _focusIndex = null;
            return;
        }

        // ONE WRAPPING ROW, and the panel decides where a row ends.
        //
        // The stack build had that decision in two places: BuildColumn computed how many tiles a
        // column drew, and keyboard navigation asked VisibleRowsShown the same question
        // separately. The rule itself was also written down wrong in the verification document
        // ("a column draws at most two tiles" when a stack of three draws exactly one, because
        // the count chip costs a slot), and the fixture built on that wrong rule made two
        // verification items report the product broken. A WrapPanel of a known width has one
        // definition of where a row ends and no chip to make room for.
        var grid = new WrapPanel
        {
            // N * (tile + gap), NOT N * tile + (N - 1) * gap.
            //
            // Every tile carries a uniform right margin so the WrapPanel spaces rows and columns
            // alike, which means a tile OCCUPIES tile + gap. Sizing the panel to the narrower
            // "gaps only between tiles" figure leaves the fifth tile of each row 8 DIP short of
            // fitting, so it wraps: four per row instead of five, twelve tiles instead of
            // fifteen, and the last three arranged nowhere at all. The capacity guarantee would
            // have been quietly false. Caught by render-widgets.ps1's tile-hit check reporting a
            // tile that was never arranged.
            //
            // The last tile's trailing gap hangs off the right edge, which costs nothing: 360
            // still sits inside the 384 frame.
            Width = NotchGeometry.ShelfTilesPerRow
                  * (NotchGeometry.ShelfTileSize + NotchGeometry.ShelfGap),
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        for (var i = 0; i < items.Count; i++)
            grid.Children.Add(BuildTile(items[i], model.Selection.Contains(items[i].Path), i));

        Columns.Children.Add(grid);

        // On THIS control, not on the panel - see the empty branch above for why.
        AutomationProperties.SetName(this, string.Create(CultureInfo.CurrentCulture,
            $"Shelf, {items.Count} item{(items.Count == 1 ? "" : "s")}"));
    }

    /// <summary>
    /// Remember a press, so a later move can decide it was the start of a drag.
    ///
    /// THE TRAP THIS SHAPE EXISTS TO CLOSE, because it cost the headline feature of this slice
    /// and nothing on this branch could see it: the press state used to be a local in
    /// <see cref="BuildTile"/>, captured by that one tile's own handlers. A press raises
    /// <see cref="EntryPressed"/>, ShelfWindow answers it by re-rendering, and
    /// <see cref="Render"/> clears Columns.Children and rebuilds every tile from scratch. So the
    /// element that took the press was out of the tree before the button came up, the
    /// REPLACEMENT tile's handlers saw a fresh null, and no move on it could ever reach the drag
    /// threshold. Dragging a file out to another application, and dragging a tile between
    /// stacks, were both dead on the first gesture and on every one after it.
    ///
    /// Keeping it here, keyed by the path rather than by the element, is what makes a render
    /// survivable rather than merely avoidable. ANY Render may run between a press and the move
    /// that follows it - not just the one this press causes: Plith re-sends the whole shelf after
    /// every mutating verb, and a Palette message re-renders too. A fix that only stopped this
    /// one render would leave the next person to add one to rediscover the same defect. If you
    /// are adding a Render call, that is the trap: it is safe, and it is safe BECAUSE nothing
    /// about a gesture in progress is stored on the elements Render destroys.
    ///
    /// The point is in THIS control's coordinates rather than the tile's, for the same reason.
    /// A tile-relative origin measures the pointer against something a re-render is allowed to
    /// move, so a rebuild that put the same path in a different slot would read as a large
    /// pointer movement and start a drag nobody asked for. This control does not move under its
    /// own tiles.
    /// </summary>
    public void BeginPress(string path, Point start) => _press = (path, start);

    /// <summary>The press is over without a drag: a button-up, anywhere on this control.</summary>
    public void EndPress() => _press = null;

    /// <summary>
    /// A pointer moved while a press is outstanding. Raises <see cref="DragOutRequested"/> and
    /// returns true once the move clears the system's drag threshold, and does nothing at all
    /// below it, which is what keeps a click a click.
    ///
    /// <paramref name="path"/> is the path of the tile the pointer is over now, and it must
    /// match the pressed one: a press on tile A followed by a move over tile B is not a drag of
    /// B. <paramref name="tile"/> is the live element under the pointer, which after a re-render
    /// is a different object from the one pressed - it is passed on to ShelfWindow.StartDrag,
    /// whose guard asks whether the element belongs to the window about to call DoDragDrop, and
    /// only an element that is still IN the tree can answer that.
    /// </summary>
    public bool ContinuePress(DependencyObject tile, string path, Point current)
    {
        if (_press is not { } press || !PathEquals(press.Path, path)) return false;

        if (Math.Abs(current.X - press.Start.X) < SystemParameters.MinimumHorizontalDragDistance &&
            Math.Abs(current.Y - press.Start.Y) < SystemParameters.MinimumVerticalDragDistance) return false;

        // Cleared before the drag starts, not after: ShelfWindow.StartDrag runs DoDragDrop
        // synchronously on this stack frame, that call pumps its own message loop until the drag
        // ends, and a MouseMove that reaches this method again while it is still running must not
        // try to start a second drag on top of the first.
        _press = null;
        var paths = _lastModel?.DragPaths(path) ?? [path];

        DragOutRequested?.Invoke(tile, paths);
        return true;
    }

    private static bool PathEquals(string a, string b) =>
        string.Equals(a, b, StringComparison.OrdinalIgnoreCase);

    // Asks nothing first, on purpose: the shelf holds references, not the files themselves, so
    // clearing it deletes nothing on disk, and a confirmation dialog for a reversible action on
    // a surface this small is friction rather than safety. If a future change makes Clear do
    // something that is NOT trivially reversible, this is the line that stops being true.
    private void OnClearClick(object sender, RoutedEventArgs e) => ClearRequested?.Invoke();

    /// <param name="index">Where this tile sits in the flat list, carried only so a mouse press
    /// on it can set keyboard navigation's position to match (see the press handler below), so an
    /// arrow key pressed right after a click moves on from the tile that was actually clicked
    /// rather than from wherever a previous arrow key last left it.</param>
    private NamedBorder BuildTile(ShelfEntry entry, bool selected, int index)
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

        var tile = new NamedBorder
        {
            Width = TileSize,
            Height = TileSize,
            CornerRadius = new CornerRadius(8),
            // HALF THE GAP ON EACH SIDE, not a whole one on the right.
            //
            // A whole gap on the right only is what a WrapPanel needs to space tiles, and it
            // leaves the last tile of every row trailing 8 DIP of margin off the end. Measured
            // on 2026-09-20: the row sat 12 DIP from the card's left edge and 20 from its right,
            // so the grid was 4 DIP left of centre and the leftmost tile's selection ring sat
            // hard against the edge. Reported from the running build as the ring on the end
            // tiles "going outside the area".
            //
            // Split evenly, each tile still occupies tile + gap, the spacing between tiles is
            // unchanged, and both outer edges get the same half-gap. Symmetric by construction
            // rather than by a correction somewhere else.
            Margin = new Thickness(Gap / 2, 0, Gap / 2, Gap),
            // SelectionRing, not the raw accent: see Apply's own comment. Drawn at a thickness
            // that reads as a ring rather than a coincidental extra pixel.
            BorderBrush = selected ? (Brush)FindResource("SelectionRing") : Brushes.Transparent,
            BorderThickness = new Thickness(selected ? 1.5 : 0),
            // TRANSPARENT, NOT NULL, and it is the difference between a tile that answers a
            // pointer and one that does not. WPF hit-tests a Transparent brush but not a null
            // one, so with no Background only the painted icon and label answered: a hover, a
            // click and the press a drag starts from were all dead in the gaps between them,
            // including the tile's exact centre, which is where a person aims. Measured on
            // hardware (docs/SHELF-VERIFICATION.md 3.10) as a click at the centre doing nothing
            // while the same click on the icon selected, showed the remove control and armed the
            // press. ShelfWindow.xaml states this same rule for the window's own background; the
            // tile did not follow it. render-widgets.ps1's tile-hit check now fails the build if
            // this is removed, because no other check here asks which element a POINT belongs to.
            Background = Brushes.Transparent,
            Padding = new Thickness(4),
            Cursor = Cursors.Hand,
            Child = overlay,
            ToolTip = entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name,
        };

        AutomationProperties.SetName(tile, entry.IsDirectory ? $"Folder {entry.Name}" : entry.Name);
        tile.ContextMenu = BuildTileMenu(entry);
        tile.MouseEnter += (_, _) => removeButton.Visibility = Visibility.Visible;
        tile.MouseLeave += (_, _) => removeButton.Visibility = Visibility.Collapsed;

        tile.PreviewMouseLeftButtonDown += (_, e) =>
        {
            // The remove button sits inside this tile's own visual tree, so a click on it
            // tunnels through here first. Left alone, every remove-button click would also
            // select the tile it is about to remove, which is at best a flicker and at worst a
            // selection change the person never asked for on a tile that is a moment from gone.
            if (IsDescendantOf(e.OriginalSource as DependencyObject, removeButton)) return;

            // Recorded BEFORE EntryPressed is raised, because raising it is what destroys this
            // element. See BeginPress for the whole of that trap.
            BeginPress(entry.Path, e.GetPosition(this));

            // Keyboard navigation's position follows the mouse, not just the other way round:
            // without this, clicking a tile and then pressing an arrow key would move relative to
            // wherever the LAST arrow key left off, which could be a tile nowhere near the one
            // just clicked.
            _focusIndex = index;

            EntryPressed?.Invoke(entry.Path, Keyboard.Modifiers.HasFlag(ModifierKeys.Control));
        };

        tile.PreviewMouseMove += (_, e) =>
        {
            // e.LeftButton, checked here rather than inside ContinuePress, because it is the one
            // part of a press this control cannot hold for itself: it is the mouse device's live
            // state, and only an element in a real input route can be asked for it.
            if (e.LeftButton != MouseButtonState.Pressed) return;

            // `tile` is whichever tile the pointer is over NOW, which after a re-render is a
            // different object from the one the press landed on. That is fine and is the point:
            // the press lives on this control, keyed by path, so the replacement tile can finish
            // the gesture the destroyed one started.
            _ = ContinuePress(tile, entry.Path, e.GetPosition(this));
        };

        tile.PreviewMouseLeftButtonUp += (_, _) => EndPress();

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

    /// <summary>
    /// A Border that actually has an automation peer, which a bare Border never does. WPF only
    /// creates a peer for a type that overrides <c>OnCreateAutomationPeer</c>, Border is not one
    /// of them, and <c>check-a11y.ps1</c>'s own <c>$peerless</c> list already named it as such
    /// before this class existed.
    ///
    /// A review of this task found every AutomationProperties.SetName below was landing on a
    /// plain Border - the tile, the overflow tile, the per-stack wrapper - and reaching nothing
    /// at all, the exact defect check-a11y.ps1 exists to catch. It missed it only because that
    /// script never read code-behind at all (see check-a11y.ps1's own updated header comment).
    ///
    /// This is the SMALLER of two fixes the review raised, and the choice is recorded rather than
    /// assumed. The bigger one is real: these tiles are selectable and activatable, arrow keys
    /// move between them, and a genuine <see cref="ListBoxItem"/> would give a screen reader the
    /// SelectionItem pattern for free - "N of M selected" rather than a name alone. That would
    /// also mean rebuilding drag-out, the hover remove affordance, the context menu and the
    /// keyboard handling this same task just wrote on top of a Selector's own model instead of
    /// ShelfModel's, none of it verified on hardware yet either. <see cref="NamedBorder"/> is the
    /// smaller fix: every name below now reaches a real automation peer today, with the standard
    /// <see cref="FrameworkElementAutomationPeer"/> WPF gives a plain Control - Name, HelpText and
    /// so on all work - but no selection semantics beyond that. If the console pass this project
    /// still owes finds that insufficient for how Narrator actually presents the shelf, the
    /// Selector-based rewrite is the next step, not a surprise.
    /// </summary>
    private sealed class NamedBorder : Border
    {
        protected override AutomationPeer OnCreateAutomationPeer() => new FrameworkElementAutomationPeer(this);
    }
}
