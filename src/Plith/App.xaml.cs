using System.Windows;
using Plith.Services;
using Plith.Views;

namespace Plith;

// CA1001: App holds disposable fields but isn't IDisposable. WPF owns the Application
// lifecycle and calls OnExit on shutdown, where we explicitly Dispose them; making App
// IDisposable doesn't fit the WPF pattern. Justified suppression.
[System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1001:Types that own disposable fields should be disposable",
    Justification = "Disposable fields are released in OnExit, matching the WPF Application lifecycle.")]
public partial class App : Application
{
    private DiagnosticLog? _diagnosticLog;
    private SettingsService? _settings;
    private TrayIconHost? _trayHost;
    private OsdOrchestrator? _orchestrator;
    private OsdHost? _osd;
    private HotkeyService? _hotkey;
    private ThemeService? _theme;
    private ForegroundWatcher? _foregroundWatcher;
    private NativeFlyoutSuppressor? _flyoutSuppressor;
    private VolumeKeyHook? _volumeKeyHook;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _diagnosticLog = new DiagnosticLog();
        _diagnosticLog.Info("App", "OnStartup begin — Plith.exe path: " + Environment.ProcessPath);

        _settings = new SettingsService();
        _settings.Load();

        // Reconcile the registry Run entry with the saved preference on every launch — the
        // user could have manually edited the INI between sessions, or moved the binary.
        AutoStartService.Apply(_settings.Current.AutoStart);

        // ThemeService must start before any Window is shown so the first paint already
        // uses the active palette; otherwise a Light user would see a one-frame dark flash.
        _theme = new ThemeService(this, _settings);
        _theme.Start();
        // The OSD viewmodel caches threshold brush references for the hot-path GainColor
        // getter; tell it to re-resolve from the active palette every time the theme swaps.
        _theme.ThemeApplied += () => _osd?.ViewModel.RefreshThresholdBrushes();

        _osd = new OsdHost(_settings);   // ctor calls CreateWindow() so first ShowOsd is instant
        _orchestrator = new OsdOrchestrator(_osd, _settings, _diagnosticLog);
        _orchestrator.Start();
        _diagnosticLog.Info("App", "OsdOrchestrator started");

        // Re-assert HWND_TOPMOST when the system foreground window changes so a game or
        // video player popping a topmost window mid-OSD doesn't steal the z-order ahead of us.
        _foregroundWatcher = new ForegroundWatcher(_osd);
        _foregroundWatcher.Start();
        _diagnosticLog.Info("App", "ForegroundWatcher started");

        // NativeFlyoutSuppressor uses a four-filter design (class + process + ZBID
        // ImmersiveNotifications + volume-event-coupled 400 ms window) so it hides ONLY
        // the volume OSD, not Start menu / taskbar / brightness / toasts. The orchestrator
        // forwards every Windows volume event into the suppressor's window opener.
        _flyoutSuppressor = new NativeFlyoutSuppressor(_diagnosticLog);
        _flyoutSuppressor.Start();
        _orchestrator.WindowsVolumeEvent += _flyoutSuppressor.OpenSuppressionWindow;
        _diagnosticLog?.Info("App", "NativeFlyoutSuppressor started");

        // Volume-key low-level hook: opens the suppression window on the KEY DOWN event,
        // several ms before Windows renders its flyout. Closes the race that the audio-
        // notification-driven trigger loses on Win11 (audio API callback arrives ~400 ms
        // after the flyout is already on-screen). Also fires the OSD immediately for a
        // pinned endpoint that the volume key doesn't actually target — better than
        // showing nothing while the user presses keys.
        _volumeKeyHook = new VolumeKeyHook(_diagnosticLog);
        _volumeKeyHook.VolumeKeyPressed += () =>
        {
            _flyoutSuppressor?.OpenSuppressionWindow();
            // Bounce to the UI dispatcher — VolumeKeyPressed runs on the Windows hook thread.
            Dispatcher.BeginInvoke(() => _osd?.ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs)));
        };
        _volumeKeyHook.Start();
        _diagnosticLog?.Info("App", "VolumeKeyHook started");

        // The summon hotkey pops the OSD with whatever values the view-model currently holds —
        // useful for one-handed media skips without touching the volume wheel. Default is None
        // (off); the user picks a combo in the settings window and we re-apply on every change.
        // _hotkey is created BEFORE _trayHost so the tray can hand the service to SettingsWindow
        // for the binding-conflict warning.
        _hotkey = new HotkeyService();
        _hotkey.Pressed += () => _osd?.ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs));
        ApplyHotkeyFromSettings(_settings.Current);
        _settings.Changed += ApplyHotkeyFromSettings;

        _trayHost = new TrayIconHost(this, _settings, _hotkey, _theme);
        _trayHost.Initialize();
    }

    private void ApplyHotkeyFromSettings(SettingsModel m)
    {
        if (_hotkey is null) return;
        if (!_hotkey.Apply(m.SummonHotkeyMods, m.SummonHotkeyKey))
        {
            System.Diagnostics.Trace.WriteLine(
                "Plith: hotkey " + HotkeyService.FormatCombo(m.SummonHotkeyMods, m.SummonHotkeyKey)
                + " unavailable — already owned by another process.");
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _diagnosticLog?.Info("App", "OnExit — begin");
        if (_settings is not null) _settings.Changed -= ApplyHotkeyFromSettings;

        // Per-step logging: shutdown hangs used to freeze silently after "OnExit". Wrapping
        // each Dispose lets the next hang report the exact culprit step in plith.log.
        DisposeStep("ForegroundWatcher", () => _foregroundWatcher?.Dispose());
        DisposeStep("VolumeKeyHook",     () => _volumeKeyHook?.Dispose());
        DisposeStep("HotkeyService",     () => _hotkey?.Dispose());
        DisposeStep("ThemeService",      () => _theme?.Dispose());
        DisposeStep("Orchestrator",      () => _orchestrator?.Dispose());
        DisposeStep("FlyoutSuppressor",  () => _flyoutSuppressor?.Dispose());
        // BandWindow.Ext.OnAppExit disposes HwndSource on Application.Exit; no manual unblock needed.
        DisposeStep("TrayIconHost",      () => _trayHost?.Dispose());

        _diagnosticLog?.Info("App", "OnExit — base.OnExit");
        base.OnExit(e);
        _diagnosticLog?.Info("App", "OnExit — Environment.Exit");
        // Force-exit before the GC finalizer thread runs — WinRT COM objects (SMTC session
        // via MediaSessionClient) crash from Finalize when the COM apartment has already
        // been torn down. Manifests as ".NET Runtime unhandled exception in
        // WinRT.IObjectReference.Finalize / GC.RunFinalizers" during shutdown. All our own
        // cleanup already ran above; skipping finalizers here loses nothing meaningful.
        Environment.Exit(e.ApplicationExitCode);
    }

    private void DisposeStep(string name, Action action)
    {
        _diagnosticLog?.Info("App", $"Disposing {name}");
        try { action(); }
        catch (Exception ex) { _diagnosticLog?.Warn("App", $"{name} threw: {ex.GetType().Name}: {ex.Message}"); }
        _diagnosticLog?.Info("App", $"Disposed {name}");
    }
}
