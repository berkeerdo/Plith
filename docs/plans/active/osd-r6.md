# R6 OSD Helper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** `plith-osd.exe`, a signed UIAccess helper under Program Files that draws the volume,
brightness and mute OSD over exclusive fullscreen and elevated windows, fed by `plith.exe` over a
versioned named pipe; plus the `plith-system` hardening the OSD depends on.

**Architecture:** The spike (Tasks 1 and 2) decides the drawing path before anything is built on
it: a DComp swap chain from `plith-render` (`Gpu` + `CompositionTarget`, both already public) in a
top-level window moved into the UIAccess z-band; fallback is Direct2D into a layered window.
`plith-ipc` holds a pure protocol (encode and decode, unit tested) and a thin `cfg(windows)` pipe
server and client. `plith-osd` keeps its state machine pure (`model`) and its Win32, render loop
and log thin. `plith-system` gets panic containment in both hook procs, a restore that survives
forced exits, repeat collapse and brightness flyout suppression.

**Tech Stack:** Rust 2024, `windows` 0.62.2 (workspace pin, features per crate), std only.
PowerShell 7 scripts, `signtool` from Windows Kits 10.0.22621, the existing `CN=Plith Self-Signed`
certificate (thumbprint `144A7919...AC0F`, already in `CurrentUser\My`, `LocalMachine\Root` and
`LocalMachine\TrustedPublisher`, so no import is needed).

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` sections 6 (processes) and 10 (OSD
rendering, signing); ADR 0002 (integrity split), 0005 and 0008 (swap chain: 2 buffers,
`FLIP_SEQUENTIAL`, latency 2, `Present(1, 0)`). WPF reference: `src/Plith/Interop/BandWindow/`,
`docs/PHASE5-VERIFICATION.md` (Valorant note), `docs/PHASE6-VERIFICATION.md` sections 14 and on.

## Global Constraints
- Limits: 500 source, 150 entry (`main.rs` composes only), 800 test, 80 per function.
  `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`. Pure logic gets unit tests (TDD).
- Done per task = green through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`: `cargo fmt --all
  --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`. Then commit (subject max 50
  chars, `git commit -F <file>`), push `feature/osd-r6`, tick the box.
- Owned paths only: `crates/plith-osd`, `crates/plith-ipc`, `crates/plith-system`, new
  `scripts/*osd*`, this plan. Root `Cargo.toml` and `clippy.toml` add-only. `plith-render` and
  `plith-core` changes go to the master as requests.
- `plith-osd` may depend on `plith-render`, `plith-ipc`, `plith-core` (check-deps rules).
- Check every `windows` API with context7 before use. Undocumented user32 exports
  (`CreateWindowInBand`, `SetWindowBand`, `GetWindowBand`) are resolved at run time with
  `GetProcAddress`, never linked, as `flyout/hook.rs` does for `GetWindowBand`.
- No em dash. English everywhere; no user-facing strings in this job.
- No mouse or key injection. No `rm` on variable paths, no `bash -c`/`eval`, never `bash` from
  the PowerShell tool. Subagent briefs repeat these.
- Hardware and machine writes wait for the master's relayed yes: installing under Program
  Files (`sudo`), any certificate change, flyout suppression in Hide mode, brightness writes.
  Tests and examples default to read-only or dry-run.
- On-screen runs (spike, final check) only in a console session with Berke present, asked
  through the master and batched.

## Review Focus
- Helper started twice (notch restart, a stale helper): the second exits 0 at once, the first
  keeps serving. Test in Task 4 (`FILE_FLAG_FIRST_PIPE_INSTANCE`, `bind` returns `AlreadyRunning`).
- The notch dies or is killed: the helper hides and exits within 2 s, never stays as an orphan
  over a game. Test in Task 7 (`model` exit rule) and Task 9 check 9.4.
- A volume key held down (auto-repeat, 135 events for one press): one OSD signal per press, the
  suppression window still kept open for the whole hold. Test in Task 5.
- A panic or a kill while the flyout is hidden: the Windows volume flyout comes back (now via
  `catch_unwind`, `RestoreHandle`, or the journal on the next start). Tests in Task 6.
- A malformed, truncated, newer-version or unknown-kind message: decoded to an error, logged
  once, the connection survives (unknown kind) or is dropped (bad magic or version); never a
  panic. Tests in Task 3.

---

