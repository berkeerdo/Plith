# Merge review: feature/pages-r4 (fresh read)

Reviewed: `git diff main...origin/feature/pages-r4` at `f895939` (151 files, merge base `0a73ade`).
I read in full: `crates/plith/src/app.rs`, `app/state.rs`, `app/input.rs`, `app/feed.rs`,
`app/wire.rs`, `pages.rs`, `live.rs`, `model.rs`, `services.rs`, `services/writes.rs`,
`services/start.rs`, `services/fixture.rs`, `services/fixture/tick.rs`, and the diffs of `idle.rs`,
`demo.rs`, `main.rs`, `app/backstop.rs`. In `plith-render`: `thread/run.rs`,
`thread/run/engine.rs`, `thread/run/ambient.rs`, `pages.rs`, `paint.rs`, `live.rs`, the first half
of `text.rs`, the image and stroke parts of `list.rs`, `gpu_paint.rs`, `thread.rs` (`render_main`).
In `plith-shell`: `window/mouse.rs`, `timer.rs`, the diffs of `window.rs`, `window/state.rs`,
`poster.rs`, `process.rs`, `suppress.rs`, `hit.rs`, `hit/apply.rs`. In `plith-core`: `input.rs`,
`ambient.rs`, `pages.rs`, `pages/audio.rs`, `pages/audio/model.rs`, the targets part of
`pages/audio/layout.rs`, `pages/derive.rs`, `pages/copy.rs`, the targets and copy of
`pages/media.rs`, `pages/system.rs`, `pages/shelf.rs`, and the diffs of `scene.rs`, `shapes.rs`,
`geometry.rs`, `layout.rs`, `demo.rs`. In `plith-audio` the whole Voicemeeter diff; in
`plith-media` `art.rs` and the `service.rs` diff. Plus the plan `docs/plans/active/pages-r4.md`,
`pages-r4-adr-drafts.md`, `docs/design/r4/renders-1/README.md`, `docs/LANES.md` and the R6 review
for format. Nothing was built or run. Line numbers refer to the branch.

## Verdict

**Mergeable, no Critical findings.** Thread ownership holds: every Direct2D, DirectWrite and
bitmap object is created and used on the render thread, WIC runs only on the media worker, and
every cross-thread message to the UI thread is a post. Real hardware writes all go through the
`Services` trait onto the `plith-writes` thread. The fixture's writes stay inside the fixture.
Demo and idle runs issue no write, and a click on a collapsed island only expands.

Three Important items, to fix or track before the console run that owes the measurements:
- I1: a button release with no press on the island is a click, so a page write can fire.
- I2: page bitmaps are rasterized inside motion and ambient frames, which risks the draw p99
  and hitch bar. The media elapsed text forces one raster every second.
- I3: `--idle-check 60` with real services can run past the 75 s watchdog.

The lane's own known minor (`derive.rs:151`, a zero-length stream) is real and stays Minor (M1).

## Important

### I1. A release without a press on the island counts as a click, and clicks write hardware
`crates/plith-shell/src/window.rs:375-378`, `crates/plith-shell/src/window/mouse.rs:41-49`.

**What happens:** the shell turns every `WM_LBUTTONUP` inside the drawn shape into
`ShellEvent::Click`. It never checks that the button went down on the island; the branch has no
`WM_LBUTTONDOWN` handling and no capture. Before R4 a click only toggled the island. Now
`route_click` maps it to page actions: `SetBrightness`, `ToggleMic`, `PickOutput` (a Voicemeeter
route toggle), `SetDefaultEndpoint`, `PlayPause`, `Seek`. `app/input.rs:127-157` sends all of
them to the writes thread.

**Failure scenario:**
1. The island is open on the system page, for example 300 ms into the leave delay.
2. The person presses the left button in a window that does not capture the mouse, drags
   towards the top of the screen and releases over the brightness ruler.
3. Our window gets the `WM_LBUTTONUP`, `hit` resolves `BrightnessRuler` to `SetBrightness(x)`,
   and the monitor brightness changes. A release over an output row works the same way after a
   pick and toggles a Voicemeeter route. Nobody clicked the island.

**Fix:** use standard button semantics. On `WM_LBUTTONDOWN` inside the shape, record the
island point and call `SetCapture`. On `WM_LBUTTONUP`, emit `Click` only when a press was
recorded, then `ReleaseCapture`. Clear the record on `WM_CAPTURECHANGED` and on leave. A stricter
version also requires the press and the release to hit the same target. Add a shell test: an
up with no down emits nothing.

