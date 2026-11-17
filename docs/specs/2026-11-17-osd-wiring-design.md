# OSD wiring into crates/plith (short design)

Date: 2026-11-17. Approved by Berke the same day ("önerin hangisi ise devam edelim"): approach A
below, plus the master's proposals in this file.

## Intent
R6 built `plith-osd.exe` and the `plith-ipc` pipe, but nothing starts or feeds them. This job
makes `plith.exe` start the helper, keep the link healthy and send a `Level` on every volume,
mute and brightness change, so the OSD appears in normal use. It also gives the OSD the same type
as the pages (R4 review M8). Source notes: `crates/plith-ipc/README.md` "Wiring" (8 steps) and
the osd Record in `plans/active/osd-r6.md`.

## Goals
1. **Single instance.** A named mutex per user session; a second `plith.exe` logs
   `single instance: another plith runs` and exits 0. Needed before flyout Hide exists.
2. **Helper and link.** Normal mode with real services only: `launch_helper(INSTALLED_HELPER)`
   once at start on a helper thread (it blocks and wants COM), `OsdLink::start(current_pipe_name())`
   for the session. Health per README step 4: relaunch when `has_exited()` or the link stays down
   for a few seconds, at most one launch per few seconds, give up after a handful of quick
   failures in a row, every failure logged with `last_error()` to `plith.log`. An elevated notch
   (own integrity above medium) logs that the link fails closed and does not retry (step 6).
   Exit drops the link (`Bye`).
3. **Triggers.** When a service drain changes the followed endpoint's volume percent or mute, or
   the brightness percent, send `Level`. The device name goes as is (the helper upper-cases it
   invariantly); Plith's own Turkish fallback labels go through `plith_core` `tr_upper` first
   (step 7). No OSD while the island is open on the page that already shows that value (audio
   page for volume, system page for brightness). The first reading after start is not a change.
4. **Flyout suppression, approach A.** Launch option `--flyout off|dry|hide`, default `off`.
   `hide` starts `FlyoutSuppressor` in `Mode::Hide` and the volume key hook with its
   `suppression_window()`; it runs only while the link is up and stops when the link goes down
   (otherwise the person gets no OSD at all). A start failure (another process owns the journal)
   means suppression off, logged once, never retried in a loop. `restore_handle().restore_now()`
   runs on the close backstop and watchdog exits (M7). Brightness suppression is not wired
   (review I3, unmeasured).
5. **M8.** The helper draws with `plith_render::TextSystem` (bundled Public Sans and Martian
   Mono) instead of its own system-font factory, and its `upper_tr` is replaced by
   `plith_core` `tr_upper`, so one rule and one type set exist.

## Non-goals
Settings UI for the OSD or suppression (R7), brightness suppression, the standard-user integrity
measurement (release check), a random pipe name per launch, OSD in fixture, demo or idle-check
runs (they stay measurement runs with no helper process).

## Anti-goals
The OSD path must not add UI-thread blocking, wakes at rest (idle check stays at 0 wakes) or a
retry loop that spins when the helper cannot start. A down link must never leave the native
flyout hidden.

## Approaches (for goal 4)
- **A, picked:** wired, default off, `--flyout hide` for the console run; a setting comes in R7.
  Nothing changes on Berke's PC until he turns it on, and Task 9 measures it first.
- **B:** on by default for volume. Rejected for now: a system behavior change before its
  on-screen measurement, and a hardware-class write that needs a separate yes.

## Constraints
Pure decision logic (trigger diff, relaunch policy, `--flyout` parsing) lives in `plith-core`
with unit tests; Windows calls stay in `crates/plith` and `plith-osd`. File limits 500 / 150 /
800 / 80 per function. `check-deps.mjs` already allows `plith -> *` and
`plith-osd -> plith-render, plith-ipc, plith-core`. Builds and tests through `heavy-run.mjs`.

## Left to the builder
Exact relaunch numbers (proposal: 3 s between launches, give up after 5 failures within 60 s),
module names under `crates/plith/src/osd*`, how `TextSystem` exposes the formats the OSD needs.

## Verification
- Unit tests for the pure parts; five checks green.
- On this PC (RDP is fine): a normal `plith.exe` run logs the helper launch and a connected link
  in `plith.log`, and `plith-osd.log` shows the admitted client; a second `plith.exe` exits 0.
- Owed to the console session: what the OSD looks like with the new type, suppression in
  `--flyout hide`. The installed helper under Program Files is the R6 build, so the M8 change is
  seen only after a signed reinstall, which needs Berke's yes (batched with the console run).
