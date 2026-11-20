# R7 settings window: final whole-branch review

Range 6fa8235..8f5306b (feature/settings-window), reviewer: Task 11 final review (opus).
Read in passes: the spec, the plan, the ledger, then the wiring end to end (app.rs, state.rs,
state/live.rs, state/sheet.rs, sheet_side.rs, saver.rs, osd_slot.rs, weather_link.rs,
feed/settings.rs, feed/idle.rs diff, input.rs + input/quick.rs, pages.rs diff), the sheet model
(sheet.rs, keys.rs, field.rs, general/osd/about.rs, idle.rs head), the sheet renderer
(render/sheet.rs, sheet/target.rs, sheet/paint.rs, list/inset.rs, measure.rs), the shell window
(sheet_window.rs, proc.rs, keys.rs), settings/change.rs, settings.rs + launch.rs + ipc diffs,
ADR 0017. No tests were re-run (the task reports cover them); `node scripts/structure-check.mjs`
was run read-only: 780 files, 0 violations.

## Strengths
- One change path, as the spec asks: window click, window key and quick page look all end in
  `App::change` (state/live.rs:76), which applies `apply_change`, the feed, weather, OSD, then
  queues the save. A value already set does nothing (no rebuild, no write).
- The OSD slot (osd_slot.rs) is a clean state machine: stops never block the UI thread, a start
  waits for the old helper's pipe to go (`after_stop` + `pipe_exists`), a mode change while a
  stop is pending is carried by `self.mode`, and the feed's sink is a `Weak` so a stopped side's
  sink goes quiet by itself.
- Thread ownership holds: the `plith-sheet` thread owns its own `Gpu`, DComp target, swap chain,
  brushes and `TextSystem`; the island render thread is untouched. Close order is right: the
  render thread is joined before `DestroyWindow` (sheet_side.rs:330, and `WM_CLOSE` only reports,
  proc.rs:262).
- Cost at rest with the window closed: nothing new wakes. The saver blocks on a channel, the
  caret timer exists only while the field has focus (killed in `close_sheet` via `sync_caret`),
  the minute clock for the previews is tied to `sheet_open` and dropped on close, the sheet
  thread waits on a condvar and is gone after close, previews and text caches are dropped.
- Shutdown honours Review Focus 5: `finish_settings` (close sheet, commit a typed city, flush the
  saver) runs right after the message loop and before the render, feed and OSD stops.
- Re-entrancy is handled deliberately on both windows: the sheet window queues events until
  `open` returns and after a panic it mutes; the app's callback only queues and wakes.
- Pure logic really is in `plith-core` with tests (sheet layout and hit test, keys, field, change
  mapping, settings round trip), and every file stays under the limits; the near-limit files of
  the plan grew by only a few lines (feed.rs +4, list.rs +8).

## Spec section 6 traced end to end
Each row from a click (window or quick page) to the visible effect and the saved file:

| Row | Click -> change | Live effect | Saved |
|---|---|---|---|
| Dil | general.rs `Lang(choice)` | feed lang + all pages + pill (feed/settings.rs:41), window title and list (`Shown` holds lang), OSD word on the next level (feed lang) | yes |
| Ada açılınca | `OpenOn` | `Idle::set_settings`, read by `Feed::open_page` on the next open | yes |
| Kapalı ada (window and quick page) | `Look` | rest shape in the mailbox + pill + quick page (`with_quick`) | yes |
| Şerit, Sağ üst, Bugün çalanlar | `Span`/`Head`/`Plays` | welcome dirty; plays on loads the log and saver once (`enable_plays`) | yes |
| Hava | `Weather` | `WeatherWiring::apply`: start, or `stop_detached` off the UI thread; forecast cleared | yes |
| Konum, Şehir | `Location`/`City` (field commits on Enter, Tab, click away, close) | reconfigure + fresh fetch; while off only the settings change | yes |
| Birim | `Unit` | pill + welcome (and so quick page and window previews) | yes |
| Ekran göstergesi | `Osd` | `OsdSlot::apply`: stop on a short thread, start after; sink handed on the next wake | yes |
| Windows göstergesini gizle | `Flyout` (dead while osd off) | restart in the new mode; `--flyout` wins | yes |
| Hakkında | `OpenFolder` | explorer on a short thread | n/a |

Every row is delivered. The one gap is the reverse direction (quick page -> open window), Minor 1.

