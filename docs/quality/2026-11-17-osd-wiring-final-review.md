# Final whole-branch review: feature/osd-wiring (192fe6c..a4249f7)

Reviewer method: superpowers `code-reviewer.md`. Read-only. The 3,645-line diff was read in four
passes: (1) header, Cargo and plith-core (`launch`, `osd`, `osd/policy` and tests); (2) plith-ipc
security, plith-osd (`paint`, deleted `paint/text.rs`, `render`, `render/stack`); (3) crates/plith
`app.rs`, `backstop`, `feed`, `feed/osd`, `feed/tests`, `state`, `wire`, `instance`, `main`;
(4) crates/plith `osd.rs`, `osd/flyout`, `osd/health`, `osd/supervisor` and tests, plan and spec.
Code outside the diff read to check cross-task claims: `plith-ipc/src/client.rs` (link worker),
`plith-render/src/text.rs` (layout cache, Label upper-casing), `plith-system` flyout `Drop` and
`restore_now`, `app/wire.rs` (panic hook, watchdog), `app.rs::run_shell`, `copy.rs` constants.
No test was run: no concrete doubt needed one.

## Strengths

- The flow is cleanly layered and matches the plan: pure decisions in plith-core
  (`osd::changes`, `policy::Relaunch`, `wants_suppression`, `parse_flyout`), a pure supervisor
  state machine in `crates/plith/src/osd/health.rs` with 14 scenario tests (connect-then-die
  give-up, stale link, crash behind a stale link counting once, stable-up credit), and only the
  Windows glue in `supervisor.rs` / `flyout.rs`.
- feed -> sink -> link is non-blocking end to end: the sink holds a `Weak<OsdLink>`
  (`osd.rs:sink`), `OsdLink::send` is encode + slot put, and nothing on the supervisor thread posts
  to the UI thread. Review Focus 5 holds.
- Drop order in `app.rs::run_shell` is right: render joined, feed stopped, then `drop(osd)`
  (supervisor stops suppression and shows flyouts, then the last strong link reference sends
  `Bye`). The early `return 1` after `start_osd` drops `osd` by scope. Exit is bounded:
  `join_within(STOP_WAIT 1.5 s)` plus the link's ~1 s drop, with `restore_now()` when the
  supervisor is left behind (M2 fix).
- "A down link never leaves the flyout hidden" is held in depth: `wants_suppression` gates Hide on
  `Health::link_up()`, which also distrusts a link whose helper exited (stale guard); a failed
  `stop` restores through the handle (M1 fix); both `process::exit` paths call `restore_now()`;
  and plith-system restores on hook-thread unwind and journals for a crash.
- No spinning: `LAUNCH_SPACING` 3 s, give-up after 5 in 60 s, `STABLE_UP` 10 s so a
  connect-and-die helper still reaches give-up (the T1..T3 carry was honoured). The real run saw
  the 3 s spacing.
- Single instance is small and correct: the guard is a named binding living to the end of
  `run_logged`, probing runs skip it, `Err` continues.
- Labels: device names go as is (the helper's `TextStyle::Label` upper-cases invariantly through
  `TextSystem::display_text`), the fallback `TAB_AUDIO` is already `"SES"`, the muted word is
  `copy::MUTED_LABEL`. The label cut at 64 bytes on a char boundary is tested with `ğ`.
- Commit subjects 29 to 49 chars, Conventional, no attribution in subject or body; no em dash in
  any added line; all touched files far under 500 lines.

## Issues

### Critical (Must Fix)

None.

### Important (Should Fix)

1. **Task 8 docs are not on the branch** (`docs/plans/done/wiring.md:147-148`,
   `docs/plans/done/wiring.md:45-146`, `docs/STATUS.md:26`, `crates/plith-ipc/README.md:52-85`).
   - What: the plan's Record section is empty and no box is ticked although Tasks 1 to 7 are
     done; STATUS still lists the wiring job and M8 as next; the README "Wiring into
     crates/plith" section still reads as a to-do list and does not state what was built (3 s / 5
     in 60 s / 3 s grace / 10 s stable-up, one supervisor thread, `--flyout` default off, failure
     lines logged once until the link comes up, elevated runs never start the link).
   - Why: project-docs rule (tick as work progresses, STATUS rewritten) and Task 8's own step; the
     next session reads STATUS first and would re-plan finished work.
   - Fix: fill the Record from the ledger (including the run facts below), tick Tasks 1 to 7,
     rewrite STATUS, turn the README Wiring section into a description of the wiring (keep step
     5 and 6 as open items). ADR 0012 line 19 is immutable; STATUS should say the item is done.

2. **The end-to-end acceptance is unproven** (spec `docs/specs/2026-11-17-osd-wiring-design.md:67-68`).
   - What: no run has shown `osd: link connected`, an admitted client in `plith-osd.log`, a
     `Level` reaching the helper or `Bye` ending it. The installed helper predates the SID pipe
     name, hello admission and keepalive, so the run could only show launch, spacing and give-up.
   - Why: unit tests and the measured failure path cover every decision, but nothing has seen the
     wiring work. The harm if broken is small (default `--flyout off`, so the result is "no OSD"
     plus five logged launches), which is why this is not Critical.
   - Fix: the signed reinstall run (Berke's yes) before merge, or Berke's explicit call to merge
     with it recorded as owed in STATUS and the plan Record.

### Minor (Nice to Have)

1. `crates/plith/src/app/feed.rs:195`, `crates/plith/src/app/feed/osd.rs:26`: `feed_osd` builds a
   reading (two label `String`s, an endpoint search) on every wake, also scene-only wakes.
   Correct, cheap; gate on `changes.any() || seen.is_none()` (deferred T4+T5 minor; optional).
2. Feed-level brightness / System page test missing (deferred T4+T5); the pure diff covers it.
3. `crates/plith/src/osd/supervisor.rs:181` and `crates/plith-ipc/src/client.rs:22`: a normal run
   now carries about 3 background wakes per second forever (supervisor 1 Hz, link worker 2 Hz
   while connected or 1 Hz retrying, also after give-up: deferred M3). None reach the UI or
   render thread, so the spec's anti-goal holds, but the idle check cannot see them (probing runs
   start no OSD). Measure per-process wakes once hands-off; later consider waiting on the helper
   process handle and the stop channel instead of ticking when the mode is `Off` and the link is up.
