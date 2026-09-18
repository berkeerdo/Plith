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
    private readonly WeatherService? _weather;
    private readonly DiagnosticLog? _log;
    private TaskbarIcon? _tray;
    private SettingsWindow? _settingsWindow;

    /// <param name="weather">Passed straight through to SettingsWindow, which uses it only to
    /// explain why weather is not appearing. Optional so a caller that has no weather service
    /// still compiles rather than being forced to invent one.</param>
    /// <param name="log">The log the diagnostic bundle collects. Optional so a caller with no
    /// log still compiles; without one the bundle menu item is not offered at all, rather than
    /// offered and then producing an empty zip.</param>
    public TrayIconHost(Application app, SettingsService settings, HotkeyService hotkey, ThemeService theme, OsdHost osd,
                        WeatherService? weather = null, DiagnosticLog? log = null)
    {
        _app = app;
        _settings = settings;
        _hotkey = hotkey;
        _theme = theme;
        _osd = osd;
        _weather = weather;
        _log = log;
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

        if (_log is not null)
        {
            var bundle = new System.Windows.Controls.MenuItem { Header = "Create diagnostic bundle" };
            bundle.Click += (_, _) => CreateDiagnosticBundle();
            menu.Items.Add(bundle);
        }

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

    /// <summary>
    /// Write a zip to the Desktop and open Explorer with it selected.
    ///
    /// The work happens off the UI thread because the environment snapshot enumerates monitors
    /// and reads each one over DDC/CI, measured at 56 ms per read. On the dispatcher that would
    /// freeze the tray menu mid-click for as long as there are displays.
    /// </summary>
    private void CreateDiagnosticBundle()
    {
        var log = _log;
        if (log is null) return;

        var settings = _settings.Current.Clone();

        Task.Run(() =>
        {
            try
            {
                var now = DateTime.UtcNow;
                var text = DiagnosticEnvironment.Describe(settings, now);
                var desktop = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory);
                var zip = DiagnosticBundle.Create(desktop, log, text, now);

                log.Info("Diagnostics", "Bundle written to " + zip);

                _app.Dispatcher.BeginInvoke(new Action(() => RevealInExplorer(zip)));
            }
            catch (Exception ex)
            {
                // A diagnostic tool that crashes the app it is diagnosing is worse than one
                // that quietly fails, and the failure itself lands in the log it was collecting.
                log.Error("Diagnostics", $"Bundle failed: {ex.GetType().Name}: {ex.Message}");
            }
        });
    }

    private static void RevealInExplorer(string path)
    {
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = "/select,\"" + path + "\"",
                UseShellExecute = true,
            });
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // The file is already on the Desktop; not opening a window for it is survivable.
        }
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
        _settingsWindow = new SettingsWindow(_settings, _hotkey, _theme, _osd, _weather);
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