## Issues

### Critical (must fix)
None.

### Important (should fix)
None.

### Minor (nice to have)

1. **A look picked on the quick page does not refresh an open settings window at once.**
   crates/plith/src/app/input/quick.rs:11 -> state/live.rs:76. `App::change` never calls
   `refresh_sheet`; only `drain_sheet` (a Wake) and the `Timer` arm do. A look click on the
   island arrives as `ShellEvent::Click`, so the window's Boşta previews keep the old picked look
   until the next wake or the minute tick (at most 60 s, because the clock runs while the window
   is open). The spec wants every change shown at once. Fix: call `self.refresh_sheet()` at the
   end of `App::change` (it returns at once when no window is open), and add a state/sheet test
   for it.

2. **`--idle-look` leaks into the settings file.** idle_wiring.rs:33 puts the command line look
   into `Settings`, and the first change of any kind saves it (`App::change` saves the whole
   struct). `--flyout` was kept apart for exactly this reason (`OsdControl.cli`). Either keep the
   override out of the saved copy, or note that `--idle-look` in a normal run is a dev flag whose
   value is persisted.

3. **The flyout row is a switch that does nothing while `--flyout` is on the command line.**
   sheet/osd.rs:28 enables it whenever `osd` is on; `flyout_mode(cli, pref)` then ignores the
   change (the slot correctly does not restart). Spec 6 says the command line wins, and the
   anti-goal says no dead switch. Cheap fix: dim the row while a CLI flyout is set (needs one more
   fact in `SheetFacts`). Dev-only path, hence Minor.

4. **About shows no build date.** Spec 6: "`0.1.0` and build date"; about.rs shows only
   `CARGO_PKG_VERSION`. Either add a build date (a `build.rs` env var) or amend the spec line.

5. **The Playing title vanishes from the previews while its one-shot scroll runs.** A long title
   with a running scroll is a `Node::Live(Scroll)` (idle_pill.rs:191), and Live nodes are skipped
   inside an `Inset`, so for those seconds the Karot/Playing preview (quick page and window) shows
   no title. Fallback: the preview build could pass `scroll_start: None` so it gets the static,
   cut text.

6. **Opening the window blocks the UI thread** on `SheetRender::spawn` (waits for device and swap
   chain) and on `TextMeasure::new` (a DirectWrite factory and the bundled fonts) inside the
   island click (sheet_side.rs:222 and :227). Already parked in the ledger; measure the
   click-to-window delay in the console session, and if it is noticeable create `TextMeasure`
   lazily on first field focus.

7. **Committing a typed city at exit reconfigures the weather during shutdown.**
   `finish_settings` -> `close_sheet` -> `change(City)` -> `WeatherWiring::apply` starts a fetch
   (or a new service) just before `stop_feed` drops it. Harmless (the link is dropped, not
   joined), but a wasted network call; the exit path could save the city without applying it.

8. **Records.** The plan has no ticked box and its Progress log is empty (the ledger lives in
   `.superpowers/`, which is not part of the branch); project rule: tick boxes as work
   progresses. ADR 0017 Consequences still says "The look click and the settings row only log a
   line until ... task 10", now false, and it does not record Ruling 7 (DirectWrite measuring on
   the UI thread, `TextMeasure`), a visible exception to spec 3 "the UI thread never draws"
   (it measures, it does not draw, but it is a DirectWrite object on the UI thread). Fix both
   while the ADR is still unmerged, with Task 11 step 4.

9. **Parked string minors still open.** Italian `WordPlaying` = "IN ASCOLTO" while the look name
   is "IN CORSO" (the same thing named twice, lang/sheet/it.rs:53 vs lang/it.rs:18); Spanish
   `RowPlayed` "Reproducido hoy" (the songs: "Reproducidas hoy").

10. **Parked render minors.** Task 3: a test for a node after the inset and for missing art inside
    an inset; Task 6: rebuild on any frame error, not only device loss (sheet.rs:288 rebuilds on
    every first error, which is fine but unmeasured); device-loss path unmeasured.

Files near the 500 line limit (for the next feature, not a defect): plith-shell/src/window.rs
466, plith-render/src/text.rs 452, plith-render/src/list.rs 449, plith/src/app/feed.rs 434,
plith/src/app.rs 418 (grew 349 -> 418 in this branch), plith-core/src/draw.rs 414. Tests:
plith-render/tests/paint.rs 747 of 800.