### Task 1: UIAccess manifest, build, sign and install scripts
**Files:** Create `crates/plith-osd/build.rs`, `crates/plith-osd/plith-osd.manifest`,
`scripts/build-osd.ps1`, `scripts/install-osd.ps1`; Modify `crates/plith-osd/README.md`.
**Interfaces (produces):** env `PLITH_OSD_UIACCESS=1` at build time embeds `uiAccess='true'`;
unset embeds `'false'` (dev runs and `cargo test` keep working). Output
`target\release\plith-osd.exe`. Install path `C:\Program Files\Plith\osd\plith-osd.exe` (a new
subfolder; the WPF files beside it are not touched).
- [x] **Step 1:** `plith-osd.manifest`: PerMonitorV2 DPI, Windows 10/11 `supportedOS`,
  `longPathAware`, no `trustInfo` (the linker writes it). `build.rs` emits
  `cargo:rerun-if-env-changed=PLITH_OSD_UIACCESS`, `cargo:rerun-if-changed=plith-osd.manifest`
  and, for MSVC targets only, `cargo:rustc-link-arg-bins=` `/MANIFEST:EMBED`,
  `/MANIFESTINPUT:<abs path>`, `/MANIFESTUAC:level='asInvoker' uiAccess='<true|false>'`.
- [x] **Step 2:** Verify: build twice (with and without the env), extract with
  `mt.exe -inputresource:target\release\plith-osd.exe;#1 -out:<job tmp>\m.xml`; expected
  `uiAccess="true"` and `"false"` respectively, PerMonitorV2 present in both.
- [x] **Step 3:** `build-osd.ps1`: sets the env, runs `cargo build --release -p plith-osd` through
  heavy-run, finds the cert by subject (`CN=Plith Self-Signed`, valid, has private key; **fails**
  instead of creating one), finds signtool as `sign-plith.ps1` does, signs with
  `/fd SHA256 /tr http://timestamp.digicert.com /td SHA256`, then checks
  `Get-AuthenticodeSignature` is `Valid`. Prints the exe path and thumbprint.
