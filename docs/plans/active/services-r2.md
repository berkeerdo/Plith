# R2 Services (headless) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Three headless service crates (`plith-audio`, `plith-media`, `plith-system`), each with a
typed snapshot, a change channel and a read-only probe example, proven on this PC.

**Architecture:** Each crate has a pure layer (types, decisions, state machines; unit tested, no
Windows) and a `cfg(windows)` layer (FFI and COM, thin). A service struct owns its thread(s) and
exposes `snapshot() -> Snapshot` and `subscribe() -> std::sync::mpsc::Receiver<Snapshot>` (full
snapshot per change, consumer drains to latest). No crate depends on another `plith-*` crate. The
channel helper (~40 lines) is duplicated per crate on purpose, no shared crate.

**Tech Stack:** Rust 2024, `windows` 0.62.2 (workspace pin; features added per crate), std only
otherwise. Voicemeeter DLL loaded at run time (`LoadLibraryW` + `GetProcAddress`), no link step.

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` sections 2 (G3, G5), 7, 10.

## Global Constraints
- Limits: 500 source, 150 entry, 800 test lines, 80 per function; `src/foo.rs` + `src/foo/bar.rs`,
  never `mod.rs`.
- Done = green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`,
  all through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No em dash anywhere. Code, comments, docs in English. Commit via `git commit -F <file>`.
- No mouse or key injection. No `rm` on variable paths, no `bash -c`/`eval`.
- Probes and tests **never write** to the machine. Writes exist as functions, are unit-tested only
  through fakes, and are exercised on hardware only in Task 8 after Berke's yes (via master).
- Check each `windows` API with context7 before use; add features only to the owning crate's
  `Cargo.toml` (`windows = { workspace = true, features = [...] }`).
- Voicemeeter: standard, Banana, Potato via one Remote API; counts come from the reported type.

## Review Focus
- No Voicemeeter installed / engine not running: `start` returns a "unavailable" status, never panics
  or blocks; snapshot says so. (Task 2)
- Voicemeeter engine cold start (login rc 1, 10-20 s of -1 from `IsParametersDirty`): no Logout/Login
  flapping. (Task 1 state machine test)
- Default endpoint swapped or pinned endpoint unplugged mid-run: falls back to default, re-attaches
  without deadlock (WPF deadlocked unregistering the callback under a lock). (Tasks 3, 4)
- No capture device (desktop without mic): mic snapshot is `None`, not "unmuted". (Task 4)
- SMTC with no session, a live stream (zero duration), `LastUpdatedTime` default: no division by
  zero, no bar pinned to the end. (Task 5)
- Suppressor must restore every window it hid on stop; hook proc never blocks. (Task 7)

---

### Task 1: `plith-audio` pure Voicemeeter layer
**Files:** Create `crates/plith-audio/src/voicemeeter.rs` (types), `voicemeeter/layout.rs`,
`voicemeeter/session.rs`, `voicemeeter/tests.rs`; Modify `src/lib.rs`.
**Interfaces (produces):**
- `enum VmKind { Standard, Banana, Potato }` with `from_api(i32) -> Option<VmKind>` (1,2,3),
  `strip_count() -> usize` (3,5,8), `bus_count() -> usize` (2,5,8).
- `enum Rail { Strip, Bus }`, `fn param_name(Rail, usize, &str) -> String` (`Strip[2].Gain`).
- `const MIN_GAIN_DB: f32 = -60.0; MAX_GAIN_DB: f32 = 12.0; fn clamp_gain(f32) -> f32` (NaN -> MIN).
- `fn fallback_label(Rail, usize) -> String` (`Strip 1`; bus `A1..A5,B1..B3`).
- `struct ChannelSnapshot { rail, index, label, gain_db, muted }`, `struct VmSnapshot { kind, strips, buses }`.
- `struct LoginSession` pure state machine, clock passed in (`Instant`): `on_login(rc, now)`,
  `on_dirty(rc, now) -> DirtyOutcome { Clean, Dirty, Transient, EngineGone }`.
  Grace 5 s when rc 0, 30 s when rc 1; EngineGone after 5 consecutive negatives past grace; never
  asks for Logout when declaring death.
