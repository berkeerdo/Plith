# OSD Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `plith.exe` starts the OSD helper, keeps the pipe healthy, sends a `Level` on every
volume, mute and brightness change, optionally hides the native flyout, and the OSD shares the
pages' type (M8).

**Architecture:** Pure decisions (what changed, when to relaunch, when to suppress, `--flyout`)
in `plith-core`. A supervisor thread in `crates/plith/src/osd*` owns the helper process, the
`Arc<OsdLink>` and the flyout hooks; the UI-thread feed only calls `OsdLink::send`, which never
blocks. `plith-osd` swaps its text stack for `plith_render::TextSystem`.

**Tech Stack:** Rust 2024, `windows` crate, `plith-ipc`, `plith-system`.

**Spec:** `docs/specs/2026-11-17-osd-wiring-design.md`. Wiring notes:
`crates/plith-ipc/README.md` "Wiring into crates/plith".

## Global Constraints
- Limits: 500 lines source, 150 entry, 800 test, 80 per function; `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`.
- Pure logic in `plith-core` with unit tests; no Windows code there.
- All code and comments English; user-facing strings Turkish, upper-cased with `tr_upper`.
- No em dash anywhere. Conventional Commits, subject max 50 chars, no AI attribution.
- Builds and tests: `node ~/.claude/bin/heavy-run.mjs -- <cmd>`. Never move the mouse from a script.
- No hardware writes: suppression stays off by default (`--flyout off`); turning it on is Berke's call at the console.
- Done = green `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`.

## Review Focus
1. A device name over 64 bytes (`MAX_LABEL`) makes `encode` fail: the label must be cut at a char boundary first (Task 2 test).
2. The first service reading after start, or an endpoint appearing (`attached` None to Some), must not pop the OSD (Task 2 test).
3. A helper that dies on every launch must not spin: spacing and give-up (Task 3 tests).
4. Link down while `--flyout hide` runs must leave no flyout hidden (Task 3 `wants_suppression` test, Task 6 stop path).
5. Idle at rest: the supervisor must never post to the UI thread (review point for Task 6; probing runs start no OSD, so the idle check cannot see it).

---

### Task 1: `--flyout` launch option

**Files:** Modify `crates/plith-core/src/launch.rs` (+ its tests file).

**Interfaces:** Produces `pub enum FlyoutMode { Off, DryRun, Hide }` (`Default` = `Off`) in
`plith_core::launch`, field `pub flyout: FlyoutMode` on `Launch`.

- [x] Tests: `--flyout off|dry|hide` parse to the three modes; no flag gives `Off`; `--flyout x` gives `LaunchError::BadValue { flag: "--flyout", value: "x" }`; missing value gives `MissingValue`.
- [x] Run, see them fail; implement `parse_flyout(value: &str) -> Option<FlyoutMode>` like `parse_clock`; add `[--flyout off|dry|hide]` to `USAGE` in `crates/plith/src/main.rs`.
- [x] Tests pass; commit `feat(core): add --flyout launch option`.

### Task 2: OSD trigger diff

**Files:** Create `crates/plith-core/src/osd.rs`, `crates/plith-core/src/osd/tests.rs`; register in `lib.rs`.

**Interfaces:** Produces in `plith_core::osd`:
- `pub enum OsdMeter { Volume, Brightness }`
- `pub struct OsdLevel { pub meter: OsdMeter, pub percent: u8, pub muted: bool, pub label: String }`
- `pub struct OsdReading { pub volume: Option<OsdLevel>, pub brightness: Option<OsdLevel> }` (`Default`, `PartialEq`)
- `pub fn reading(audio: &AudioFacts, system: &SystemFacts) -> OsdReading`
- `pub fn changes(prev: &OsdReading, next: &OsdReading, open: Option<PageId>) -> Vec<OsdLevel>`
- `pub const MAX_LABEL_BYTES: usize = 64;`

Rules: volume from `audio.attached` (`(f32 0..1, muted)`, percent rounded and clamped to 100),
label = name of the endpoint whose `id == default_id`, else `copy::TAB_AUDIO`. Brightness from
`system.brightness` `(name, percent)`, label the name, `muted` false. Labels cut to at most 64
bytes at a char boundary. A change = same meter `Some` on both sides and percent or muted
differ. Volume is skipped when `open == Some(PageId::Audio)`, brightness when
`open == Some(PageId::System)`.

- [x] Tests: `percent_and_mute_changes_fire`, `same_values_fire_nothing`, `appearing_meter_is_not_a_change` (None to Some), `device_switch_with_same_level_fires_nothing`, `open_audio_page_hides_volume_only`, `open_system_page_hides_brightness_only`, `long_label_is_cut_on_a_char_boundary` (70 bytes of `ğ` gives at most 64 bytes, valid UTF-8), `missing_default_name_uses_tab_audio`, `percent_rounds_and_clamps` (0.004 to 0, 0.995 to 100, 1.2 to 100).
- [x] Run, fail; implement; pass; commit `feat(core): add OSD trigger diff`.