## Ledger triage (what must be fixed before merge)
- Must before merge: none of the parked findings is a correctness or cost-at-rest defect.
- Should before merge (small, cheap): Minor 1 (one line plus a test), Minor 8 (records and ADR
  wording, part of Task 11 anyway).
- Can wait for the console session: Minor 5, 6 (measure first).
- Can wait: Minor 2, 3, 4, 7, 9, 10.
- Still owed by the plan, not a code finding: Task 11 step 1 idle check after opening and closing
  the window (spec 9, anti-goal "cost at rest"), and the Task 12 console session. Reading the
  code, I expect 0 new wakes; it is not measured yet.

## Declined to judge
- Pixel layout and visual quality of the sections and quick page (renders exist in
  docs/design/r7/settings-2 and quick-1; judged by the task reviews and the console session).
- Translation quality beyond the two parked strings: I cannot verify all eight languages.
- `pipe_exists` semantics of `WaitNamedPipeW` against a busy helper on real hardware: logic
  matches the docs; only a live run proves it.
- Re-entrancy of island messages sent synchronously while `finish_settings` holds the app borrow
  (DestroyWindow of the sheet after the loop): the island is already destroyed on a normal exit
  and probing runs never open the sheet, so no reachable path found; not proven either way.
- Device-loss rebuild of the sheet thread: unmeasured by design (ledger).
- Whether arrow keys in a choice group should apply the change at once (they do): the plan says
  so, the spec only says "arrows move inside a choice group".

## Assessment
**Ready to merge? Yes, with the small fixes above recommended (Minor 1 and 8) and the plan's
own idle check and console session still owed.**

Reasoning: every spec section 6 row is delivered end to end and saved through one path; thread
ownership, close and shutdown order, and cost at rest are sound by reading; the remaining
findings are small, mostly edge or record issues, none blocking.

## Fix wave report


Commits: 22541c0 (strings), 4e6798a (ADR 0017), de6025d (code minors).

Commands (all through cargo.mjs): `test --package plith-core` (538 ok), `test --package plith`
(254 ok), `test --package plith-render` (all ok), `clippy --workspace --all-targets -- -D warnings`
(clean), `cargo fmt --all`, `node scripts/structure-check.mjs` (781 files, 0 violations).

1. Look pick refreshes an open window: `App::change` ends with `refresh_sheet()` (state/live.rs).
   Test `every_change_refreshes_an_open_settings_window` uses a cfg(test) refresh counter in
   `SheetHost` (a real window cannot open in unit tests); a no-op change does not refresh.
2. `--idle-look` not saved: `App.file_look` holds the file's look while the CLI overrides it;
   `settings_and_file_look` (idle_wiring.rs) supplies it, `keep_file_look` stores it, saves write
   it until a Look change (even to the shown look) unpins. Tests: `the_command_line_look_is_not_saved_with_another_change`,
   `picking_a_look_saves_it_even_when_it_is_the_one_shown`.
3. Flyout row dim while `--flyout` is set: `SheetFacts.flyout_locked` (from `OsdControl::flyout_locked`),
   osd.rs uses `s.osd && !flyout_locked`. Test `flyout_row_dead_while_the_command_line_sets_it`.
4. Build date: `crates/plith/build.rs` sets `PLITH_BUILD_DATE` (UTC, SystemTime, civil-from-days),
   `SheetFacts.build_date`, About shows `0.1.0  2026-11-20`. Test `about_shows_the_build_date_next_to_the_version`;
   built value checked against `date -u`.
5. Static previews: `static_previews` builds the six previews with `scroll_start: None`; the stored
   comparison facts keep the real value. Test `previews_show_the_title_while_its_scroll_runs`.
6, 10: left alone.
7. City at exit: `finish_settings` -> `close_sheet_with(false)` -> `save_change` (record and save, no
   apply). Tests `a_city_committed_at_exit_is_saved_but_not_applied` (link survives; saved file has the
   city) and the contrast `a_city_change_while_running_does_apply`.
8. ADR 0017 consequences updated (change path wired; UI thread `TextMeasure` measures only, never draws).
   Plan not touched.
9. Italian `WordPlaying` = "IN CORSO"; Spanish `RowPlayed` = "Reproducidas hoy"; the fit test in
   plith-core passed.
