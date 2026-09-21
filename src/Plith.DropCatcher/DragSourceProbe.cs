using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Threading;

namespace Plith.DropCatcher;

/// <summary>
/// A measurement, not a feature. The last one the shelf's outbound direction needs before it can
/// be designed.
///
/// Task 7 settled that a High-integrity process cannot be a drag source: the same binary returns
/// None from High and Copy, Move from Medium with everything else identical. So the catcher has
/// to start the outbound drag as well as receive the inbound drop. That leaves exactly one thing
/// unmeasured, and it is the thing the whole interaction rests on: the press that starts the
/// gesture lands on PLITH's window, and only then does the notch stand aside and the catcher
/// take its place. The catcher would be calling DoDragDrop for a gesture whose button went down
/// in a different process. Task 7 pressed on the probe's OWN window, so it says nothing about
/// this, and nothing in OLE's contract promises either answer.
///
/// The call below is deliberately identical to <see cref="DragOutWindow"/>'s, down to the
/// allowed effects. Task 7's run is this probe's control, and a control is only worth having if
/// one variable changed: here, where the button went down.
///
/// Run it from a console session (over Remote Desktop the gesture is meaningless), press and
/// hold on any ordinary window, drag to the top of the screen, and release over an Explorer
/// window:
///   Plith.DropCatcher.exe --dragsource "C:\some\file.txt"
/// The answer is the DoDragDrop return value in dropcatcher.log. If it is None, the press-a-tile
/// interaction is dead and the next slice starts from a different question — which is the point
/// of finding out before a spec rather than after one.
/// </summary>
internal sealed class DragSourceProbe : IDisposable
{
    private const int VK_LBUTTON = 0x01;

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOACTIVATE = 0x0010;
    private const uint SWP_SHOWWINDOW = 0x0040;

    /// <summary>
    /// How close to the top edge the cursor must come before the probe stands in, in physical
    /// pixels. It stands in for the notch's approach band without being it: the band's measured
    /// size (356x48 DIP) is about not opening for every drag that passes by, and this probe
    /// opens for exactly one gesture that the person aims at it deliberately.
    /// </summary>
    private const int BandHeightPx = 48;

    /// <summary>
    /// The stand-in's rectangle, in physical pixels, anchored to the top edge and centred on the
    /// cursor. Deliberately larger than the notch's 356x116 DIP panel: the cursor has to be
    /// inside it with no arithmetic to get wrong, and the question being measured is OLE's, not
    /// geometry's.
    /// </summary>
    private const int StandInWidthPx = 400;
    private const int StandInHeightPx = 160;

    private readonly CatcherLog _log;
    private readonly string _path;
    private readonly DispatcherTimer _timer;
    private readonly Window _standIn;

    private bool _buttonWasDown;
    private bool _armed;
    private bool _stoodIn;
    private bool _dragging;