### Task 3: Relaunch and suppression policy

**Files:** Create `crates/plith-core/src/osd/policy.rs`, `crates/plith-core/src/osd/policy/tests.rs`.

**Interfaces:** Produces in `plith_core::osd::policy`:
- `pub const LAUNCH_SPACING: Duration = 3 s; pub const GIVE_UP_AFTER: usize = 5; pub const FAILURE_WINDOW: Duration = 60 s; pub const DOWN_GRACE: Duration = 3 s;`
- `pub struct Relaunch` with `new() -> Self`, `fn on_failure(&mut self, now: Instant) -> Next`, `fn on_connected(&mut self)` (clears the failure history), where `pub enum Next { At(Instant), GiveUp }`.
- `pub fn wants_suppression(mode: FlyoutMode, link_up: bool) -> Option<SuppressMode>` with `pub enum SuppressMode { DryRun, Hide }`: `Off` gives `None`; `DryRun` gives `DryRun` always; `Hide` gives `Hide` only while `link_up`, else `None`.

- [x] Tests: `first_failure_retries_after_spacing`, `fifth_failure_in_a_minute_gives_up`, `failures_older_than_the_window_do_not_count`, `connect_clears_history`, `hide_only_while_link_up`, `dry_run_ignores_link`, `off_is_none`.
- [x] Run, fail; implement; pass; commit `feat(core): add OSD relaunch policy`.

### Task 4: Single instance

**Files:** Create `crates/plith/src/instance.rs`, `crates/plith/src/instance/tests.rs`; modify `crates/plith/src/main.rs` (module), `crates/plith/src/app.rs` (`run_logged`, first thing after the log opens).

**Interfaces:** Produces `pub struct InstanceGuard` (owns the mutex handle, closes on drop),
`pub fn acquire(name: &str) -> Result<Option<InstanceGuard>, String>` (`None` = another holder),
`pub const INSTANCE_NAME: &str = "Local\\Plith.Notch";`.

- [x] Tests: `second_acquire_in_process_sees_the_first` (unique test name with the pid; second call `None`), `drop_releases` (acquire again after drop gives `Some`).
- [x] Implement with `CreateMutexW` and `GetLastError() == ERROR_ALREADY_EXISTS`. In `run_logged`: `None` logs `single instance: another plith runs` and returns 0; `Err` logs and continues. Probing runs (`--demo`, `--idle-check`) skip the guard so measurement runs keep working beside a running notch.
- [x] Pass; commit `feat(app): run a single notch per session`.

### Task 5: OSD sink in the feed

**Files:** Modify `crates/plith/src/app/feed.rs` (+ tests file under `app/feed/` or the existing app tests).

**Interfaces:** Consumes Task 2. Produces `pub type OsdSink = Box<dyn FnMut(OsdLevel)>;` and
`Feed::set_osd(&mut self, sink: OsdSink)`. The feed keeps `osd_seen: Option<OsdReading>`. After
every drain it computes `reading(&facts.audio, &facts.system)`; with a previous reading and a
sink it calls the sink for each `changes(prev, next, open)` item (`open` = `Some(page)` only
when `is_open(scene)`), then stores the reading. Without a sink nothing is computed.

- [x] Tests with a recording sink and a fake `Services`: `no_osd_on_first_drain`, `volume_change_reaches_the_sink`, `open_audio_page_keeps_volume_quiet`.
- [x] Implement; pass; commit `feat(app): feed volume and brightness to the OSD`.

### Task 6: Supervisor, link and flyout

**Files:** Create `crates/plith/src/osd.rs`, `crates/plith/src/osd/supervisor.rs`, `crates/plith/src/osd/flyout.rs` (+ tests files for any pure helpers); modify `crates/plith/src/app.rs` (start after `attach_feed`, only when `launch` is normal mode and real services), `crates/plith/src/app/backstop.rs` and `crates/plith/src/app/wire.rs` (call `osd::restore_now()` before each `process::exit`).

**Interfaces:** Consumes Tasks 1, 3, 5 and `plith_ipc::{launch_helper, OsdLink, current_pipe_name, INSTALLED_HELPER, Message, Meter}`, `plith_system::{FlyoutSuppressor, VolumeKeyHook, flyout::Mode}`. Produces:
- `pub struct OsdSide` with `pub fn start(mode: FlyoutMode, log: Arc<LogFile>) -> Option<OsdSide>` (`None` and one log line when the process runs above medium integrity), `pub fn sink(&self) -> OsdSink` (maps `OsdLevel` to `Message::Level` on a shared `Arc<OsdLink>`, logs an `EncodeError`), drop stops the supervisor and drops the link (`Bye`).
- `pub fn restore_now()`: restores hidden flyouts through a process-wide `OnceLock<RestoreHandle>`; a no-op when suppression never ran.