### I2. Page bitmaps are rasterized inside motion and ambient frames
`crates/plith-render/src/paint.rs:190-233` (`place_pages`), `crates/plith-render/src/pages.rs:92-133`,
`crates/plith/src/app/feed.rs:238-241` and `267-275`,
`crates/plith-core/src/pages/media.rs:243-249`, `crates/plith/src/demo.rs:99-102`.

**What happens:** `PageCache::draw` rebuilds a bitmap lazily, the first time a frame shows the
page. That frame runs text layout for every new string and draws the whole list. Nothing
pre-draws a bitmap before a motion. This frame is inside the measured motion or ambient run in
four common cases:
1. **Expand after a minute or more collapsed.** `flush` sees a new clock text and marks all four
   pages dirty (`feed.rs:268-272`). The first expand frame with `content_opacity > 0` then
   rasterizes the incoming page.
2. **A page turn to a page whose list changed since it was last drawn.** Every page changes each
   minute because of the clock. The raster lands in a turn frame.
3. **Media playing.** The elapsed time is static text in the bitmap (`media.rs:243-249`).
   `MEDIA_TIMER` rebuilds the page every second, and the list differs every second, so the
   whole media page is rasterized once per second inside the ambient run. That is 1 frame in
   about 60, or 1.7 % of ambient frames. That share alone puts the ambient draw p99 over 1 ms
   if one raster costs more than 1 ms. The plan's Task 13 interface said the position text is
   `Live::Seek`. As built it is not, and the "As built" note does not say so.
4. **The demo measures a different path from a real click.** `demo::play` publishes
   `expanded` first, then wakes the UI thread. The feed flushes from `on_scene` after the
   motion has started, so every `pages` demo expand gets a content swap mid-motion. A real
   click flushes first (`app/input.rs:42-51`). The Task 18 numbers would then measure the demo
   path, not the click path.

**Failure scenario:** the owed console run (Task 18 Step 3) misses "draw p99 under 1 ms" or
logs a hitch on expands and turns. That would mean redesigning after merge, and nothing in the
WARP tests can show it now.

**Fix:**
- Warm the cache before the first clock wait of a motion. After `take` returns `Aimed`, draw
  the bitmap of the target page while still at rest, outside the motion stats.
- On a `ContentOnly` snapshot at rest, draw the bitmaps of the changed lists even when they are
  not visible.
- Make the elapsed text a live node (or its own small cached run) so the media list stays equal
  for a whole track.
- Have the demo `expand` step go through the same flush, for example by having the UI thread
  apply demo scene steps.
- Track it in the plan: Task 18 Step 3 should log the ambient line on the media page while
  playing, not only on the audio page.

### I3. `--idle-check 60` with real services can outlive the watchdog
`crates/plith/src/idle.rs:13-15, 115-127`, `crates/plith/src/app/wire.rs:19`,
`crates/plith/src/app.rs:279-280`.

**What happens:** with real services the idle thread first waits for every start, up to
`START_CAP` (20 s). It then settles for 1.5 s and measures `secs`. The watchdog starts at the
same moment and quits after `WATCHDOG_SECS` (75 s). Worst case: 20 + 1.5 + 60 = 81.5 s.

