# Phase 4g — Force-topmost + Boot Race Recovery

**Date:** 2026-05-29
**Status:** Approved design, ready for writing-plans

## Problem statement

Two distinct, user-reported bugs:

1. **OSD doesn't appear over fullscreen games.** Works in borderless windowed mode for most titles but loses the z-order race against focused topmost game windows. True-exclusive-fullscreen (Valorant / CS2 with anti-cheat) is out of scope for this phase.
2. **Boot race:** when Plith auto-starts at Windows login the tray icon comes up but the OSD never pops, even after spinning the volume wheel. Manually killing Plith and re-launching restores correct behaviour. Symptom is consistent with the Windows audio subscription registering before the audio service is fully ready, so the COM callback never fires.

## Goals

- Solve the borderless-fullscreen z-order race so OSD reliably pops over modern AAA / esports titles set to borderless mode.
- Solve the boot race so users don't have to babysit Plith after every reboot.
- Keep the change small and reversible — no installer change, no manifest change, no new external dependencies, no anti-cheat surface.

## Non-goals

- True-exclusive-fullscreen support (Valorant, CS2 in fullscreen). That needs a full BandWindow + UIAccess + WiX-installer-to-Program-Files path, which is its own phase ("Phase 4h Game Mode") and carries anti-cheat-flagging risk we don't want to incur until A is validated.
- Voicemeeter retry behaviour — `OsdOrchestrator.TryConnectVoicemeeter` already runs every 3 s, no changes needed.

## Approach

### Component 1 — Force-topmost in OsdWindow

`OsdWindow` sets `Topmost = true` once in the constructor. WPF maps that to `WS_EX_TOPMOST` at first show, but does not re-assert on subsequent shows. A game window that later raises itself as topmost can sit above us.

**Fix:** after `Reposition()` in `OsdWindow.ShowOsd`, call

```
SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0,
             SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
```

`SWP_NOACTIVATE` keeps the game's focus untouched. Cheap when nothing has changed.

### Component 2 — Foreground change watcher

Even with the per-show re-assert, the OSD can be visible for ~2 s and another topmost window can pop in front during that window. A foreground watcher closes the gap:

- New `ForegroundWatcher` service. Single hook via `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ..., WINEVENT_OUTOFCONTEXT)`.
- When the callback fires AND `OsdWindow.Opacity > 0` AND foreground is not our own window, schedule a UI-dispatched `SetWindowPos(HWND_TOPMOST)` call.
- Throttle to one re-assert per ~150 ms via a one-shot `DispatcherTimer` so a burst of foreground events doesn't spam SetWindowPos.
- Disposed in `App.OnExit` — `UnhookWinEvent` plus GC handle release for the delegate.

### Component 3 — Windows audio re-subscribe watchdog

`WindowsAudioClient.Start()` currently succeeds at boot but events never arrive — classic boot race where `Audiosrv` is "running" but the default endpoint isn't fully wired.

**Fix:** subscribe to `IMMNotificationClient.OnDefaultDeviceChanged` (the device-level notification, not the volume one). When the default playback endpoint changes — which is what happens when the audio stack finishes coming up after boot — `WindowsAudioClient` re-subscribes its `IAudioEndpointVolume::RegisterControlChangeNotify` against the new default device.

NAudio's `MMDeviceEnumerator.RegisterEndpointNotificationCallback` is the C# entry point.

**Backup watchdog (in `OsdOrchestrator`):** if the active source is Windows AND no volume notification has fired within 10 s of the activation, call `_windowsAudio.Stop(); _windowsAudio.Start();` once. This catches the case where the device callback also misses the boot race. Logged once per heal so future debugging is possible.

## Lifecycle / data flow

```
App.OnStartup
 ├─ creates ThemeService, OsdWindow, HotkeyService, TrayIconHost (unchanged)
 ├─ creates ForegroundWatcher(_osd) ── new
 └─ creates OsdOrchestrator(_osd, _settings)
     └─ Start():
        - Polls VM (unchanged)
        - Starts WindowsAudioClient + arms 10 s watchdog timer ── new
        - WindowsAudioClient registers both
          - per-endpoint volume callback (existing)
          - DefaultDeviceChanged callback (new); on fire, re-Start
```

## Error handling

- `SetWindowPos` returning false is benign and silent — the next show will retry.
- `SetWinEventHook` returning 0 (hook failed to register) is logged once via `Trace.WriteLine` and the watcher quietly stays off — no crash, just no foreground tracking. Plith's per-show re-assert still covers borderless games for the common case.
- COM exceptions inside `IMMNotificationClient` callbacks are caught and swallowed at the boundary (same pattern as the existing volume callback) — these fire on a COM MTA thread and an exception there crashes the app.

## Testing

- **Force-topmost (manual smoke):** start a borderless-windowed game, spin volume — OSD pops on top of the game. Alt-tab out and back — OSD still re-asserts. Repeated rapid foreground switches don't spam SetWindowPos (verified with PerfView/ETW or by adding a temporary counter).
- **Boot race recovery (manual smoke):** reboot PC. Wait for Plith tray icon. Spin volume — OSD pops within the watchdog window (≤ 10 s). Test both the `DefaultDeviceChanged` path (unplug + re-plug default audio device) and the watchdog path (cold-boot only).
- **No unit tests** for either component — both are Win32 / COM glue with no pure-logic surface that's worth mocking. Existing unit tests (36/36) must stay green.

## Risk / open questions

- **`SetWinEventHook` thread affinity:** `WINEVENT_OUTOFCONTEXT` callbacks fire on the thread that called the hook. We'll call from the UI dispatcher so the callback IS the UI thread — no marshalling needed. Verify before shipping.
- **Watchdog false-fire on a genuinely silent machine:** if the user has no audio activity for 10 s after boot AND the volume control happens to also be silent, the watchdog will Stop+Start once. That's harmless — re-subscribe is idempotent and the user sees nothing.
- **OSD re-asserting topmost during fade-out:** only re-assert while `Opacity > 0.01` to avoid pulling a fading-out OSD back to topmost on a foreground change that arrives mid-fade.

## Out of scope

Phase 4h (separate spec, if Phase 4g is insufficient):
- BandWindow / `CreateWindowInBand` for above-fullscreen z-band
- `uiAccess="true"` manifest entry
- Code-signing OR WiX installer that drops Plith into `\Program Files\`
- Anti-cheat compatibility audit
