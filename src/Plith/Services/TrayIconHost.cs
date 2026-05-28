using System.Drawing;
using System.Windows;
using Hardcodet.Wpf.TaskbarNotification;
using Plith.Views;

namespace Plith.Services;

public sealed class TrayIconHost : IDisposable
{
    private readonly Application _app;
    private readonly SettingsService _settings;
    private TaskbarIcon? _tray;
    private SettingsWindow? _settingsWindow;

    public TrayIconHost(Application app, SettingsService settings)
    {
        _app = app;
        _settings = settings;
    }

    public void Initialize()
    {
        _tray = new TaskbarIcon
        {
            ToolTipText = "Plith",
            Icon = SystemIcons.Application, // Replace with branded icon in Phase 4.
        };

        var menu = new System.Windows.Controls.ContextMenu();

        var settingsItem = new System.Windows.Controls.MenuItem { Header = "Settings…" };
        settingsItem.Click += (_, _) => ShowSettings();
        menu.Items.Add(settingsItem);

        menu.Items.Add(new System.Windows.Controls.Separator());

        var exit = new System.Windows.Controls.MenuItem { Header = "Exit" };
        exit.Click += (_, _) => _app.Shutdown();
        menu.Items.Add(exit);

        _tray.ContextMenu = menu;
        _tray.TrayMouseDoubleClick += (_, _) => ShowSettings();
    }

    private void ShowSettings()
    {
        // IsVisible — not IsLoaded — is the right sentinel: IsLoaded stays true forever
        // once a window has been into the visual tree, even after Close(). Using IsVisible
        // also closes the double-click race (the second click sees a visible window and
        // just activates it instead of opening a second instance).
        if (_settingsWindow is { IsVisible: true })
        {
            _settingsWindow.Activate();
            return;
        }
        _settingsWindow = new SettingsWindow(_settings);
        _settingsWindow.Closed += (_, _) => _settingsWindow = null;
        _settingsWindow.Show();
    }

    public void Dispose() => _tray?.Dispose();
}