**Failure scenario:** a DDC/CI monitor makes the brightness start take more than 13.5 s
(`start.rs:1` says up to 15 s). The watchdog closes the window at 75 s and `stop` cuts the
idle wait. The run exits 4 with no `idle` line. The G2 gate ("`--idle-check 60` must still
pass") then fails on a machine at rest. Separately, a real-services idle check also fails
whenever a pill fact changes during the window (a signal edge or a play state change). That is
correct, but the run instructions should say "no audio playing".

**Fix:** for idle checks, give the watchdog a budget of `START_CAP + SETTLE + secs + margin`.
Or start the watchdog after the start wait. Keep the fixture path (`board == None`) unchanged.

## Minor

**M1. A zero-length playing stream rebuilds the media page on every wake** (`derive.rs:151`,
the lane's known item; I agree it is Minor).
- What happens: `duration_s == 0.0` is finite, so `at_end` is true. The position is not floored,
  so `shift > 0` on every call. Each `facts()` call then gives new media facts, and the
  position keeps counting up on a stream with no length.
- Severity: only while the island is open. Collapsed, the change only marks `pending`
  (`feed.rs:200-210`), so G2 holds. The cost is one media list rebuild per service wake, plus
  one raster when the media page is visible.
- Fix: `at_end` only when `duration_s > 0.0` (floor otherwise). Treat a zero duration like
  "no timeline" for the ruler.

**M2. A timer tick while closed returns without killing the timer** (`feed.rs:223-226`).
- What happens: the comment relies on the queued wake to sync the timers.
- Scenario: a wake post fails (a full queue; `Coalesce` clears and drops it). The 4 Hz activity
  or clock timer keeps waking the UI thread while collapsed until the next scene change.
- Fix: call `self.sync_timers(mailbox)` in the early-return branch.

**M3. A wasted present before every expand** (`thread/run.rs:193-196`).
- What happens: `ContentOnly` always paints a frame. The expand flush publishes new page lists
  while the scene is still collapsed, so the render thread presents a frame identical to the
  last one (only the pill shows).
- Fix: on `ContentOnly`, paint only when a list that the current frame shows changed (`Arc`
  pointer compare). This also keeps G2 robust if a later publisher sends page content while
  collapsed.

**M4. Duplicated helpers and a second signal path.**
- What is duplicated:
  - `SIGNAL_PEAK` exists three times: `plith-audio voicemeeter/signal.rs:8`, core
    `pages/derive.rs:11` and core `pages/audio/model.rs:9`.
  - The 2 s hold exists twice: core `Activity` and `plith-audio` `SignalHold`.
  - `percent()` exists twice (`derive.rs:96`, `audio/model.rs:238`).
  - The bus tag is built three times (`derive::bus_tag`, `audio/model.rs:149-153`,
    `plith-audio` `fallback_label`).
  - Turkish upper casing exists twice: core `copy::tr_upper` and `plith-osd` `paint.rs:129`
    `upper_tr` on main.
- Suggestion: have the audio page read the debounced `vm.signal`, which already changes only on
  edges. That removes `Activity`, `ACTIVITY_TIMER`, the 4 Hz audio rebuilds and the UI-thread
  `levels()` call. Keep one `percent` and one `bus_tag` in core. The OSD switching to
  `tr_upper` is a wiring-job item for the osd owner.

**M5. `shapes::PAGES` still says shelf 214** (`shapes.rs:26`), but the shelf page is 170
(`pages/shelf.rs:13`). The plan's Global Constraints also say 214.
- Effect: none on screen, since `Feed::new` publishes the real shapes at once. The constant is
  misleading.
- Fix: set it to 170, or document `PAGES` as pre-feed defaults only.

**M6. `PickOutput` indices are not tied to the edition** (`app/input.rs:133-137`).
- What happens: `pick_within` only bounds the pick by the strip count.
- Scenario: Voicemeeter restarts as another edition with at least as many strips between a pick
  and the output click. The route toggle then goes to a different strip/bus pair. Unlikely.
- Fix: keep the edition (or the strip and bus counts) in the pick and drop the write on a
  mismatch.

**M7. ADR draft wording before filing** (`pages-r4-adr-drafts.md`).
- P5 says the ambient tick runs "at `hz / 4`". The code paints every `round(hz / 60)`-th tick:
  about 60 fps, and every 4th tick only at 240 Hz.
- P7 says "every own label is a constant in `copy.rs`". Own copy in other styles lives in the
  page modules: "Çalan bir şey yok", "Parlaklık", "Mikrofon", "Dosyaları buraya sürükle",
  "Kaynak N", "Kullanılmıyor".
- Fix: correct the wording, or move that copy into `copy.rs`.

**M8. Cross-lane follow-up: two text stacks.**
- What happens: `plith-osd/src/paint/text.rs` builds its own DirectWrite factory with system
  fonts, while the pages now bundle Public Sans and Martian Mono in `plith_render::TextSystem`.
  The OSD and the pages will set type differently.
- Fix: in the wiring job, decide whether the OSD adopts `TextSystem`. That is the osd owner's
  change, not this branch's.

## Not findings (checked)

**Cross-lane.**
- `git diff 0a73ade origin/feature/pages-r4 -- crates/plith-osd crates/plith-ipc crates/plith-system`
  is empty.
- `MotionKind::OsdShow/OsdHide` are intact (`motion.rs:62-77`), and `plith-osd/src/render.rs:245-247`
  still matches them.
- The OSD uses `Gpu`, `CompositionTarget`, `FrameSource`, `clock::open_clock` and
  `SurfaceLayout` fields. None of their files changed. The OSD builds its `SurfaceLayout` by
  hand (`app.rs:124`), so `TOP_GAP = 0` and `PAGE_MAX_HEIGHT` do not reach it.
- No root `Cargo.toml`, `Cargo.lock`, `clippy.toml` or `scripts/` change.

**Thread ownership.**
- `GpuPaint`, `Painter`, `PageCache`, `ListKit` and `TextSystem` are built in `render_main` on
  the render thread and rebuilt there on recovery.
- `TextSystem` uses an isolated factory, so its in-memory loader is private.
- The UI thread only builds display lists.
- `UiWaker`, `ShapePoster` and `QuitHandle` only post; starter, poll, writer and ticker threads
  never send.
- The `App` borrow is never held across a call that pumps messages: services, timers and the
  mailbox do not pump, and writes are queued.

**Hardware writes.**
- `dispatch` (`app/input.rs:127-157`) is the only path to a write, and it is reached only from
  `ShellEvent::Click`.
- `RealServices` queues every write on `plith-writes` (`services.rs:311-341`).
- `FixtureServices` writes mutate the fixture only (`fixture.rs:337-404`).
- Demo `content` steps reach only the fixture channel, and are ignored and logged without
  `--fixture` (`demo.rs` `content_requester`). The idle check issues none.
- `route_click` returns `Expand` while collapsed, so a pill click never runs an action.
- An output click without a pick writes nothing (`input.rs:133-137`).

**Shutdown order.** The render thread is joined before `stop_feed`. The writer is stopped before
the services, and queued writes are dropped. Starter threads still inside `start()` are left
alone and logged.

**Mailbox.** `update` and `publish_content` skip no-op publishes. The deep compare runs outside
the lock with a ptr re-check. `Pages::rebuild` keeps the `Arc` of an equal list, so the cache
keeps its bitmap.

**G2 logic.**
- Collapsed, `on_wake` rebuilds only the pill and parks page changes in `pending`.
- `on_scene` kills every UI timer on collapse or suppression, and the collapse tick kills its
  own.
- The render thread parks when `wants_ambient` is false.
- The fixture ticker changes only brightness (system page), never the pill.

**Collapse state machine.**
- `CollapseTimer` keeps the first leave and cancels on `Enter`.
- An early tick re-arms for the remainder, and a stray tick kills the timer.
- A due tick ends a pick, then collapses.
- Leave while collapsed is ignored.
- `on_click` clears `tracking` without cancelling the system request, so a flick out after a
  click still delivers `WM_MOUSELEAVE`.

**Click routing.**
- The click point and the composited pages share the current island origin, so targets match
  what is drawn during morphs.
- The page on screen comes from the last drawn `ShapeFrame.pages`.
- Hidden output rows still map `PickOutput(o)` to bus `o`.

**Device loss and COM.**
- The page cache, the art bitmaps and the dash styles die with the `Stack`.
- `render_into` restores the target and text mode even when drawing fails.
- The art buffer length is checked before `CreateBitmap` (`list.rs:305-307`).
- WIC objects are locals dropped before the borrowed bytes (`art.rs:134-180`), on the media
  worker's COM apartment.

**Structure and conventions.**
- Largest source file on the branch: 497 lines (`plith-media/src/service.rs`). Largest test:
  657 lines.
- No `mod.rs`. No em dash in any branch-touched file. No `windows` use in `plith-core`.
- `clippy.toml` sets `too-many-lines-threshold = 80`, and the branch adds no `too_many_lines`
  allow.
- Own copy is written upper-case with Turkish rules (`copy.rs`). Foreign names get invariant
  `to_uppercase` (`text.rs:369-372`).

**Plan honesty.**
- Every test named in a ticked step exists, except two renamed with their deviation recorded or
  implied by an answered question:
  - `shelf_empty_is_170_high` (log: "empty shelf 170").
  - `click_expands_and_empty_space_keeps_it_open` (Q10 replaced toggle with click-to-expand).
- The unticked steps are exactly the on-screen items: Task 4 Step 4, 7a Step 5, 16 Step 4 and
  18 Steps 3 and 4. The progress log lists them as unmeasured.
- No ticked box claims a measurement. The only recording gap is the Task 13 position text
  (I2, point 3).