- `fn level_to_db(f32) -> f32` (VB level is linear amplitude; 0 -> `-inf` clamped to `-90.0`).
- [x] Step 1: tests: `kind_counts`, `clamp_gain_range_and_nan`, `param_name_format`,
  `fallback_labels`, `grace_blocks_early_negatives`, `five_negatives_after_grace_is_gone`,
  `success_resets_streak`, `rc1_uses_long_grace`, `level_to_db_floor`. Run, expect compile FAIL.
- [x] Step 2: implement the signatures above. Run tests, expect PASS.
- [x] Step 3: checks green, commit `feat(audio): add Voicemeeter pure layer`, push, tick.

### Task 2: `plith-audio` Voicemeeter client and probe
**Files:** Create `src/voicemeeter/dll.rs` (discovery + `LoadLibraryW`), `voicemeeter/api.rs`
(typed fn table), `voicemeeter/service.rs`, `examples/probe_voicemeeter.rs`.
**Interfaces:** Consumes Task 1. Produces `VoicemeeterService::start() -> Status`
(`Status::{Running, Unavailable(reason)}`), `snapshot() -> Option<VmSnapshot>`,
`subscribe()`, `levels() -> VmLevels` (peak per strip/bus channel pair, read-only),
`set_gain(Rail, usize, f32) -> bool`, `toggle_mute(Rail, usize) -> Option<bool>` (read then write).
- [x] Step 1: tests for the pure parts (registry uninstall string to directory, UTF-8/Latin-1 label
  decode with NUL, `VBVMR_GetLevel` channel index mapping per kind) in `voicemeeter/tests.rs`. FAIL, then pass.
- [x] Step 2: DLL discovery: HKLM uninstall key `VB:Voicemeeter {17359A74-1236-5467}` (64 and WOW6432
  views), fallback `C:\Program Files (x86)\VB\Voicemeeter`, then default search. Resolve
  `VBVMR_Login`, `Logout`, `IsParametersDirty`, `GetParameterFloat`, `GetParameterStringA`
  (512 byte buffer), `SetParameterFloat`, `GetVoicemeeterType`, `GetLevel`. Use `stdcall`
  (`extern "system"`).
- [x] Step 3: poll thread (50 ms dirty check, level read at its own divisor), defensive Logout before
  Login only, drives `LoginSession`; snapshot rebuilt when dirty. Stop joins the thread, then Logout.
- [x] Step 4: probe prints type, every strip and bus (label, gain, mute), then levels for 3 s. Read only.
- [x] Step 5: run the probe on this PC (Banana), paste output into the record; checks green; commit
  `feat(audio): add Voicemeeter client and probe`; push; tick.

### Task 3: `plith-audio` Core Audio endpoints (read side)
**Files:** Create `src/endpoints.rs` (types, pure), `endpoints/pin.rs`, `endpoints/com.rs`
(enumerator, notifications), `endpoints/service.rs`, `endpoints/tests.rs`,
`examples/probe_endpoints.rs`.
**Interfaces:** Produces `struct Endpoint { id, friendly_name, device_name }` (both names kept, no
shortening in this crate; UI decides), `struct EndpointState { default_id, active: Vec<Endpoint>,
attached: Option<AttachedState> }`, `struct AttachedState { id, volume: f32, muted: bool }`,
`fn resolve_target(pinned: Option<&str>, active: &[Endpoint], default_id: Option<&str>) -> Option<String>`
(pinned if active, else default), `EndpointService::start()`, `snapshot()`, `subscribe()`,
`set_pin(Option<String>)`.
- [x] Step 1: tests: `pin_wins_when_active`, `pin_falls_back_when_missing`, `no_default_no_pin_is_none`,
  `pin_ids_compare_case_insensitively`. FAIL, implement, PASS.
