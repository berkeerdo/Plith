using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using WpfScreenHelper;

namespace Plith.Views;

/// <summary>
/// Full-screen semi-transparent overlay for placing the OSD. Displays nine snap
/// hotspots on a 3x3 grid; clicking one snaps the OSD there. Clicking anywhere else
/// places the OSD at that exact point (custom position). Save persists whatever is
/// picked; Cancel restores the pre-edit settings. Esc = cancel, Enter = save.
///
/// One overlay window per monitor - see PositionOverlayCoordinator (invoked from
/// OsdHost) for the multi-screen composition. Every overlay reports clicks back to
/// a single delegate so Save/Cancel from any monitor commits the same value.
/// </summary>
public partial class PositionOverlayWindow : Window
{
    // Physical inset from working area edges for hotspot placement. Matches the
    // EdgeMarginDip constant in OsdHost so snapping via the overlay lands the OSD in
    // the same spot the classic preset corners always used.
    private const double EdgeMarginDip = 96;
    private const double HotspotDiameter = 96;
    private const double SnapThresholdDip = 60;

    private readonly Screen _screen;

    // OSD's current on-screen rectangle in absolute DIP coordinates. OsdHost updates
    // this on every position change so the overlay knows where the user can grab the
    // card for dragging.
    private Rect _osdRect;
    private bool _isDragging;
    private Point _dragOffset;

    /// <summary>Emits the screen and a centre point (screen-DIP coordinates) whenever
    /// the user picks a new spot: hotspot click, or drag move.</summary>
    public event Action<Screen, Point>? PositionRequested;

    public event Action? SaveRequested;
    public event Action? CancelRequested;

    /// <summary>Publishes the OSD's current rectangle so the overlay can decide when
    /// a mouse-down starts a drag. Called from OsdHost at Enter mode and on every
    /// position change. Also nudges the header/toolbar chrome to whichever screen
    /// side is furthest from the OSD, so the buttons never hide under the card.</summary>
    public void UpdateOsdRect(Rect absoluteOsdRect)
    {
        _osdRect = absoluteOsdRect;
        RelocateChrome();
    }

    private void RelocateChrome()
    {
        if (HeaderPanel is null || ToolbarPanel is null) return;   // not loaded yet

        var area = _screen.WorkingArea;
        var osdCenterX = _osdRect.X + _osdRect.Width / 2;
        // Pick the horizontal side furthest from the OSD - keeps buttons visible
        // while the user drags the card into either corner.
        bool osdOnRight = osdCenterX > area.Left + area.Width / 2;
        var chromeH = osdOnRight ? HorizontalAlignment.Left : HorizontalAlignment.Right;
        double leftMargin = chromeH == HorizontalAlignment.Left ? 24 : 0;
        double rightMargin = chromeH == HorizontalAlignment.Right ? 24 : 0;

        HeaderPanel.HorizontalAlignment = chromeH;
        HeaderPanel.Margin = new Thickness(leftMargin, 20, rightMargin, 0);

        ToolbarPanel.HorizontalAlignment = chromeH;
        ToolbarPanel.Margin = new Thickness(leftMargin, 0, rightMargin, 20);
    }

    public PositionOverlayWindow(Screen screen)
    {
        _screen = screen;
        InitializeComponent();

        // Manually position + size the window over exactly this monitor's working
        // area. WPF's Screen classes want DIPs and so do Window.Left/Top/Width/Height,
        // so no DPI conversion is needed here (the Window handles native scaling).
        Left = screen.WorkingArea.Left;
        Top = screen.WorkingArea.Top;
        Width = screen.WorkingArea.Width;
        Height = screen.WorkingArea.Height;

        Loaded += OnLoaded;
        SaveButton.Click += (_, _) => SaveRequested?.Invoke();
        CancelButton.Click += (_, _) => CancelRequested?.Invoke();
        PreviewKeyDown += OnKeyDown;

        // Drag detection uses PREVIEW events at the Window level (tunneling top-
        // down). The OSD floats visually over the overlay but its click surface
        // isn't reachable under the Topmost overlay window; the overlay owns every
        // click. Preview at Window fires BEFORE any hotspot Border's bubbling
        // MouseDown, so we get first refusal: if the click lands inside the OSD
        // rectangle, we start a drag and Handled=true stops the hotspot Border
        // (which sits underneath the OSD visually) from stealing the click. If
        // the click misses the OSD, the event continues and the appropriate
        // hotspot Border handler runs.
        PreviewMouseLeftButtonDown += OnPreviewMouseDown;
        PreviewMouseMove += OnPreviewMouseMove;
        PreviewMouseLeftButtonUp += OnPreviewMouseUp;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        // Nine dots at the classic 3x3 anchor points inside the working area. Dot
        // positions match the OSD's snap-corner geometry so a user releasing the
        // drag near a dot ends up exactly ON that dot's target.
        var area = _screen.WorkingArea;
        double[] xs = { EdgeMarginDip, area.Width / 2, area.Width - EdgeMarginDip };
        double[] ys = { EdgeMarginDip, area.Height / 2, area.Height - EdgeMarginDip };

        for (int row = 0; row < 3; row++)
        for (int col = 0; col < 3; col++)
        {
            double x = xs[col];
            double y = ys[row];
            var dot = new Border
            {
                Width = HotspotDiameter,
                Height = HotspotDiameter,
                CornerRadius = new CornerRadius(HotspotDiameter / 2),
                Background = new SolidColorBrush(Color.FromArgb(0x20, 0xFF, 0xFF, 0xFF)),
                BorderBrush = new SolidColorBrush(Color.FromArgb(0x60, 0xFF, 0xFF, 0xFF)),
                BorderThickness = new Thickness(2),
                Cursor = Cursors.Hand,
                Tag = (col, row),
            };
            dot.Style = (Style)FindResource("HotspotStyle");
            dot.MouseLeftButtonDown += OnHotspotClick;

            Canvas.SetLeft(dot, x - HotspotDiameter / 2);
            Canvas.SetTop(dot, y - HotspotDiameter / 2);
            ClickCanvas.Children.Add(dot);
        }

        Focusable = true;
        Keyboard.Focus(this);
    }

