using System.Windows;
using Plith.Cards;
using Plith.Services;
using Plith.Services.Brightness;
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
    private readonly NotchHomeState _home = new();
    private AudioCard? _audioCard;
    private MediaCard? _mediaCard;
    private AmbientCard? _ambientCard;
    private BrightnessCard? _brightnessCard;
    private BrightnessMonitor? _brightnessMonitor;
    private BrightnessWriter? _brightnessWriter;
    private IReadOnlyList<IBrightnessDevice> _brightnessDevices = [];
    private HotkeyService? _brightnessUpHotkey;
    private HotkeyService? _brightnessDownHotkey;
    private BrightnessRunLog? _brightnessRunLog;
    private System.Windows.Threading.DispatcherTimer? _brightnessRunTimer;
    private bool _loggedNoBrightnessDevices;
    private readonly BrightnessLevelCache _brightnessLevel = new();
    private OpenMeteoClient? _weatherClient;
    private WindowsLocationProvider? _windowsLocation;
    private IpLocationProvider? _ipLocation;
    private WeatherService? _weatherService;
    private MicrophoneClient? _microphone;
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
        _weatherClient = new OpenMeteoClient(_diagnosticLog);
        _windowsLocation = new WindowsLocationProvider(_diagnosticLog);
        _ipLocation = new IpLocationProvider(_diagnosticLog);
        _weatherService = new WeatherService(_settings, _weatherClient, _windowsLocation, _ipLocation, _diagnosticLog);
        _ambientCard = new AmbientCard(_home, _settings, _weatherService);
        _brightnessCard = new BrightnessCard(_settings);

        _fullscreenWatcher = new FullscreenVideoWatcher(_settings, _mediaSession, Dispatcher, _diagnosticLog);

        _cardHost = new CardHost(_settings, _fullscreenWatcher, line => _diagnosticLog?.Info("Cards", line));
        _cardHost.Register(_ambientCard);  // Order 5 — the notch's ambient row, above media
        _cardHost.Register(_mediaCard);   // Order 10 — renders above
        _cardHost.Register(_audioCard);   // Order 20
        _cardHost.Register(_brightnessCard);  // Order 30, below audio, and only while it has something to say

        _osd = new OsdHost(_settings, _theme, _cardHost, _home);   // ctor calls CreateWindow() so first ShowOsd is instant
        _cardHost.ShowRequested += (reason, d) => _osd.ShowOsd(d, reason: reason);
        _cardHost.HideRequested += () => _osd.HideOsd();
        // Suppression reaches CardHost by injection above; this is the separate signal the
        // notch needs. Deliberately not routed through IShowSuppressor: that means "do not
        // show at all", while this means "retract the strip and behave like Classic".
        _fullscreenWatcher.ForegroundCoversMonitorChanged += _osd.OnForegroundCoversMonitorChanged;
        _cardHost.Start();
        // Constructed and started here, on the UI thread, and never off it: WeatherService's
        // refresh chain calls WindowsLocationProvider.GetAsync, whose Geolocator.RequestAccessAsync
        // is UI-thread-only, and can reach SettingsService.Save (via GeocodeAndCacheAsync) and
        // AmbientCard's Tick, both of which must run on the dispatcher too. See WeatherService.
        _weatherService.Start();

        // The capture endpoint. Start() failing is an ordinary outcome rather than an error - a
        // desktop with no microphone reports nothing, and Current staying null is how the notch
        // knows to show no mic mark at all instead of an unmuted mic nobody has.
        _microphone = new MicrophoneClient(_diagnosticLog);
        _microphone.Start();

        _orchestrator = new OsdOrchestrator(_audioCard, _mediaCard, _settings, _osd.Dispatcher, _mediaSession, _diagnosticLog);
        _orchestrator.Start();
        _diagnosticLog.Info("App", "OsdOrchestrator started");

        // The notch's audio widget needs both halves at once: the view model the card already
        // paints from, and a way to write back. Wired here rather than in OsdHost's constructor
        // because the orchestrator needs that window's Dispatcher to exist first, so there is
        // nothing to hand over until now.
        // Display brightness, over DDC/CI. Constructed here but NOT started here: the query is
        // I2C traffic down the display cable, tens of milliseconds at best and occasionally far
        // worse, and a slow cable would become a visible pause at startup. It is asked on a
        // background thread, and the page appears if and when the display answers.
        _osd.AttachAudioSource(_audioCard.Vm, _orchestrator.TrySetNormalizedVolume, _mediaCard.Vm,
                               () => _weatherService.Current,
                               _orchestrator.TryToggleMute,
                               () => _mediaSession.TryOpenSourceApp(),
                               () => _microphone.Current,
            brightness: _brightnessCard.Vm);


        // Marshalled, because the endpoint's notification arrives on a COM thread.
        var osdForMic = _osd;
        _microphone.Changed += _ => osdForMic.Dispatcher.BeginInvoke(new Action(osdForMic.OnMicrophoneChanged));

        // The notch's weather page reads the snapshot when it comes on screen, which is not
        // enough on its own: the first fetch needs a location lookup and a network round trip,
        // so a page opened before that lands would show "Weather unavailable" and never correct
        // itself. AmbientCard used to be the only subscriber, and slice 3 made it unreachable.
        //
        // Marshalled onto the OSD's dispatcher because Updated is raised from the refresh's
        // continuation, which is not on the UI thread.
        var osd = _osd;
        _weatherService.Updated += () => osd.Dispatcher.BeginInvoke(new Action(osd.OnWeatherUpdated));
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

        StartBrightness();

        _trayHost = new TrayIconHost(this, _settings, _hotkey, _theme, _osd, _weatherService, _diagnosticLog);
        _trayHost.Initialize();
    }

    /// <summary>
    /// Bring up both halves of brightness: the WMI watcher that notices a change on an
    /// internal panel, and the writer plus hotkeys that make one on an external monitor.
    /// </summary>
    private void StartBrightness()
    {
        if (_settings is null || _osd is null) return;

        // Discovery costs one DDC/CI read per attached monitor, measured at 56 ms each, so it
        // happens here rather than per key press. It can legitimately find nothing: inside a
        // Remote Desktop session no physical display is reachable at all. See
        // EnsureBrightnessDevices.
        // One line when a run of key presses starts and one summary when it ends, rather than
        // a line per write. A write is 56 ms, so a held key produces roughly fifteen a second
        // and per-write logging would flood the file during the one gesture worth reading.
        _brightnessRunTimer = new System.Windows.Threading.DispatcherTimer();
        _brightnessRunTimer.Tick += (_, _) =>
        {
            _brightnessRunTimer.Stop();
            _brightnessRunLog?.Close();

            // The level is only cached for the length of a gesture. Someone can change
            // brightness from the monitor's own buttons and nothing tells us, so holding the
            // value past the run would let it drift away from the screen.
            _brightnessLevel.Invalidate();
        };
        _brightnessRunLog = new BrightnessRunLog(
            line => _diagnosticLog?.Info("Brightness", line),
            (after, _) =>
            {
                _brightnessRunTimer.Stop();
                _brightnessRunTimer.Interval = after;
                _brightnessRunTimer.Start();
            });

        _brightnessDevices = BrightnessDiscovery.Discover();
        _brightnessWriter = NewBrightnessWriter(_brightnessDevices);
        _diagnosticLog?.Info("Brightness", $"Discovery found {_brightnessDevices.Count} device(s).");

        _brightnessMonitor = new BrightnessMonitor(_osd.Dispatcher, _diagnosticLog);
        _brightnessMonitor.Changed += percent => _brightnessCard?.Report(percent);
        _brightnessMonitor.Start();

        ApplyBrightnessHotkeys(_settings.Current);
        _settings.Changed += ApplyBrightnessHotkeys;
    }

    /// <summary>The writer, with its result routed to the card on the dispatcher. The pump runs
    /// on the thread pool, and CardHost's own documentation names an off-dispatcher card update
    /// as the expected cause of a crash inside the WPF binding engine.</summary>
    private BrightnessWriter NewBrightnessWriter(IReadOnlyList<IBrightnessDevice> devices)
    {
        var writer = new BrightnessWriter(devices);
        writer.Wrote += value => _osd?.Dispatcher.BeginInvoke(
            new Action(() => _brightnessCard?.Report(ToBrightnessPercent(value))));

        // Refusals arrive on the pump thread and the run log is only ever touched on the
        // dispatcher, alongside the steps it is summarising.
        writer.Refused += id => _osd?.Dispatcher.BeginInvoke(
            new Action(() => _brightnessRunLog?.NoteRefusal(id)));

        return writer;
    }

    /// <summary>
    /// The devices, rediscovering them first if there are none.
    ///
    /// Measured: moving a session from the console to Remote Desktop takes every DDC/CI capable
    /// display away mid-session, and moving back returns it. Discovery that ran only at startup
    /// would leave the feature dead until a restart for anyone who connects to their desktop
    /// remotely and later sits back down at it.
    ///
    /// Retried here rather than from a WM_DISPLAYCHANGE and WM_WTSSESSION_CHANGE listener,
    /// which is the fuller answer and needs a message window this slice does not have. The cost
    /// of this version is one enumeration per key press while the list is empty, and nothing at
    /// all once it is not.
    /// </summary>
    private IReadOnlyList<IBrightnessDevice> EnsureBrightnessDevices()
    {
        if (_brightnessDevices.Count > 0) return _brightnessDevices;

        _brightnessDevices = BrightnessDiscovery.Discover();
        if (_brightnessDevices.Count == 0) return _brightnessDevices;

        _diagnosticLog?.Info("Brightness", $"Rediscovery found {_brightnessDevices.Count} device(s).");
        _brightnessLevel.Invalidate();

        // The writer holds the list it was built with, so a new set needs a new writer.
        _brightnessWriter = NewBrightnessWriter(_brightnessDevices);
        return _brightnessDevices;
    }

    /// <summary>
    /// The first device's value expressed as 0 to 100, because the card shows one number while
    /// every display is written together. On two monitors reporting different ranges the OSD is
    /// exact about the first and approximate about the rest, which is a display inaccuracy
    /// rather than a control bug. Recorded in the spec as the known limit of this slice.
    /// </summary>
    private int ToBrightnessPercent(int value)
    {
        // From the cached range rather than a fresh read. This runs on every write, and a read
        // here was the third DDC/CI round trip of a single key press: 60 ms spent re-asking the
        // monitor for a minimum and a maximum that cannot change while it is plugged in.
        if (!_brightnessLevel.TryGet(out var reading))
        {
            if (_brightnessDevices.Count == 0) return value;
            if (!_brightnessDevices[0].TryRead(out reading)) return value;
            _brightnessLevel.Set(reading);
        }

        var span = reading.Max - reading.Min;
        if (span <= 0) return 100;
        return (int)Math.Round((value - reading.Min) * 100.0 / span);
    }

    /// <summary>
    /// Bind or unbind the two brightness keys.
    ///
    /// Deliberately NOT conditioned on any device having been found. Inside a Remote Desktop
    /// session none can be, and a binding that only appeared after a restart would strand the
    /// person who walks back to their machine. The key press is also what triggers rediscovery.
    /// </summary>
    private void ApplyBrightnessHotkeys(SettingsModel m)
    {
        if (!m.BrightnessEnabled)
        {
            _brightnessUpHotkey?.Apply(0, 0);
            _brightnessDownHotkey?.Apply(0, 0);
            return;
        }

        // noRepeat: false, so holding the key keeps moving the value. The coalescing writer is
        // what makes that safe at 56 ms per write.
        _brightnessUpHotkey ??= BuildBrightnessHotkey(hotkeyId: 2, up: true);
        _brightnessDownHotkey ??= BuildBrightnessHotkey(hotkeyId: 3, up: false);

        // Logged, because "did the key even bind" was a question the log could not answer and
        // it is the first thing to ask when a direction does nothing. Windows refuses a combo
        // another process already owns, and says so only through this return value.
        var boundUp = _brightnessUpHotkey.Apply(m.BrightnessUpHotkeyMods, m.BrightnessUpHotkeyKey);
        var boundDown = _brightnessDownHotkey.Apply(m.BrightnessDownHotkeyMods, m.BrightnessDownHotkeyKey);

        _diagnosticLog?.Info("Brightness",
            $"Hotkeys bound: brighter {HotkeyService.FormatCombo(m.BrightnessUpHotkeyMods, m.BrightnessUpHotkeyKey)}={boundUp}"
            + (boundUp ? "" : $" (err {_brightnessUpHotkey.LastError})")
            + $", dimmer {HotkeyService.FormatCombo(m.BrightnessDownHotkeyMods, m.BrightnessDownHotkeyKey)}={boundDown}"
            + (boundDown ? "" : $" (err {_brightnessDownHotkey.LastError})")
            // Only when something actually failed. A note about an error code printed next to
            // two successes is noise in the one file that has to stay readable.
            + (boundUp && boundDown ? "." : ". Error 1409 means another window already owns that combination."));
    }

    private HotkeyService BuildBrightnessHotkey(int hotkeyId, bool up)
    {
        var service = new HotkeyService(hotkeyId, noRepeat: false);
        service.Pressed += () => StepBrightness(up);
        return service;
    }

    private void StepBrightness(bool up)
    {
        var devices = EnsureBrightnessDevices();

        if (devices.Count == 0 || _brightnessWriter is null || _settings is null)
        {
            // Logged once per dry spell rather than per press. Held down, this path runs as
            // fast as the key repeats, and the reader only needs to know the key arrived and
            // had nothing to write to.
            if (!_loggedNoBrightnessDevices)
            {
                _loggedNoBrightnessDevices = true;
                _diagnosticLog?.Info("Brightness",
                    $"{(up ? "Brighter" : "Dimmer")} pressed with no display answering. "
                    + "Inside a Remote Desktop session this is expected.");
            }
            return;
        }

        _loggedNoBrightnessDevices = false;

        // The first device is the one the step is measured from. They all move together, so a
        // second display with a different span follows rather than leads.
        //
        // Read only when the cache is empty, which means once per gesture. A read costs the
        // same 60 ms as a write, so asking before every step made a held key half as fast as
        // the hardware allows and a single press twice as slow as it needed to be.
        if (!_brightnessLevel.TryGet(out var reading))
        {
            if (!devices[0].TryRead(out reading))
            {
                _diagnosticLog?.Info("Brightness", $"{devices[0].Id} stopped answering a read.");
                return;
            }

            _brightnessLevel.Set(reading);
        }

        var next = BrightnessStep.Next(reading, _settings.Current.BrightnessStepPercent, up);
        _brightnessLevel.NoteWritten(next);
        _brightnessRunLog?.Step(up, reading.Current, next);
        _brightnessWriter.Request(next);
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
        DisposeStep("FullscreenVideoWatcher", () =>
        {
            if (_fullscreenWatcher is not null && _osd is not null)
                _fullscreenWatcher.ForegroundCoversMonitorChanged -= _osd.OnForegroundCoversMonitorChanged;
            _fullscreenWatcher?.Dispose();
        });
        DisposeStep("BrightnessRunLog",   () => { _brightnessRunTimer?.Stop(); _brightnessRunLog?.Close(); });
        DisposeStep("BrightnessMonitor",  () => _brightnessMonitor?.Dispose());
        DisposeStep("BrightnessHotkeys",  () => { _brightnessUpHotkey?.Dispose(); _brightnessDownHotkey?.Dispose(); });
        DisposeStep("CardHost",           () => _cardHost?.Dispose());
        // After CardHost: AmbientCard.Deactivate() unsubscribes from _weatherService.Updated
        // as part of that Dispose, so the service must still be alive when it runs.
        DisposeStep("BrightnessDevices",  () => { foreach (var d in _brightnessDevices) (d as IDisposable)?.Dispose(); });
        DisposeStep("WeatherService",     () => _weatherService?.Dispose());
        DisposeStep("MicrophoneClient",   () => _microphone?.Dispose());
        DisposeStep("OpenMeteoClient",    () => _weatherClient?.Dispose());
        DisposeStep("IpLocationProvider", () => _ipLocation?.Dispose());
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