- [x] Step 2: COM side: `MMDeviceEnumerator`, `IMMNotificationClient` (default changed for
  Render/Multimedia, state changed, added, removed), `IAudioEndpointVolumeCallback` on the attached
  endpoint. Callbacks only post a message to the service thread; **no lock held across
  `UnregisterEndpointNotificationCallback`**, the service thread owns attach/detach (removes the WPF
  deadlock by construction). Read each device fully before adding it to a list.
- [x] Step 3: probe lists endpoints, marks default and attached, prints volume and mute, then prints
  change events for 10 s while Berke changes things by hand. Read only.
- [x] Step 4: run on this PC, record output; checks; commit `feat(audio): add endpoint service`; push; tick.

### Task 4: `plith-audio` default-endpoint switch, mic mute, write paths
**Files:** Create `endpoints/policy.rs` (IPolicyConfig), `src/microphone.rs`, `microphone/tests.rs`,
`endpoints/write.rs`, `examples/probe_microphone.rs`.
**Interfaces:** `set_volume(f32) -> bool` (clamp 0..1), `toggle_mute() -> Option<bool>` on the
attached endpoint; `set_default_endpoint(id) -> bool` (Console and Multimedia only, never
Communications); `MicrophoneService::start()`, `snapshot() -> Option<MicSnapshot { id, name, muted,
volume }>` (None = no capture device), `subscribe()`, `toggle_mute() -> Option<bool>`.
- [x] Step 1: tests on pure helpers: volume clamp (NaN, negative, >1), `MicSnapshot` None when no
  device. FAIL, implement, PASS.
- [x] Step 2: IPolicyConfig `{f8679f50-850a-41cf-9c72-430f290290c8}`, CLSID
  `{870af99c-171d-4f9e-af0d-e63df40c2bc9}`, **ten** reserved vtable slots before `SetDefaultEndpoint`
  (measured in WPF: nine landed on SetPropertyValue, 0x800706F4). Capture default uses
  Communications role (as WPF).
- [x] Step 3: probe reads mic state only. Write functions are NOT called by any probe.
- [x] Step 4: checks; commit `feat(audio): add mic mute and endpoint writes`; push; tick.

### Task 5: `plith-media` SMTC
**Files:** Create `src/timeline.rs` (pure), `src/snapshot.rs`, `src/session.rs` (windows),
`src/service.rs`, `src/tests.rs`, `examples/probe_media.rs`.
**Interfaces:** `struct Timeline { position, duration, stamp: SystemTime }`;
`fn normalize(start, end, position, last_updated: Option<SystemTime>, now) -> Option<Timeline>`
(None when duration <= 0; missing stamp becomes now; start subtracted);
`fn position_at(&Timeline, playing: bool, now) -> Duration` (interpolated, clamped to duration);
`struct MediaSnapshot { has_session, source_id, title, artist, thumbnail: Option<Vec<u8>>, playing,
can_seek, can_next, can_prev, timeline: Option<Timeline> }`; `struct SessionInfo { source_id, title,
playing }`; `MediaService::start()`, `snapshot()`, `subscribe()`, `sessions() -> Vec<SessionInfo>`,
async-free commands: `toggle_play_pause`, `next`, `previous`, `seek(Duration)` each `-> bool`.
- [x] Step 1: tests: `zero_duration_is_none`, `start_time_subtracted`, `missing_stamp_uses_now`,
  `position_interpolates_only_when_playing`, `position_clamped_to_duration`. FAIL, implement, PASS.
- [x] Step 2: SMTC through `windows::Media::Control`. Traps: re-read bursts coalesced with a
  generation counter so a stale read never overwrites a newer one; thumbnails re-read only on
  media-properties change, never on the ~1 Hz timeline event (separate `timeline` event path);
  `IsPlaybackPositionEnabled` asked, not assumed; every read tolerates a dead session (None).
  Commands block on the WinRT op with a 2 s cap on a worker thread.
- [x] Step 3: probe prints sessions, current track, timeline, capabilities, then 15 s of events. Read only.
- [x] Step 4: run on this PC with something playing (Berke starts it), record; checks; commit
  `feat(media): add SMTC service and probe`; push; tick.

