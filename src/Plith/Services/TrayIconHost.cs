using System.Drawing;
using System.Windows;
using Hardcodet.Wpf.TaskbarNotification;

namespace Plith.Services;

public sealed class TrayIconHost : IDisposable
{
    private readonly Application _app;
    private TaskbarIcon? _tray;

    public TrayIconHost(Application app) => _app = app;

    public void Initialize()
    {
        _tray = new TaskbarIcon
        {
            ToolTipText = "Plith",
            Icon = SystemIcons.Application, // Replace with branded icon in Phase 4.
        };

        var menu = new System.Windows.Controls.ContextMenu();
        var exit = new System.Windows.Controls.MenuItem { Header = "Exit" };
        exit.Click += (_, _) => _app.Shutdown();
        menu.Items.Add(exit);

        _tray.ContextMenu = menu;
    }

    public void Dispose() => _tray?.Dispose();
}