    public DragSourceProbe(CatcherLog log, string path)
    {
        _log = log;
        _path = path;

        _standIn = new Window
        {
            Title = "Plith drag-source probe",
            WindowStyle = WindowStyle.None,
            ResizeMode = ResizeMode.NoResize,
            ShowInTaskbar = false,
            ShowActivated = false,
            Topmost = true,
            AllowsTransparency = false,
            Width = 400,
            Height = 160,
            Left = -32000,
            Top = -32000,
            Background = new SolidColorBrush(Color.FromRgb(0x14, 0x14, 0x14)),
            Content = new TextBlock
            {
                Text = "drag-source probe",
                Foreground = Brushes.White,
                FontSize = 14,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };

        // Opaque, unlike the catcher's own window. The catcher is layered because it has to morph
        // into the notch's shape, and the cost recorded there is that nothing can photograph it.
        // A probe is read from its log and from a screenshot, so it pays none of that.
        _timer = new DispatcherTimer(DispatcherPriority.Input, _standIn.Dispatcher)
        {
            Interval = TimeSpan.FromMilliseconds(15),
        };
        _timer.Tick += (_, _) => Poll();
    }

    public void Start()
    {
        _log.Info($"DRAG-SOURCE PROBE ready with {_path}. Exists={System.IO.File.Exists(_path)}");
        _log.Info($"Waiting for a left press that lands somewhere else, then a move above y={BandHeightPx}px.");
        _timer.Start();
    }

    private void Poll()
    {
        // DoDragDrop pumps messages while it blocks, so this timer keeps ticking inside it.
        if (_dragging) return;

        var down = (GetAsyncKeyState(VK_LBUTTON) & 0x8000) != 0;
        if (!GetCursorPos(out var p)) return;

        if (down && !_buttonWasDown)
        {
            // Any press seen here landed elsewhere by construction — the stand-in is off-screen
            // until it is needed. The owner is logged anyway, because that is the control: a run
            // whose press landed on this probe's own window would be measuring Task 7 again.
            _armed = true;
            _stoodIn = false;
            _log.Info($"Press at {p.X},{p.Y} on {DescribeWindowAt(p)}.");
        }
        else if (!down && _buttonWasDown)
        {
            if (_armed && !_stoodIn)
            {
                _log.Info($"Released at {p.X},{p.Y} without reaching the band. Nothing measured; press again.");
            }

            _armed = false;
            _stoodIn = false;
            if (_standIn.IsVisible) _standIn.Hide();
        }
        else if (down && _armed && !_stoodIn && p.Y <= BandHeightPx)
        {
            _stoodIn = true;
            StandInAndDrag(p);
        }

        _buttonWasDown = down;
    }

    private void StandInAndDrag(POINT cursor)
    {
        var x = cursor.X - (StandInWidthPx / 2);
        var y = 0;

        // Show() first, and it is not optional. SetWindowPos alone makes a window the window
        // manager can see while WPF still considers it unshown, builds no visual tree, and the
        // whole surface is inert — the exact failure the catcher's first probe run produced,
        // where the only symptom was MainWindowHandle staying 0.
        if (!_standIn.IsVisible) _standIn.Show();

        var handle = new WindowInteropHelper(_standIn).Handle;
        _ = SetWindowPos(handle, HWND_TOPMOST, x, y, StandInWidthPx, StandInHeightPx,
            SWP_NOACTIVATE | SWP_SHOWWINDOW);

        _log.Info($"Stood in at {x},{y} {StandInWidthPx}x{StandInHeightPx}, cursor at {cursor.X},{cursor.Y}.");

        // Painted BEFORE the drag, and the delay is the whole point. Calling DoDragDrop straight
        // after Show() blocks the UI thread before WPF renders a frame, and a screenshot taken
        // mid-gesture proved it: the stand-in was logged as shown and was not on the screen. OLE
        // does not require a visible source window, so the measurement may well have been sound -
        // but "may well have been" is not what this plan has been measuring things to.
        _dragging = true;
        var paint = new DispatcherTimer(DispatcherPriority.Normal, _standIn.Dispatcher)
        {
            Interval = TimeSpan.FromMilliseconds(150),
        };
        paint.Tick += (_, _) => { paint.Stop(); StartDrag(); };
        paint.Start();
    }

    private void StartDrag()
    {
        var data = new DataObject();
        data.SetData(DataFormats.FileDrop, new[] { _path });

        _log.Info($"Painted. Visible={_standIn.IsVisible}, ActualWidth={_standIn.ActualWidth:0}. Starting DoDragDrop for a press that landed in another process...");
        try
        {
            // Identical to the Task 7 call. The return value is the answer: None means nothing
            // accepted it, anything else means a target took it and the press crossing a process
            // boundary did not stop OLE's loop from picking the gesture up.
            var result = DragDrop.DoDragDrop(_standIn, data, DragDropEffects.Copy | DragDropEffects.Link);
            _log.Info($"DoDragDrop RETURNED: {result}");

            // Where the gesture ended, and it is not optional. The first run of this probe
            // returned None and it meant nothing: a maximised terminal had covered the Explorer
            // window the release was aimed at, so the answer being read as "OLE refused the drag"
            // was really "the window underneath takes no files". A None over the wrong window and
            // a None over the right one are the same line in a log without this one.
            if (GetCursorPos(out var end)) _log.Info($"Ended at {end.X},{end.Y} over {DescribeWindowAt(end, warnIfOwn: false)}.");
        }
        catch (Exception ex)
        {
            _log.Info($"DoDragDrop THREW: {ex.GetType().Name}: {ex.Message}");
        }
        finally
        {
            _dragging = false;
            _armed = false;
            if (_standIn.IsVisible) _standIn.Hide();

            // One gesture per run, deliberately. Left armed, the probe fires on every later click
            // that happens to drift near the top edge, and the first run filled the log with nine
            // 60 ms attempts around the one real 3.6 s measurement — noise that reads exactly like
            // repeated failure. A measurement that can only happen once cannot be misread.
            _timer.Stop();
            _log.Info("Probe complete. Relaunch to measure again.");
        }
    }

    /// <summary>
    /// Who owns the window under the cursor. Worth a P/Invoke because the one reading that would
    /// invalidate this whole run — a press that landed on the probe itself — is otherwise
    /// indistinguishable in the log from a press that landed on another process's window.
    ///
    /// <paramref name="warnIfOwn"/> is false at the END of a drag, and the distinction is not
    /// cosmetic: this probe's own window sits under the cursor perfectly legitimately there, and
    /// a run was nearly misread as void because the same warning was printed in both places.
    /// </summary>
    private static string DescribeWindowAt(POINT p, bool warnIfOwn = true)
    {
        var hwnd = WindowFromPoint(p);
        if (hwnd == 0) return "no window";

        _ = GetWindowThreadProcessId(hwnd, out var pid);
        var mine = warnIfOwn && pid == Environment.ProcessId ? " — THIS PROCESS, the run is void" : string.Empty;

        string name;
        try
        {
            using var process = Process.GetProcessById((int)pid);
            name = process.ProcessName;
        }
        catch (ArgumentException)
        {
            name = "gone";
        }
        catch (InvalidOperationException)
        {
            name = "gone";
        }

        return $"hwnd=0x{hwnd:X} pid={pid} ({name}){mine}";
    }

    public void Dispose()
    {
        _timer.Stop();
        _standIn.Close();
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT
    {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetCursorPos(out POINT lpPoint);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern short GetAsyncKeyState(int vKey);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint WindowFromPoint(POINT point);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint GetWindowThreadProcessId(nint hWnd, out uint processId);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
}