### Task 6: `plith-system` brightness
**Files:** Create `src/brightness.rs`, `brightness/step.rs` (pure), `brightness/writer.rs` (pure
coalescing), `brightness/ddc.rs` (dxva2), `brightness/wmi.rs`, `brightness/tests.rs`,
`examples/probe_brightness.rs`.
**Interfaces:** `trait BrightnessDevice { id() -> &str; read() -> Option<u8>; write(u8) -> bool }`;
`fn step_level(current: u8, delta: i8) -> u8` (0..=100, saturating);
`struct CoalescingWriter` (latest value wins, one write in flight, failures reported per device id
and never retried, never stop the other devices; injectable runner for tests);
`BrightnessService::start()`, `snapshot() -> Vec<DeviceLevel { id, name, percent }>`,
`subscribe()`, `set_percent(u8)`.
- [x] Step 1: tests: `step_saturates_at_bounds`, `writer_keeps_only_latest_pending`,
  `writer_one_in_flight`, `failed_device_does_not_block_others`, `discovery_keeps_only_answering`.
- [x] Step 2: DDC through dxva2 `GetNumberOfPhysicalMonitorsFromHMONITOR` + `GetMonitorBrightness`
  (a read is the capability test; do NOT gate on `GetMonitorCapabilities`, measured false on a
  working monitor); handle opened once and kept; internal panel first. WMI
  (`WmiMonitorBrightness`, `WmiMonitorBrightnessMethods.WmiSetBrightness`, namespace `root\wmi`,
  1 s timeout, backslash-escaped instance names) via the `windows` WMI/COM, event listener for
  `WmiMonitorBrightnessEvent` (internal panels only).
- [x] Step 3: probe reads each device's level only.
- [x] Step 4: run on this PC; checks; commit `feat(system): add brightness service`; push; tick.

### Task 7: `plith-system` flyout suppression and volume key hook
**Files:** Create `src/flyout.rs` (pure decision), `flyout/hook.rs` (WinEvent hooks),
`flyout/registry.rs` (hidden windows), `src/volume_keys.rs`, `src/flyout/tests.rs`,
`examples/probe_flyout.rs`.
**Interfaces:** `struct WindowFacts { class, process, band: Option<u32> }`;
`fn should_hide(&WindowFacts, window_open: bool) -> bool` (class in the five host classes, process in
`ShellExperienceHost|Explorer|WindowsInputExperience`, band in `0x4,0x10,0x11,0x12` or unknown,
open 400 ms window); `struct SuppressionWindow` (injectable clock, `open(now)`, `is_open(now)`);
`struct HiddenRegistry` (hide records the hwnd, `restore_all` calls the restore fn for each once);
`FlyoutSuppressor::start(mode: Mode::{DryRun, Hide})`, `stop()` restores every hidden window with
`SW_SHOWNA`; `VolumeKeyHook::start() -> Receiver<()>` (WH_KEYBOARD_LL, only observes
VK 0xAD/0xAE/0xAF, always `CallNextHookEx`, handler does nothing but a channel send).
- [x] Step 1: tests: `class_process_band_all_required`, `unknown_band_allowed`, `closed_window_hides_nothing`,
  `window_expires_after_400ms`, `restore_all_restores_each_once`, `restore_after_stop_is_empty`. FAIL, PASS.
- [x] Step 2: hooks on SHOW, CREATE, LOCATIONCHANGE with `OUTOFCONTEXT | SKIPOWNPROCESS`; cheapest
  filter first (window open) because LOCATIONCHANGE fires thousands of times per second; pid to
  process-name cache; `GetWindowBand` resolved with `GetProcAddress`, absence tolerated.
  Both hooks need a message-pumping thread owned by the service.
- [x] Step 3: probe runs `DryRun`: installs the hook, logs what it **would** hide when Berke presses a
  volume key by hand, hides nothing. Read only.
- [x] Step 4: checks; commit `feat(system): add flyout suppression and key hook`; push; tick.