4. `crates/plith/src/osd/health.rs:181`: give-up is permanent for the session (the spec's
   choice). A burst of shell failures at logon (autostart before Explorer is ready) would leave
   the whole session without an OSD. Plan-level suggestion: one slow retry every few minutes
   after give-up.
5. `crates/plith/src/osd/supervisor.rs:133`: `Notes` logs identical failure texts once until the
   link comes up; the spec says every failure is logged. Sensible deviation, document it (Important 1).
6. Deferred M4: a supervisor panic is noticed only at exit. Suppression restores on unwind and is
   journaled, so no hidden flyout; relaunching stops silently. Acceptable.
7. Deferred M5: `Down` while our helper is alive lets the handle go and launches a second helper,
   which exits "already served" and costs a second failure.
8. `crates/plith-ipc/src/security.rs:38` uses `{e:?}` in a user-readable log text; the
   `restore_now_without_a_suppressor_is_a_no_op` test asserts nothing (deferred M6).
9. `crates/plith-osd/src/render/stack.rs:84` with `crates/plith-osd/src/render.rs:390`: a text
   failure is now fatal for the helper's render thread (old `DrawText` could not fail). A
   persistent DirectWrite layout failure would end the helper on each `Level` and use up the
   relaunch budget. Consider logging once and drawing without text.
10. `crates/plith-osd/src/paint.rs:335`: `SetTextAlignment` mutates a layout owned by the
    `TextSystem` cache. Safe today (Label and Value keys never collide); fragile if a style is
    shared. Brush clones per draw at `paint.rs:255,262` (deferred T7).
11. `crates/plith/src/app.rs:54`: `--fixture` runs take the instance guard, so a fixture dev run
    beside a running notch exits 0 with only a log line. The plan names demo and idle-check
    only; make it explicit or treat fixture runs as probing.
12. OSD triggers that are not key presses (console check): a brightness or default-endpoint
    change made on an open page whose reading lands after the island collapsed (DDC/CI lag),
    and on laptops adaptive brightness or battery-saver dimming, will pop the OSD.
13. `crates/plith-osd/src/paint/tests.rs:96` `eprintln!` in a test; `take_notes` lacks
    `#[must_use]`; number now 10 DIP regular and ruler 136 DIP (deferred T7, console look).
14. `crates/plith/src/osd.rs:131`: on the drop timeout `restore_now()` runs while the left-behind
    suppressor still hooks; a volume key in the last milliseconds before exit could hide a flyout
    again. The journal restores it on the next start. Very low.

Deferred-minor triage: none of the ledger's deferred minors must be fixed before merge. Worth
doing in the same docs commit if cheap: Minor 1 (one-line gate).

## Declined to judge

- Helper hang (alive, pipe accepting writes, nothing on screen) with `--flyout hide`: no health
  signal exists for it; outside the plan, unmeasured.
- Session end / logoff (`WM_ENDSESSION`) and hard kills with Hide active: relies on the
  plith-system journal, which predates this branch.
- Standard-user integrity of the UIAccess helper: spec non-goal (release check).
- Visual result of the M8 type change (sizes, ruler width): owed to the console session.
- Version skew between a dev `plith.exe` and the installed helper (the real-run failure): an
  install concern, not this branch's code; only noted under Important 2.
- `OsdLink` worker cadence (POLL 500 ms, RETRY 1 s): plith-ipc design from R6, only its effect
  is noted in Minor 3.

## Recommendations

- Record in STATUS that the 60 s idle check cannot regress from this branch (no sink and no guard
  in probing runs), so the owed hands-off re-run is a sanity check, not a gate.
- Add a dev path to point the supervisor at a locally built helper with `DevSameUser` trust
  (behind a flag), so the wiring can be seen working without a signed reinstall.

## Assessment

**Ready to merge? With fixes**

**Reasoning:** The code is sound and the cross-task lifecycle (drop order, bounded exit, restore
on every exit path, no UI-thread posting, bounded relaunch) holds; no Critical or code-level
Important issue was found. Before merge the Task 8 docs must land, and the connected-link run
must either pass after the signed reinstall or be explicitly accepted as owed by Berke.

## Deferred minors from the task reviews (master ledger, 2026-11-17)

- Task 6: minor (deferred): M3 two 1 Hz wakes after give-up (doc in spec); M4 supervisor panic unnoticed until exit; M5 Down while helper alive launches a second helper; M6 {e:?} in own_integrity, weak no-op test.
- Task 7: minor (deferred): brush clones per draw; take_notes lacks must_use; number size 13->10 DIP judged at console (possible shared Readout style).
- Task 7: minor (deferred): ruler now 136 DIP (ticks 13.6 apart) and number 10 DIP regular, console look; eprintln in test.

Fix wave after this review: `a30119c`, `1e911aa`, `2e4dfca` (Minor 1, 9, 11 and no console window in release); re-review approved.