Supervisor thread (one, named `plith-osd-supervisor`, initializes COM apartment-threaded for
`launch_helper`): launch, then every 1 s check `has_exited()` and `is_connected()`; down for
`DOWN_GRACE` or exited counts as a failure for `Relaunch`; `GiveUp` logs once and stops
launching (the link stays, a manually started helper still connects). Each tick applies
`wants_suppression(mode, link_up)`: start `FlyoutSuppressor` in the matching `Mode` plus
`VolumeKeyHook::start(Some(suppression_window()))` when it turns `Some`, stop both when it turns
`None`; a start failure logs once and pins suppression off for the session. Drain the key
receiver each tick (keys only open the window). Log `failure()` of both hooks once when it
appears. Log lines start with `osd:`. The thread wakes only itself, never the UI thread.

- [x] Tests for any pure helper extracted (for example the log line texts); the thread itself is verified in Task 7.
- [x] Implement; five checks green; commit `feat(app): start and supervise the OSD helper`.

### Task 7: M8, the OSD on the shared text stack

**Files:** Modify `crates/plith-osd/src/paint.rs`, `crates/plith-osd/src/paint/text.rs` (remove or shrink), `crates/plith-osd/src/paint/tests.rs`.

**Interfaces:** Consumes `plith_render::TextSystem` (`new`, `layout`, `measure`, `display_text`), `plith_core::draw::TextStyle`, `plith_core::pages::copy::{tr_upper, MUTED_LABEL}`.

- [x] `upper_tr` is deleted; the muted word is `copy::MUTED_LABEL`; tests that covered `upper_tr` move to asserting the shown muted word is `"SESSİZ"` and device labels stay invariant (`"display"` shows `"DISPLAY"`).
- [x] The painter owns a `TextSystem` built on the render thread; label = `TextStyle::Label`, end trim; value = `TextStyle::Value`, right-aligned by measuring; `uses_bundled_fonts()` false logs a `paint:` note once.
- [x] Five checks green; commit `refactor(osd): draw text with TextSystem`.

### Task 8: Run on this PC and record

- [x] Build release; run `plith.exe` (normal mode, real services). `plith.log` shows `osd:` launch and connect lines; `%LOCALAPPDATA%\Plith\plith-osd.log` shows an admitted client. Change nothing on the hardware; a volume change by Berke is optional.
- [x] A second `plith.exe` exits 0 with `single instance: another plith runs`.
- [x] `plith.exe --idle-check 60` with real services still reports 0 wakes (regression check of Tasks 4 and 5).
- [x] Close the notch; the helper logs `Bye` and exits.
- [x] Record below; update `docs/STATUS.md` (next job, single-instance wording) and the README "Wiring" section where behavior differs.

## Record

> A ticked box means the step was taken, not that nothing deviated.
> Deviations, deliberate omissions and decisions are listed here and in STATUS.

Run 2026-11-17, one session with subagents (Sonnet implementers, Opus for Task 6 and the final
review). Commits `7f7da98` to `2e4dfca` on `feature/osd-wiring`.

**Deviations and decisions (master rulings):**
- Tasks 1 to 3 and 4 to 5 were each done by one implementer with one review.
- Task 6: `restore_now` keeps a `Mutex<Option<RestoreHandle>>`, not a `OnceLock` (suppression
  restarts with a new hidden set each time the link comes back). Extra pure `osd/health.rs`.
  A link whose helper exited counts as down at once ("stale"). `Relaunch::on_connected` only
  after 10 s of stable link. `plith-ipc` gained `own_integrity_above_medium`. Fix round 1: one
  crash counts as one failure (was two, about 16 s recovery), restore on a failed suppressor
  stop, supervisor join bounded at 1.5 s.
- Task 7: the OSD value now uses the shared `Value` style (10 DIP regular, was 13 DIP medium).
  "SESSİZ" measured 45.6 DIP, so the value slot grew to 48 DIP (`VALUE_LEFT` 236) and the ruler
  shrank to 136 DIP.
- Final review fix wave: the feed reads the OSD only after a change, a text failure no longer ends
  the helper's render thread, `--fixture` runs skip the instance guard, and release `plith.exe` has
  no console window (a bad flag is written to `plith.log` as `launch error:`).

**Measured on this PC (console, Berke present):**
- Installed helper was the 2026-11-16 16:09 build (pipe name without the user SID): the link
  never connected and the second launch exited "already served". After a signed reinstall
  (Berke's yes, SHA-256 `3C1ABB16...8711`): `osd: helper launched`, `osd: link connected` 1 s
  later, helper logs `hello`, then `bye` and exits on close; helper font `Martian Mono`.
- A second `plith.exe` logs `single instance: another plith runs` and exits 0 (179 to 210 ms).
  Close by `WM_CLOSE` 85 to 97 ms.
- Idle check 60 s with real services: one run during use (2145 wakes, all motions), one
  hands-off run 2 wakes, `main` 0, this branch again 0 (78 ms CPU each). No OSD code runs in
  probing runs, so the 2 are taken as noise.

**Not measured:** a `Level` reaching the helper from a real key press, `--flyout hide`, the look
of the new type and ruler, the background wake rate of a normal run (supervisor 1 Hz plus link
worker 1 to 2 Hz, never on the UI or render thread).