### Task 8: hardware write verification (needs Berke's yes via master)
**Files:** Create `examples/verify_writes.rs` in each crate, each requiring `--yes` and printing what
it will change and restoring the old value afterwards.
- [ ] Step 1: send the master the list: Voicemeeter gain nudge and restore on one strip, endpoint
  volume nudge and restore, endpoint mute toggle twice, mic mute toggle twice, default endpoint
  switch and back, brightness nudge and restore, flyout suppression in `Hide` mode. Wait for the yes.
- [ ] Step 2: after the yes, run each, record before/after in the handoff. Skipped items are recorded
  as skipped.

### Task 9: handoff
- [ ] Per-crate `README.md` (public API, probe command), `docs/STATUS.md` services section and an
  ADR only if a decision shaped later work (candidate: single-owner service thread for COM).
  `docs/journal/2026-11.md` entry; message the master with commits, record path, next job (R4 pages
  consume these crates through `crates/plith`).

## Self-review
Spec coverage: audio (Voicemeeter, endpoints, pinning, mic mute) T1-4; media (SMTC, seek, sessions
list) T5; system (brightness, flyout suppression) T6-7; probes in every crate; no cross-crate deps.
Output picker data = `EndpointState.active` plus `sessions()`. Names used across tasks
(`Rail`, `VmSnapshot`, `Endpoint`, `Timeline`, `DeviceLevel`) are defined once, in the task that
produces them.

## Record (2026-11-14)

A ticked box means the step was taken, not that nothing deviated.

Deviations and findings:
- Task 2 step 5 / Voicemeeter: not installed on this PC (DLL renamed to `.del`, uninstall keys absent), so
  login, snapshot, levels, reconnect and the `GetLevel` channel map are **unverified on hardware**.
- API shape: `new()` then `start(&self) -> Status`, `stop()`, `status()`, `subscribe()`, `set_waker()` on every
  service; `subscribe()` never replays the current value (subscribe first, then `snapshot()`).
  Voicemeeter subscribers get `Option<VmSnapshot>`.
- Extra direct deps added to the root `[workspace.dependencies]` (add-only): `windows-core`, `windows-future`.
- Voicemeeter DLL loads only from absolute install paths (`LoadLibraryExW`, DLL dir plus System32).
- G5 gap, deferred: brightness flyout suppression. The suppressor opens its window on volume keys only; the
  consumer must call `open_window` on `WmiMonitorBrightnessEvent`. Not verifiable on this desktop.
- Not measured live: volume/mute/default-change callbacks, media play/pause/track change/commands, WMI read and
  write, DDC write, flyout class and band on this Windows build.
- Deferred minors: brightness never re-discovers displays (`WM_DISPLAYCHANGE`), hidden flyout windows are restored
  only at stop, Standard-bus fallback label unverified, DDC ids shift if a monitor stops answering.
- Task 8 (hardware writes): examples built (`verify_audio_writes`, `verify_system_writes`,
  `verify_media_writes`), none run with `--yes`; waiting for Berke's yes via the master.

### Task 8 results (2026-11-14, Berke present, relayed approval)

- PASS with restore: endpoint volume 0.50 to 0.52 and back (event yes), endpoint mute (event yes), mic mute
  (event yes), default endpoint Console+Multimedia to Steam Streaming Speakers and back, brightness 30 to 35 and
  back (DDC). Voicemeeter gain SKIPPED (not installed).
- Flyout Hide mode PASS on this build: 143 windows matched, 137 hidden, 0 left hidden after stop.
- **R4 design note:** the flyout run counted 135 volume key events for what was meant as one press: the
  low-level hook sees key auto-repeat. A held key must not be counted as 135 presses for the OSD (collapse
  repeats, or use the previous-key-state bit in `KBDLLHOOKSTRUCT` flags). Those presses also raised the
  endpoint volume from 50% to 100%; set back to exactly 0.50 afterwards (read back 0.5).
- Media transport PASS: play/pause toggled twice and restored (Spotify was playing), seek +5 s and back, events
  seen. Item 8 (manual volume change during the 10 s probe window) saw no change event; live callbacks are
  however proven by items 1 to 3 (event: yes). Task 8 complete.