    private void OnHotspotClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is Border dot && dot.Tag is ValueTuple<int, int> cell)
        {
            // Compute the OSD-centre for this grid cell using the OSD's actual size,
            // so the OSD lands snapped to the target edge inset (EdgeMarginDip + w/2),
            // not merely on the dot's visual centre.
            PositionRequested?.Invoke(_screen, GridCellCenter(cell.Item1, cell.Item2));
        }
        e.Handled = true;
    }

    private Point GridCellCenter(int col, int row)
    {
        var area = _screen.WorkingArea;
        double w = _osdRect.Width > 0 ? _osdRect.Width : 200;
        double h = _osdRect.Height > 0 ? _osdRect.Height : 80;
        double cx = col switch
        {
            0 => area.Left + EdgeMarginDip + w / 2,
            1 => area.Left + area.Width / 2,
            _ => area.Right - EdgeMarginDip - w / 2,
        };
        double cy = row switch
        {
            0 => area.Top + EdgeMarginDip + h / 2,
            1 => area.Top + area.Height / 2,
            _ => area.Bottom - EdgeMarginDip - h / 2,
        };
        return new Point(cx, cy);
    }

    private Point OverlayLocalToAbsolute(Point local)
        => new(_screen.WorkingArea.Left + local.X, _screen.WorkingArea.Top + local.Y);

    private void OnPreviewMouseDown(object sender, MouseButtonEventArgs e)
    {
        var absolute = OverlayLocalToAbsolute(e.GetPosition(this));
        if (_osdRect.Contains(absolute))
        {
            _isDragging = true;
            _dragOffset = new Point(absolute.X - _osdRect.Left, absolute.Y - _osdRect.Top);
            CaptureMouse();
            Cursor = Cursors.Hand;
            e.Handled = true;
        }
    }

    private void OnPreviewMouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        var absolute = OverlayLocalToAbsolute(e.GetPosition(this));

        if (_isDragging)
        {
            // Straight cursor-follow during drag - no mid-drag snap. Snapping only
            // kicks in on release so the motion feels buttery instead of jittery
            // when the cursor grazes a magnet zone.
            var newLeft = absolute.X - _dragOffset.X;
            var newTop = absolute.Y - _dragOffset.Y;
            var center = new Point(newLeft + _osdRect.Width / 2, newTop + _osdRect.Height / 2);
            PositionRequested?.Invoke(_screen, center);
        }
        else
        {
            // Hover feedback: hand cursor over the OSD, plain arrow elsewhere.
            Cursor = _osdRect.Contains(absolute) ? Cursors.Hand : Cursors.Arrow;
        }
    }

    private void OnPreviewMouseUp(object sender, MouseButtonEventArgs e)
    {
        if (!_isDragging) return;
        _isDragging = false;
        if (IsMouseCaptured) ReleaseMouseCapture();
        Cursor = _osdRect.Contains(OverlayLocalToAbsolute(e.GetPosition(this)))
            ? Cursors.Hand : Cursors.Arrow;

        // Snap on release. Compute the current OSD centre and pull it to the
        // nearest 3x3 hotspot if close enough. Alt held = no snap.
        var currentCenter = new Point(_osdRect.X + _osdRect.Width / 2, _osdRect.Y + _osdRect.Height / 2);
        var snapped = SnapCenterOnRelease(currentCenter);
        if (snapped != currentCenter)
        {
            PositionRequested?.Invoke(_screen, snapped);
        }
        e.Handled = true;
    }

    // Snap targets are OSD CENTRES: EdgeMarginDip inset from each edge plus w/2 or
    // h/2, and the middle of the working area. Threshold ~80 DIP so a natural
    // "close-enough" release locks into the preset spot.
    private Point SnapCenterOnRelease(Point candidate)
    {
        if ((Keyboard.Modifiers & ModifierKeys.Alt) != 0) return candidate;
        const double snapThreshold = 80;

        var area = _screen.WorkingArea;
        double w = _osdRect.Width;
        double h = _osdRect.Height;
        double[] cxTargets =
        {
            area.Left + EdgeMarginDip + w / 2,
            area.Left + area.Width / 2,
            area.Right - EdgeMarginDip - w / 2,
        };
        double[] cyTargets =
        {
            area.Top + EdgeMarginDip + h / 2,
            area.Top + area.Height / 2,
            area.Bottom - EdgeMarginDip - h / 2,
        };

        double cx = candidate.X, cy = candidate.Y;
        foreach (var tx in cxTargets)
            if (Math.Abs(cx - tx) < snapThreshold) { cx = tx; break; }
        foreach (var ty in cyTargets)
            if (Math.Abs(cy - ty) < snapThreshold) { cy = ty; break; }
        return new Point(cx, cy);
    }

    private void OnKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Escape) { CancelRequested?.Invoke(); e.Handled = true; }
        else if (e.Key == Key.Enter) { SaveRequested?.Invoke(); e.Handled = true; }
    }
}
