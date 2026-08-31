using System.Drawing;
using System.IO;
using System.Windows;
using System.Windows.Threading;
using Hardcodet.Wpf.TaskbarNotification;
using Plith.Views;

namespace Plith.Services;

public sealed class TrayIconHost : IDisposable
{
    private readonly Application _app;
    private readonly SettingsService _settings;
    private readonly HotkeyService _hotkey;
    private readonly ThemeService _theme;
    private readonly OsdHost _osd;
    private TaskbarIcon? _tray;
    private SettingsWindow? _settingsWindow;

    public TrayIconHost(Application app, SettingsService settings, HotkeyService hotkey, ThemeService theme, OsdHost osd)
    {
        _app = app;
        _settings = settings;
        _hotkey = hotkey;
        _theme = theme;
        _osd = osd;
    }

    public void Initialize()
    {
        _tray = new TaskbarIcon
        {
            ToolTipText = "Plith",
            Icon = LoadBrandIcon() ?? SystemIcons.Application,
        };

        var menu = new System.Windows.Controls.ContextMenu();

        var settingsItem = new System.Windows.Controls.MenuItem { Header = "Settings…" };
        settingsItem.Click += (_, _) => ShowSettings();
        menu.Items.Add(settingsItem);

        menu.Items.Add(new System.Windows.Controls.Separator());

        var exit = new System.Windows.Controls.MenuItem { Header = "Exit" };
        // Post Shutdown at Background priority instead of calling it inline. Calling
        // Application.Shutdown() from inside a ContextMenu.Click handler starts the
        // shutdown flow while the popup is still mid-close, which can wedge the
        // dispatcher on the tray-popup HWND teardown. Deferring one dispatcher turn
        // lets the popup fully close first, then OnExit runs cleanly.
        exit.Click += (_, _) => _app.Dispatcher.BeginInvoke(
            new Action(_app.Shutdown), DispatcherPriority.Background);
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
        _settingsWindow = new SettingsWindow(_settings, _hotkey, _theme, _osd);
        _settingsWindow.Closed += (_, _) => _settingsWindow = null;
        _settingsWindow.Show();
    }

    public void Dispose() => _tray?.Dispose();

    private static Icon? LoadBrandIcon()
    {
        try
        {
            var uri = new Uri("pack://application:,,,/Resources/icons/plith.ico", UriKind.Absolute);
            using var stream = Application.GetResourceStream(uri)?.Stream;
            if (stream is null) return null;
            using var ms = new MemoryStream();
            stream.CopyTo(ms);
            ms.Position = 0;
            return new Icon(ms);
        }
        catch
        {
            return null;
        }
    }
}

