# Console Fix Round Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the five things Berke found on screen on 2026-11-18: home mark, speaker glyph, title
scroll, ruler drag, OSD prewarm.

**Architecture:** Pure layout, drag and scroll math in `plith-core` with unit tests; the shell
gains pointer press, drag and release events; the render thread draws a new one-shot live scroll
node in the collapsed pill and parks when it rests; the OSD helper draws one hidden frame after
every stack build.

**Tech Stack:** Rust 2024 workspace, `windows` crate (Win32, Direct2D, DirectWrite).

**Spec:** `docs/specs/2026-11-19-console-fix-round-design.md`

## Global Constraints
- Branch `feature/console-fix-round` in worktree `.claude/worktrees/fix-round`, base `main` @ `1f81c52`.
- Limits: 500 source, 800 test, 80 per function; `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`.
- `plith-shell/src/window.rs` (443) and `plith/src/app/feed.rs` (423) do not grow: new code goes
  in `window/mouse.rs` and a new `app/feed/drag.rs`. `pages/media/tests.rs` (476): new media
  tests go in a new file.
- Pure logic in `plith-core`, no Windows code there.
- Every commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  focused `cargo test -p <crate>`; builds and test runs through
  `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- English code and docs, Conventional Commits (subject max 50), no em dash, no AI mention.
- No script moves the mouse. Installing the rebuilt OSD helper needs Berke's yes.
- No looping marquee; no seek per pointer move; no keyboard path.

## Review Focus
1. A drag that leaves the island shape (or the window) must keep the drag and must not start the
   collapse delay until release. Test in Task 4.
2. A press that moves less than the slop is still a click (tabs, transport buttons). Task 4.
3. A drag on the timeline with no timeline (no session, duration 0) writes nothing. Task 5.
4. A title that fits its slot never scrolls and never asks for frames (idle budget). Task 7.
5. The readback that lags the write must not snap the ruler back mid-drag; a write that the
   service rejects must not leave the override forever (1 s expiry). Task 6.

---

### Task 1: Home mark in the tab row (+ ADR 0016)

**Files:**
- Modify: `crates/plith-core/src/pages/tabbar.rs`, `crates/plith-core/src/pages/glyphs.rs`,
  `crates/plith-core/src/pages/welcome.rs` (its `tabbar(None, "", ..)` call)
- Test: `crates/plith-core/src/pages/tabbar/tests.rs`, plus any page test that pins old station xs
  (`rg "142\.0|366\.0" crates`)
- Create: `docs/decisions/0016-welcome-from-the-tab-row.md` (amends ADR 0014; mark 0014
  "amended by 0016")

**Interfaces:**
- Produces: `pub(crate) fn home(x: f64, y: f64, ink: Ink) -> Vec<Node>` in `glyphs.rs` (16x16
  DIP box, top-left at `(x, y)`); `tabbar(active, clock, lang)` unchanged in signature, now
  accepts `Some(PageId::Welcome)` to mark home active.

- [x] **Step 1: Write the failing tests** in `tabbar/tests.rs`:
  `stations_make_room_for_home`: station xs `[54.0, 166.0, 278.0, 390.0]`, track stroke
  `(54.0, 34.0)..(390.0, 34.0)` (rename `four_stations_one_line_between_ends`).
  `home_target_comes_first`: `targets[0].action == Action::Tab(PageId::Welcome)`, its rect is
  `x 18..43`, `y 12..40`; `hit(&targets, 30.0, 30.0) == Some(Action::Tab(PageId::Welcome))`;
  `hit(&targets, 54.0, 30.0) == Some(Action::Tab(PageId::Audio))`.
  `home_ink_follows_active`: with `Some(PageId::Welcome)` every home node ink is `Ink::Ink` and all
  stations are hollow; with `Some(PageId::Audio)` home ink is `Ink::Faint`.
- [x] **Step 2: Run** `cargo test -p plith-core tabbar`: FAIL (xs 30.., no home target).
- [x] **Step 3: Implement**: `FIRST_X = MARGIN_LEFT + 24.0`; home glyph centred on x 30 and on
  the label-and-line band; its target pushed first. Welcome passes `Some(PageId::Welcome)`.
  Drawing of the house shape is the builder's (strokes in the station weight).
- [x] **Step 4: Run** `cargo test -p plith-core`: PASS (fix every pinned old x).
- [x] **Step 5: Render** `design/r7/fix-round/` offscreen renders of the audio page and welcome
  (the tool `design/r7` renders used) and look at both.
- [x] **Step 6: Write ADR 0016, commit** `feat(pages): add a home mark to the tab row`.

### Task 2: Speaker glyph before the Karot percent

**Files:**
- Modify: `crates/plith-core/src/pages/idle_pill.rs`, `crates/plith-core/src/pages/idle_pill/` tests,
  `crates/plith/src/app/idle_facts.rs` (`pill_sound`)

**Interfaces:**
- Produces: `IdleFacts.sound` becomes `Option<SoundWords>` with
  `pub struct SoundWords { pub text: String, pub muted: bool }`; `sound_words(..) ->
  Option<SoundWords>` (same parameters).

- [x] **Step 1: Failing tests**: `karot_sound_has_speaker_glyph`: with only `sound` set
  (`%62`, not muted) the list holds the speaker glyph nodes, whose right edge is
  `RIGHT_EDGE - 3.0 * VALUE_ADVANCE - GLYPH_GAP` (`GLYPH_GAP = 4.0`, glyph 16 DIP).
  `karot_glyph_only_on_sound_branch`: with `playing` or `now_sky` set there is no glyph.
  `muted_has_no_glyph`: muted shows the off word without the glyph.
  `sound_words_fit_leaves_glyph_room`: the `· kind` suffix is dropped when
  `chars * VALUE_ADVANCE > TURN_W - 20.0`.
- [x] **Step 2: Run** `cargo test -p plith-core idle_pill`: FAIL.
- [x] **Step 3: Implement** with `super::glyphs::glyph(DeviceKind::Speaker, x, y, Ink::Muted)`,
  x from the text's estimated width (Martian Mono is monospace).
- [x] **Step 4: Run** `cargo test -p plith-core -p plith`: PASS.
- [x] **Step 5: Render** the Karot pill in `tr`, `de`, `fr` (percent forms differ) and look.
- [x] **Step 6: Commit** `feat(pages): show a speaker glyph by the volume`.

### Task 3: OSD prewarm

**Files:**
- Modify: `crates/plith-osd/src/render/stack.rs`, `crates/plith-osd/src/render.rs`
  (`run`, `apply_relayout`, the device-lost rebuild), `crates/plith-osd/src/model.rs`

**Interfaces:**
- Produces: `pub fn warm_frames(label: &str, muted_word: &str) -> [OsdFrame; 2]` in `model.rs`
  (one `%100` volume frame, one muted frame, both `opacity: 0.0`);
  `Stack::prewarm(&mut self, frames: &[OsdFrame]) -> Result<f64, Error>` (total draw ms).

- [x] **Step 1: Failing test** `warm_frames_cover_both_slots`: two frames, opacity 0, levels 1.0
  and muted true with the given word, label as given.
- [x] **Step 2: Run** `cargo test -p plith-osd warm`: FAIL.
- [x] **Step 3: Implement**: `prewarm` paints each frame through the normal path (the painter
  must lay out and draw text at opacity 0, not return early) and presents while hidden. Call it
  after `build_stack` in `run` and after every rebuild; log
  `prewarm: osd frames=2 draw_ms=<x>`. Default label and word: the last ones sent, else
  `"VOLUME"` and the `en` muted word.
- [x] **Step 4: Run** `cargo test -p plith-osd`: PASS; release build of the helper.
- [x] **Step 5: Commit** `perf(osd): draw one hidden frame after each build`.

### Task 4: Shell pointer press, drag and release

**Files:**
- Modify: `crates/plith-shell/src/window/mouse.rs`, `crates/plith-shell/src/window.rs`
  (enum variants only), mouse tests next to `mouse.rs`

**Interfaces:**
- Produces: `ShellEvent::Press { x, y }`, `ShellEvent::Drag { x, y }`,
  `ShellEvent::Release { x, y }` (island DIP, not clamped to the shape);
  `plith_core::input::DRAG_SLOP: f64 = 3.0`. `Click` keeps its meaning and is emitted only for
  a press that never passed the slop.

- [x] **Step 1: Failing tests** on `PressRecord` (pure, no window):
  `small_move_stays_click`: down (10,10), move (12,11), up inside: click at release, no drag.
  `move_past_slop_drags`: down (10,10), move (14,10): drag; up returns release, no click.
  `drag_outside_shape_continues`: after a drag, a point outside the shape still yields a drag
  point. `leave_during_drag_is_deferred`: a leave while dragging returns "defer"; release then
  reports the pending leave.
- [x] **Step 2: Run** `cargo test -p plith-shell mouse`: FAIL.
- [x] **Step 3: Implement**: an unclamped `island_point` twin for captured moves; on
  `WM_MOUSEMOVE` with a press recorded emit `Press` once (at down), then `Drag` per new point;
  on up emit `Release` (dragged) or `Click` (not); a leave while dragged is emitted after
  `Release`.
- [x] **Step 4: Run** `cargo test -p plith-shell`: PASS.
- [x] **Step 5: Commit** `feat(shell): report press, drag and release`.

### Task 5: Ruler drag logic in core

**Files:**
- Create: `crates/plith-core/src/input/drag.rs` (+ `input/drag/tests.rs`)
- Modify: `crates/plith-core/src/input.rs` (module line)

**Interfaces:**
- Produces:
  `pub enum RulerKind { Volume, Brightness, Seek }`;
  `pub struct RulerDrag { pub kind: RulerKind, pub ruler: Rect }`;
  `pub fn begin_drag(targets: &[HitTarget], x: f64, y: f64) -> Option<RulerDrag>` (the first
  target under the point with the same 4 DIP slack as `hit`, only ruler actions);
  `impl RulerDrag { pub fn value_at(&self, x: f64) -> Action }` (`SetVolume`, `SetBrightness`,
  `Seek(fraction)` from the existing `brightness_at` and `seek_fraction`);
  `pub struct Throttle { .. }` with `pub fn new(min_gap_s: f64) -> Self` and
  `pub fn due(&mut self, now_s: f64) -> bool`; `pub const DRAG_WRITE_GAP_S: f64 = 1.0 / 30.0`.

- [x] **Step 1: Failing tests**: `begin_on_volume_ruler`, `begin_off_ruler_is_none` (a tab target
  gives `None`), `value_clamps_past_both_ends` (x far left gives 0, far right 100 or 1.0),
  `throttle_lets_one_per_gap` (due at 0.0, not at 0.02, due at 0.034).
- [x] **Step 2: Run** `cargo test -p plith-core drag`: FAIL.
- [x] **Step 3: Implement.**
- [x] **Step 4: Run** `cargo test -p plith-core`: PASS.
- [x] **Step 5: Commit** `feat(input): add ruler drag logic`.

### Task 6: Drag in the app, with a shown value

**Files:**
- Create: `crates/plith/src/app/feed/drag.rs` (override state), tests in
  `crates/plith/src/app/input/tests.rs` or a new `input/drag_tests.rs`
- Modify: `crates/plith/src/app/state.rs` (route the three events), `crates/plith/src/app/input.rs`

**Interfaces:**
- Consumes: Task 4 events, Task 5 `begin_drag`, `RulerDrag::value_at`, `Throttle`.
- Produces: `Feed::set_override(&mut self, kind: RulerKind, value: f64, now_s: f64, mailbox)`;
  pages read the override in place of the service value for volume, brightness and the seek
  fill until the readback is within 1 (percent) or 0.01 (fraction) of it, or 1 s passes.

- [x] **Step 1: Failing tests** (fixture services):
  `drag_volume_writes_throttled_and_final`: press on the volume ruler, 10 drags inside 0.1 s,
  release: the fixture saw at most 4 volume writes plus the release value last.
  `drag_seek_writes_once_on_release`: 10 drags on the timeline: zero seeks, one seek on release.
  `seek_drag_without_timeline_writes_nothing`.
  `override_shows_until_readback`: during the drag the audio page list shows the dragged
  percent; after a readback equal to it the override is gone; with no readback it ends after 1 s.
  `drag_leave_does_not_collapse_until_release`.
- [x] **Step 2: Run** `cargo test -p plith app::input`: FAIL.
- [x] **Step 3: Implement**: `on_press` starts a `RulerDrag` only on an expanded island over a
  ruler; `on_drag` sets the override and writes volume or brightness when the throttle is due;
  `on_release` writes the final value (seek only here).
- [x] **Step 4: Run** `cargo test -p plith`: PASS.
- [x] **Step 5: Commit** `feat(app): drag the volume, brightness and seek rulers`.

### Task 7: Scroll node and its timing in core

**Files:**
- Modify: `crates/plith-core/src/draw.rs` (new `LiveNode::Scroll`), `crates/plith-core/src/ambient.rs`,
  `crates/plith-core/src/pages/idle_pill.rs`
- Create: `crates/plith-core/src/scroll.rs` (+ `scroll/tests.rs`)

**Interfaces:**
- Produces:
  `LiveNode::Scroll { rect: Rect, text: Arc<str>, style: TextStyle, ink: Ink, start: SystemTime, run_s: f64 }`;
  `pub fn scroll_offset(elapsed_s: f64, overflow: f64) -> f64` (0 at rest, negative while moving;
  phases: hold `SCROLL_HOLD_S = 0.8`, travel at `SCROLL_SPEED = 40.0` DIP/s to `-overflow`, hold
  `SCROLL_HOLD_S`, return at the same speed, rest at 0);
  `pub fn scroll_run_s(overflow: f64) -> f64` (total of those phases);
  `IdleFacts.scroll_start: Option<SystemTime>`;
  `wants_ambient(scene, content, now: SystemTime) -> bool` gains: the island is collapsed, not
  suppressed, and the pill has a `Scroll` with `now < start + run_s`.
- Core estimates overflow as `chars * VALUE_ADVANCE - TURN_W` for `run_s`; the painter clamps to
  the measured overflow, so a run may end early on screen and tick a few still frames.

- [x] **Step 1: Failing tests**: `offset_rests_at_zero_after_run`, `offset_reaches_minus_overflow`,
  `no_overflow_never_moves` (`scroll_run_s(0.0) == 0.0`), `karot_title_fits_is_plain_text` (short
  title: a `Node::Text`, no `Live`), `karot_long_title_with_start_is_scroll`,
  `long_title_without_start_is_plain_text`, `ambient_for_pill_scroll_only_until_run_ends`,
  `ambient_none_for_pill_without_scroll`.
- [x] **Step 2: Run** `cargo test -p plith-core`: FAIL.
- [x] **Step 3: Implement**; update every `wants_ambient` caller to pass `now`.
- [x] **Step 4: Run** `cargo test --workspace` (heavy-run): PASS.
- [x] **Step 5: Commit** `feat(core): add a one-shot title scroll node`.

### Task 8: Draw the scroll in the collapsed pill

**Files:**
- Modify: `crates/plith-render/src/live.rs`, `crates/plith-render/src/paint.rs` (live pass for the
  pill layer), `crates/plith-render/src/thread/run.rs` and `run/ambient.rs` (collapsed ambient)

**Interfaces:**
- Consumes: Task 7 `LiveNode::Scroll`, `scroll_offset`, `wants_ambient(.., now)`.

- [x] **Step 1: Failing test** in `thread/tests.rs` (fake painter and clock):
  `collapsed_scroll_ticks_then_parks`: a pill content with a Scroll whose `run_s` is 1.0 gives
  ambient frames for about 1 s, one `ambient` line, then the thread parks on the mailbox.
- [x] **Step 2: Run** `cargo test -p plith-render`: FAIL.
- [x] **Step 3: Implement**: live pass for the pill at `1 - content_opacity`; the scroll draws
  inside a clip of `rect` (`push_clip` pattern) with a trailing fade (gradient opacity mask,
  length the builder's, about 16 DIP) and no fade at rest when the text fits.
- [x] **Step 4: Run** `cargo test -p plith-render`: PASS; an offscreen render of a long title at
  rest and mid-run, looked at.
- [x] **Step 5: Commit** `feat(render): scroll the long pill title once`.

### Task 9: Scroll triggers

**Files:**
- Modify: `crates/plith/src/app/idle_facts.rs` (track change sets `scroll_start`),
  `crates/plith/src/app/input.rs` (`on_move` on a collapsed island restarts it when not running)

- [x] **Step 1: Failing tests**: `track_change_starts_scroll`, `same_track_keeps_start`,
  `hover_restarts_finished_scroll`, `hover_during_run_keeps_start`.
- [x] **Step 2: Run** `cargo test -p plith`: FAIL.
- [x] **Step 3: Implement.**
- [x] **Step 4: Run** `cargo test -p plith`: PASS.
- [x] **Step 5: Commit** `feat(app): scroll the title on change and hover`.

### Task 10: Gate, review, console

- [x] **Step 1:** Five checks green on the branch head (heavy-run): fmt, clippy, workspace tests,
  `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`. Record the test count.
- [x] **Step 2:** Whole-branch review (`superpowers:requesting-code-review`), fix wave, re-review.
- [x] **Step 3:** Rebuild the OSD helper; reinstall it signed after Berke's yes.
- [x] **Step 4 (console, Berke present, `query session` first):** home mark round trip from each
  tab; Karot glyph look; a long title scrolls once on track change and on hover, then a 60 s
  idle check meets `allowed()`; drag on the volume, brightness and seek rulers (his hand); the
  first OSD show of a fresh run has no hitch (`motion` line, draw under 2 ms). Record in this
  plan's log.
- [x] **Step 5:** Merge after his yes; STATUS rewritten.

## Progress log

> A ticked box means the step was taken, not that nothing deviated.
> Deviations, deliberate omissions and decisions are listed here and in STATUS. Step 4 was
> ticked with one bar unmet: the first OSD show had a present hitch (ruled an open item).

2026-11-19, subagent route (Sonnet implementers and reviewers, Opus for Task 8, the final
review and its fix wave):
- Tasks 1 to 9 done, each with a task review; Task 6 needed one fix round (held press kept
  the override, duration-0 seek guard). Commits 429e572 to 7f438b8.
- Five checks on 7f438b8: fmt, clippy, 1105 tests 0 failed, structure 0 violations, deps clean.
- Final review: three Important (backward clock kept the pill ticking, scroll at 60 fps against
  the 240 fps bar, a drag released outside could leave the island open) and four small items,
  fixed in one wave (5cfcb84 to 9b6e8b7, 1113 tests 0 failed), re-review approved.
- Ruling: the pill scroll paints every frame while it moves (display rate); expanded ambient
  keeps its divisor.
- Ruling: in Turkish `%62 · KULAKLIK` no longer fits beside the glyph; the pill shows the glyph
  and `%62` only. Berke judges it in the console.
- Unmeasured until the console: every on-screen item of Step 4, plus whether Windows posts a
  late leave after a drag (a duplicate Leave only re-arms the 0.4 s collapse delay).
- Helper reinstalled signed 2026-11-19 (Berke's yes, second sudo prompt; the first was
  cancelled), SHA-256 `188722D6...484484`, signer `144A7919...AC0F`. A normal release run over
  RDP (`remote true`, 32 Hz, so no motion numbers): `osd: helper launched`, `link connected`
  2 s later, helper log `prewarm: osd frames=2 draw_ms=6.24` before any show, clean `exit 0`.
  The cold cost now lands in the hidden prewarm; the first real show is measured in Step 4.
- 2026-11-19 console session (release from `9da90fe` built 04:40, `query session` showed
  `>console`, log `remote false`, 240 Hz, Berke at the PC):
  - 17:16 normal run, real services: helper launched, `prewarm: osd frames=2 draw_ms=5.89`,
    link connected. 22 pill motions (7 expand, 8 page, 7 collapse), all 240 fps but one
    collapse with 1 hitch (8.3 ms, present 3.4 ms, the known collapse hitch); draw p99 at most
    1.34 ms. Clean `exit 0` by `WM_CLOSE`, helper `osd done`.
  - First OSD show of the run: draw p50 0.28 p99 0.77 ms (the prewarm moved the cold draw out),
    but **1 hitch: gap 12.5 ms, present 11.6 ms** at frame 14. The draw bar is met, the "no
    hitch" bar is not: the cold cost of a first show now sits in present, not draw. Hide 0
    hitches. Ruling (Berke took the proposal): open item, not fixed in this round; its root
    cause is measured with osd Task 9 over a game.
  - Berke: home mark, scroll, three ruler drags and a drag released outside "çalışıyor hepsi"
    (his eyes and hand; the log has no drag or leave lines to measure them by).
  - Berke: the speaker glyph is not level with the number ("sayı ile aynı hiza değil").
    Measured on a screen grab: glyph rows 11..20, `100%` caps rows 11..18. The glyph was
    centred on the 14 DIP text box, whose centre is 1 DIP below the capitals (the box holds the
    descender). Fix: centre it on the capitals (`CAP_RISE`). Renders at 2x: glyph centre 36
    against digits 34 before, 34 and 34 after. Five checks green (1113 tests, 0 failed). On
    screen at 21:31 (`remote false`): glyph rows 10..19 and digits rows 11..18, both centre 15.
  - 21:06 `--idle-check 60 --idle-look karot`, music playing: **1 render wake-up (allowed 2),
    93.8 ms CPU, exit 0**.
  - `lang=auto` sent the OSD word `MUTED` (English): auto picks the Windows UI language here.
- 2026-11-20 merged on Berke's yes as `397e7d9` (glyph fix `ec86288` on top of `9da90fe`).
  Five checks on the merge: fmt, clippy, 1113 tests 0 failed, structure 0 violations, deps
  clean.
