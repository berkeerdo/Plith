using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;

namespace Plith.DropCatcher;

/// <summary>
/// A measurement, not a feature.
///
/// The inbound direction is settled: a drop cannot reach a UIAccess process, because Windows
/// raises it to High integrity and UIPI refuses Explorer's cross-integrity call. Outbound is a
/// different question with a different answer available, because the initiator changes sides —
/// here the HIGH process starts the drag and a MEDIUM one receives it. Nothing in the inbound
/// measurement predicts this, so it is measured before anything is designed on top of it.
///
/// Run elevated, so this process stands in for an installed Plith:
///   sudo Plith.DropCatcher.exe --dragout "C:\some\file.txt"
/// then drag the tile into an Explorer window and read dropcatcher.log.
/// </summary>
internal sealed class DragOutWindow : Window
{
    private readonly CatcherLog _log;
    private readonly string _path;
    private Point _pressedAt;
    private bool _pressed;

    public DragOutWindow(CatcherLog log, string path)
    {
        _log = log;
        _path = path;

        Title = "Plith drag-out probe";
        Width = 280;
        Height = 140;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Topmost = true;
        Background = new SolidColorBrush(Color.FromRgb(0x14, 0x14, 0x14));

        Content = new TextBlock
        {
            Text = "Drag me into an Explorer window\n\n" + System.IO.Path.GetFileName(path),
            Foreground = Brushes.White,
            FontSize = 13,
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(16),
            VerticalAlignment = VerticalAlignment.Center,
        };

        // Logged, because the first run of this probe produced no DoDragDrop line at all and
        // that reading is ambiguous: a press that never landed on this window looks exactly like
        // a drag that started and was refused. One line separates them.
        MouseLeftButtonDown += (_, e) =>
        {
            _pressed = true;
            _pressedAt = e.GetPosition(this);
            _log.Info($"Press on the probe at {_pressedAt.X:0},{_pressedAt.Y:0}.");
        };
        MouseLeftButtonUp += (_, _) => _pressed = false;
        MouseMove += OnMouseMove;

        _log.Info($"Drag-out probe ready with {path}. Exists={System.IO.File.Exists(path)}");
    }

    private void OnMouseMove(object sender, MouseEventArgs e)
    {
        if (!_pressed || e.LeftButton != MouseButtonState.Pressed) return;

        var now = e.GetPosition(this);
        if (Math.Abs(now.X - _pressedAt.X) < SystemParameters.MinimumHorizontalDragDistance
            && Math.Abs(now.Y - _pressedAt.Y) < SystemParameters.MinimumVerticalDragDistance)
        {
            return;
        }

        _pressed = false;

        var data = new DataObject();
        data.SetData(DataFormats.FileDrop, new[] { _path });

        _log.Info("Starting DoDragDrop...");
        try
        {
            // Blocks for the whole drag. The return value is the answer: None means nothing
            // accepted it, Copy means a target took it.
            var result = DragDrop.DoDragDrop(this, data, DragDropEffects.Copy | DragDropEffects.Link);
            _log.Info($"DoDragDrop RETURNED: {result}");
        }
        catch (Exception ex)
        {
            _log.Info($"DoDragDrop THREW: {ex.GetType().Name}: {ex.Message}");
        }
    }
}
