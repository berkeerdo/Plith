using System.Windows;
using Plith.Cards;
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
    private CardHost? _cardHost;
    private AudioCard? _audioCard;
    private MediaCard? _mediaCard;
    private MediaSessionClient? _mediaSession;
    private HotkeyService? _hotkey;
    private ThemeService? _theme;
    private ForegroundWatcher? _foregroundWatcher;
    private NativeFlyoutSuppressor? _flyoutSuppressor;
    private VolumeKeyHook? _volumeKeyHook;
    private FullscreenVideoWatcher? _fullscreenWatcher;

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
        // Cards cache brush references for hot-path getters; fan a palette/accent swap out to
        // every one of them, visible or not.
        _theme.ThemeApplied += () => _cardHost?.NotifyThemeChanged();

        // One SMTC client shared by every consumer; App owns its lifetime.
        _mediaSession = new MediaSessionClient();

        _audioCard = new AudioCard(_settings);
        _mediaCard = new MediaCard(_settings);

        _fullscreenWatcher = new FullscreenVideoWatcher(_settings, _mediaSession, Dispatcher, _diagnosticLog);

        _cardHost = new CardHost(_settings, _fullscreenWatcher);
        _cardHost.Register(_mediaCard);   // Order 10 — renders above
        _cardHost.Register(_audioCard);   // Order 20

        _osd = new OsdHost(_settings, _theme, _cardHost);   // ctor calls CreateWindow() so first ShowOsd is instant
        _cardHost.ShowRequested += d => _osd.ShowOsd(d);
        _cardHost.HideRequested += () => _osd.HideOsd();
        _cardHost.Start();

        _orchestrator = new OsdOrchestrator(_audioCard, _mediaCard, _settings, _osd.Dispatcher, _mediaSession, _diagnosticLog);
        _orchestrator.Start();
        _diagnosticLog.Info("App", "OsdOrchestrator started");
        _fullscreenWatcher.Start();   // after the orchestrator, so the first Evaluate sees a live session client

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
            Dispatcher.BeginInvoke(() => _cardHost?.RequestShow(new ShowRequest(ShowReason.VolumeKey)));
        };
        _volumeKeyHook.Start();
        _diagnosticLog?.Info("App", "VolumeKeyHook started");

        // The summon hotkey pops the OSD with whatever values the view-model currently holds —
        // useful for one-handed media skips without touching the volume wheel. Default is None
        // (off); the user picks a combo in the settings window and we re-apply on every change.
        // _hotkey is created BEFORE _trayHost so the tray can hand the service to SettingsWindow
        // for the binding-conflict warning.
        _hotkey = new HotkeyService();
        _hotkey.Pressed += () => _cardHost?.RequestShow(new ShowRequest(ShowReason.SummonHotkey));
        ApplyHotkeyFromSettings(_settings.Current);
        _settings.Changed += ApplyHotkeyFromSettings;

        _trayHost = new TrayIconHost(this, _settings, _hotkey, _theme, _osd);
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
        // Order matters: the orchestrator must stop feeding cards before CardHost deactivates
        // them, and the shared SMTC client outlives both. The watcher must stop raising
        // SuppressionChanged before CardHost — which it feeds — is disposed.
        DisposeStep("FullscreenVideoWatcher", () => _fullscreenWatcher?.Dispose());
        DisposeStep("CardHost",           () => _cardHost?.Dispose());
        DisposeStep("MediaSessionClient", () => _mediaSession?.Dispose());
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