- [x] **Step 4:** `install-osd.ps1 [-Yes]`: without `-Yes` prints the copy it would make and
  exits 0. With `-Yes` (run under `sudo`, only after the master's yes): stops a running
  `plith-osd` (by process name, from that exact path only), copies the exe, re-checks the
  signature at the destination. `-Uninstall -Yes` removes only `...\Plith\osd\plith-osd.exe` and
  the empty `osd` folder (literal paths).
- [x] **Step 5:** Run `build-osd.ps1` (no install). Five checks green, commit
  `build(osd): uiaccess manifest, sign, install`.

### Task 2: Spike: DComp swap chain in a UIAccess band window
**Files:** Create `crates/plith-osd/src/band.rs` (window creation, band resolve and report),
`crates/plith-osd/src/spike.rs`, `crates/plith-osd/src/log.rs`, `crates/plith-osd/src/args.rs`
(+ `args/tests.rs`); Modify `src/main.rs`, `Cargo.toml` (windows features: Foundation,
Graphics_Gdi, UI_WindowsAndMessaging, System_LibraryLoader, System_Threading, HiDpi).
**Interfaces (produces):**
- `enum Creation { InBand, SetBand, Plain }`; `fn create_band_window(rect: RectPx, creation:
  Creation) -> Result<BandWindow, String>`; `BandWindow { hwnd, band: Option<u32>, creation }`.
  Ex-style `TOOLWINDOW | NOACTIVATE | TOPMOST | TRANSPARENT | NOREDIRECTIONBITMAP`, style
  `WS_POPUP`. Band target `ZBID_UIACCESS = 2`. `InBand` calls `CreateWindowInBand`; `SetBand`
  creates with `CreateWindowExW` then `SetWindowBand(hwnd, 0, 2)` (the WPF path); `Plain` does
  neither. After creation `GetWindowBand` is read back and logged.
- `log::Log::open() -> Log` appends `YYYY-MM-DD HH:MM:SS.mmm <line>` to
  `%LOCALAPPDATA%\Plith\plith-osd.log`; never panics (falls back to no file).
- `args::parse(&[String]) -> Result<Args, String>`: `--spike [inband|setband|plain]` (default
  `inband`), `--seconds N` (1 to 120, default 20), `--pipe NAME`.
- [x] **Step 1 (TDD):** `args/tests.rs`: `spike_defaults_to_inband_20s`,
  `seconds_out_of_range_is_rejected` (0 and 121), `unknown_flag_is_rejected`.
- [x] **Step 2:** `spike.rs`: one `Gpu::new(false)` and one `CompositionTarget::new` on the band
  window (360 x 96 DIP at the primary monitor's top centre, `Levers::default()`), clear to
  transparent, draw a rounded pill and a level bar that sweeps 0 to 100 % once per second, present
  every frame on the DComp clock (`open_clock(ClockKind::Dcomp, ..)`). Log a `spike` line at start
  (creation, band read back, `IsImmersiveProcess`, token UIAccess flag via `GetTokenInformation
  (TokenUIAccess)`, warp) and a `motion`-shaped summary line every second (frames, fps, worst gap,
  present p99). Exits after `--seconds`.
- [x] **Step 3:** Dev run, unsigned, `--spike plain --seconds 3` on this desktop: the log shows
  creation and frame lines, exit 0. (Not on screen with a game; that is Task 9's ask.)
- [x] **Step 4:** Five checks green, commit `feat(osd): band window spike`, push.
- [ ] **Step 5: Ask the master (batch A, see end).** Partly done 2026-11-16 (signed, over RDP): `inband` reads back band 2 with uiaccess=true and the DComp target presents; `setband` is refused (Access denied) even with UIAccess, so the app uses `InBand`. The visual cells (a) to (e) move to Task 9 (Berke: the game run is the last step of R6, not a gate). Spike matrix, release build signed and
  installed: `inband` and `setband` over (a) the desktop, (b) an elevated window (elevated
  Task Manager), (c) a borderless game, (d) the same game in fullscreen with Fullscreen
  Optimizations on, (e) a true exclusive fullscreen game (FSO off). Record per cell: visible
  (Berke's eye), band read back, fps line. **Outcome rule:** if `inband` or `setband` shows in
  (a) to (d) at 240 fps, DComp is the path (ADR 0009 records it). If neither shows in (b) or (d),
  build Task 2b. Cell (e) failing alone is recorded as a limit (WPF saw the same with Valorant),
  not a fallback trigger, since a layered window cannot beat scan-out either.

### Task 2b (only if the spike fails): layered-window fallback
**Files:** Create `crates/plith-osd/src/layered.rs`. `WS_EX_LAYERED` top-level window in the
band, Direct2D `ID2D1DCRenderTarget` into a 32-bit DIB, `UpdateLayeredWindow` with
`ULW_ALPHA`; same spike drawing, same log lines; `--spike layered`. Re-run the failed cells.

### Task 3: `plith-ipc` protocol (pure)
**Files:** Create `crates/plith-ipc/src/message.rs`, `src/message/tests.rs`,
`src/pipe_name.rs`; Modify `src/lib.rs`, `README.md`.
**Interfaces (produces):**
- `pub const PROTOCOL_VERSION: u8 = 1; pub const MAX_MESSAGE: usize = 128;
  pub const MAX_LABEL: usize = 64;`
- `enum Meter { Volume = 0, Brightness = 1 }`
- `enum Message { Hello { client_pid: u32 }, Level { meter: Meter, percent: u8, muted: bool,
  label: String }, Hide, Bye }`
- Wire: `b'P' b'O' version kind` then payload, little endian. Kinds: Hello 1 (`u32`), Level 2
  (`meter u8, percent u8, muted u8 (0|1), label_len u8, label UTF-8`), Hide 3, Bye 4.
- `fn encode(&Message) -> Result<Vec<u8>, EncodeError>` (`LabelTooLong`, `PercentOutOfRange`).
- `fn decode(&[u8]) -> Result<Message, DecodeError>`; `DecodeError { Short, Magic, Version(u8),
  UnknownKind(u8), Percent(u8), Meter(u8), Flag(u8), Label, Trailing }` with
  `fn is_fatal(&self) -> bool` (true for `Magic` and `Version`).
- `fn pipe_name(session_id: u32) -> String` = `\\.\pipe\plith-osd-<session_id>`.
- [x] **Step 1 (TDD), `message/tests.rs`:** `every_message_round_trips` (each variant, label empty,
  ASCII, Turkish `"Hoparlör"`, exactly 64 bytes); `level_wire_bytes_are_exact` (Level Volume 42
  unmuted "A" = `[0x50,0x4F,1,2,0,42,0,1,0x41]`); `label_over_64_bytes_does_not_encode`;
  `percent_over_100_is_rejected_both_ways`; `truncated_input_is_short` (every prefix of a valid
  Level); `wrong_magic_and_newer_version_are_fatal`; `unknown_kind_is_not_fatal`;
  `trailing_bytes_are_rejected`; `invalid_utf8_label_is_rejected`; `muted_flag_2_is_rejected`;
  `pipe_name_carries_the_session`.
- [x] **Step 2:** implement; tests pass; five checks; commit `feat(ipc): versioned osd protocol`.

### Task 4: `plith-ipc` pipe server, client and example sender
**Files:** Create `crates/plith-ipc/src/server.rs`, `src/client.rs`, `src/security.rs`,
`src/latest.rs` (+ `latest/tests.rs`), `examples/send_osd.rs`; Modify `Cargo.toml` (windows
features: Foundation, Storage_FileSystem, System_Pipes, System_IO, System_Threading,
System_RemoteDesktop, Security, Security_Authorization, UI_Shell for `ShellExecuteExW`).
**Interfaces (produces):**
- `latest::Slot<T>`: latest-wins hand-off (`put(T)` replaces, `take_wait(timeout) -> Option<T>`,
  `close()`); pure, Mutex + Condvar.
- `OsdServer::bind(name: &str) -> Result<OsdServer, BindError>` (`AlreadyRunning`, `Os(String)`):
  message-mode pipe, `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, one instance,
  security descriptor from `security::pipe_sddl(user_sid)` =
  `D:P(A;;GA;;;<user SID>)S:(ML;;NW;;;ME)` (owner user only; medium label so the medium notch can
  write to a UIAccess server).
- `OsdServer::serve(&self, on: impl FnMut(Result<Message, DecodeError>) -> Flow)` blocks:
  connect, read messages, on disconnect wait for the next client. `Flow { Continue, Stop }`.
  `OsdServer::stopper() -> Stopper` (`stop()` sets a flag and `CancelSynchronousIo` on the
  serving thread).
- `OsdLink::start(name: String) -> OsdLink`: a worker thread connects (`WaitNamedPipeW` 500 ms,
  `CreateFileW`), sends `Hello { client_pid }` first on each connection, then the latest pending
  message; on any error it drops the handle and retries every 1 s. `send(&self, Message)` never
  blocks (encode errors are returned, the rest goes into the `Slot`). Drop sends `Bye` if
  connected and joins.
- `fn launch_helper(exe: &Path) -> Result<(), String>`: `ShellExecuteExW` with `open` (plain
  `CreateProcess` fails on a uiAccess exe with `ERROR_ELEVATION_REQUIRED`).
- `fn current_pipe_name() -> String` via `ProcessIdToSessionId`.
- [ ] **Step 1 (TDD), `latest/tests.rs`:** `newer_value_replaces_older`, `take_wait_times_out_empty`,
  `close_wakes_a_waiter`.
- [ ] **Step 2:** `security.rs` pure part: `pipe_sddl("S-1-5-21-1-2-3-1001")` exact string test.
- [ ] **Step 3:** server, client, launch. Integration test `crates/plith-ipc/tests/pipe.rs`
  (Windows only, unique pipe name per test with the pid): `client_messages_reach_the_server_in_order`,
  `second_bind_reports_already_running`, `client_reconnects_after_server_restart` (2 s bound),
  `hello_comes_first_on_every_connection`.
- [ ] **Step 4:** `examples/send_osd.rs [--pipe NAME]`: sends Level Volume 0 to 100 in steps of
  10 every 150 ms, Level muted, Level Brightness 50, Hide, then Bye. Writes nothing to hardware.
- [ ] **Step 5:** five checks; commit `feat(ipc): pipe server, client, sender`; push.

### Task 5: `plith-system` key repeat collapse
**Files:** Modify `crates/plith-system/src/volume_keys.rs`, `volume_keys/hook.rs`,
`volume_keys/tests.rs`, `README.md`.
**Interfaces (changes):** `enum VolumeKey { Mute, Down, Up }`; `struct HeldKeys` (three bools,
`Default`); `handle_key_event(n_code, w_param, vk, held: &mut HeldKeys, tx: &SyncSender<VolumeKey>,
window, now)`: a down opens the window every time (repeats included) and sends only when the
key was not already held; `WM_KEYUP`/`WM_SYSKEYUP` clears it. `VolumeKeyHook::start` returns
`Receiver<VolumeKey>`. `HeldKeys` lives in the hook thread local beside `SENDER`.
- [ ] **Step 1 (TDD):** `held_key_sends_once_for_135_downs` (135 downs, one up: 1 signal, window
  opened 135 times); `release_then_press_sends_again`; `keys_are_independent` (Up held, Down
  pressed: 2 signals); `up_without_down_is_harmless`; existing tests adapted to the new type.
- [ ] **Step 2:** implement; five checks; commit `feat(system): collapse volume key repeats`.

### Task 6: `plith-system` panic containment and forced-exit restore (L8, M7)
**Files:** Create `crates/plith-system/src/flyout/journal.rs` (+ `journal/tests.rs`),
`crates/plith-system/src/guard.rs`; Modify `flyout/hook.rs`, `flyout/registry.rs`,
`volume_keys/hook.rs`, `hook_thread.rs`, `lib.rs`, `README.md`.
**Interfaces (produces):**
- `guard::contain(f: impl FnOnce()) -> bool` = `catch_unwind(AssertUnwindSafe(f)).is_ok()`.
  The keyboard proc runs its body inside `contain`; on a panic it sets a thread-local `disabled`
  flag (later events skip straight to `CallNextHookEx`). The WinEvent callback does the same and,
  on a panic, restores every hidden window at once.
- The hidden set moves from the thread-local `State` into `Arc<Mutex<HiddenRegistry>>` (locked
  only after the cheap filters pass; poison tolerant). `FlyoutSuppressor::restore_handle() ->
  RestoreHandle` (`Clone + Send + Sync`); `RestoreHandle::restore_now()` shows every hidden window
  with `ShowWindowAsync(SW_SHOWNA)` from the calling thread and empties the set. For the
  backstop and watchdog in `crates/plith` (wiring is the master's follow-up).
- `journal`: `fn format(&[isize]) -> String` (one decimal handle per line), `fn parse(&str) ->
  Vec<isize>` (skips bad lines). File `%LOCALAPPDATA%\Plith\flyout-hidden.txt`, rewritten after
  each hide and restore (on the hook thread, only after a match). `FlyoutSuppressor::start` first
  restores journal entries whose class still passes `is_host_class` and that are hidden, then
  clears the file. This covers a kill (`TerminateProcess`) that no code can catch.
- `hook_thread::pump` returns the `GetMessageW` error and the thread logs it into a status the
  owner can read (`HookThread::ended_early() -> Option<String>`), per services review L8.
- [ ] **Step 1 (TDD):** `guard`: `contain_reports_a_panic`, `contain_passes_a_normal_run`.
  `journal/tests.rs`: `round_trips_handles`, `skips_garbage_lines`, `empty_text_is_empty`.
  `registry`: `restore_now_through_a_shared_handle_empties_the_set`.
- [ ] **Step 2:** implement. A test-only `#[cfg(test)]` panic hook in the pure layer is enough;
  the procs are thin wrappers around tested functions.
- [ ] **Step 3:** five checks; commit `fix(system): contain hook panics, journal hides`.

### Task 7: `plith-system` brightness flyout suppression (G5, services L9)
**Files:** Modify `crates/plith-system/src/brightness/service.rs` (stays under 500),
`brightness/tests.rs`, `README.md`, `examples/probe_flyout.rs`.
**Interfaces (produces):** `BrightnessService::set_suppression_window(Option<Arc<SuppressionWindow>>)`;
the WMI listener callback opens it inline (before the send) on every `WmiMonitorBrightnessEvent`.
A listener that fails to start (COM or `wmi::listen`) sets `listener_error() -> Option<String>`
instead of returning silently.
- [ ] **Step 1 (TDD):** extract the listener's per-event decision as
  `fn on_panel_event(window: Option<&SuppressionWindow>, now: Instant, send: impl FnOnce())`;
  tests `panel_event_opens_the_window_then_sends`, `no_window_still_sends`.
- [ ] **Step 2:** implement; `probe_flyout` gains `--brightness` (dry run: wires the service's
  events to the suppressor, logs matches, hides nothing). Unverifiable here (no built-in panel):
  say so in the README and the handoff.
- [ ] **Step 3:** five checks; commit `feat(system): brightness flyout suppression`.

### Task 8: The OSD app on the chosen path
**Files:** Create `crates/plith-osd/src/model.rs` (+ `model/tests.rs`), `src/paint.rs`,
`src/render.rs`, `src/app.rs`; Modify `src/main.rs`, `README.md`.
**Interfaces:**
- Consumes: `plith_ipc::{OsdServer, Message, Meter, current_pipe_name}`, `plith_render::{Gpu,
  CompositionTarget, clock::open_clock}`, `plith_core::spring::Spring`, `plith_core::tuning::OPACITY`.
- Produces `model::OsdModel`: `new()`, `apply(msg: &Message, now: f64)`, `step(now: f64) -> bool`
  (true while animating), `frame() -> OsdFrame { opacity: f32, level: f32, meter, muted, label }`,
  `should_exit(now: f64) -> bool`. Rules: Level shows (opacity spring to 1, level spring to the
  percent, `OPACITY` tuning) and arms hide at `now + 1.5 s`; Hide hides now; Bye or no client for
  2 s after a disconnect, or the Hello pid gone, means exit; at rest `step` returns false and the
  render thread parks (no wake-ups).
- `render.rs`: one thread owns `Gpu` and the target (UI thread never draws); waits on a
  `latest::Slot`-fed model update, steps on the DComp clock only while animating, writes one
  `motion osd-show|osd-hide ...` line per motion (format of `plith.log`, see request R-2).
- `app.rs`: binds the server (exit 0 on `AlreadyRunning`), creates the band window with the
  spike's winning `Creation`, places it at the top centre of the monitor under the cursor-free
  primary (per-monitor DPI v2), watches the Hello pid with `OpenProcess(SYNCHRONIZE)`.
- [ ] **Step 1 (TDD), `model/tests.rs`:** `level_shows_then_hides_after_1_5s`,
  `new_level_rearms_the_timer`, `hide_starts_fading_at_once`, `level_retarget_keeps_velocity`,
  `rest_reports_not_animating`, `bye_and_lost_client_exit` (2 s), `muted_frame_carries_the_flag`.
- [ ] **Step 2:** implement; dev run (unsigned, `Plain`) fed by `send_osd`; log shows the
  messages and motion lines. Five checks; commit `feat(osd): helper draws the osd`; push.

### Task 9: On-screen verification (batch B ask) and handoff
- [ ] **Step 1:** Ask the master (batch B): install the signed release (`sudo
  install-osd.ps1 -Yes`), Berke at the console with the game from Task 2.
- [ ] **Step 2:** Checks, each recorded with the log line: 9.1 `send_osd` over the desktop, the
  OSD shows and hides; 9.2 over the borderless game and the FSO fullscreen game, motion lines at
  240 fps with 0 to 1 hitches; 9.3 over an elevated window; 9.4 kill `send_osd` mid-sequence, the
  helper hides and exits within 2 s; 9.5 start a second helper, it exits 0 and the first keeps
  serving; 9.6 (Berke's yes needed separately) `verify_system_writes --flyout --yes`, then kill
  it while a window is hidden, restart `probe_flyout`: the journal restores the flyout.
- [ ] **Step 3:** ADR 0009 (OSD drawing path and band, from the spike numbers) on this branch;
  `project-docs` handoff (the record section of this plan, STATUS lines for the master); message
  the master with commits, the record path and the next job (wiring `OsdLink` and
  `RestoreHandle` into `crates/plith`).

---

## Asks to the master (sent with the plan)
- **R-1 (plith-render, none needed now):** the spike and the OSD use the public `Gpu`,
  `CompositionTarget` and `open_clock` as they are. No change requested.
- **R-2 (plith-core, add-only, lane `pages`):** two `MotionKind` variants, `OsdShow` and
  `OsdHide` (`as_str` `osd-show`, `osd-hide`), so the OSD logs with the same `MotionStats` and the
  same line format. Recommended. Fallback if refused: OSD writes its own line through a local
  copy of the stats, which duplicates the hitch math.
- **Batch A (after Task 2):** yes for `sudo install-osd.ps1 -Yes` (copies one signed exe to
  `C:\Program Files\Plith\osd\`, no certificate change since the cert is already trusted), and a
  console session with Berke and one game that offers borderless and fullscreen, plus whether a
  true exclusive (FSO off) title is available. About 15 minutes.
- **Batch B (Task 9):** the same session shape, about 15 minutes, plus a separate yes for 9.6
  (flyout Hide mode).
