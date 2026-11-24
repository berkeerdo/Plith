# R5 Shelf Implementation Plan

**Goal:** Drag files into the island (hover opens the shelf), keep them as references, drag
them out with a real shell data object. One surface, in-process.

**Spec:** `docs/specs/2026-11-24-r5-shelf-design.md` (main checkout, not committed yet), ADR 0018.
Facts: `docs/research/2026-11-24-r5-shelf.md`.

## Constraints
- Code, comments, tests in English; strings Turkish plus the other seven languages. No em dash.
- 500 lines per source file, 80 per function, `foo.rs` + `foo/bar.rs`, never `mod.rs`.
- Pure logic in `plith-core`; Windows code only in `plith-shell`; every COM body in `catch_unwind`.
- No commit, no push, no plith.exe, no mouse driving, no scripted drag.

## Steps (tick only when the test passes)
- [x] 1. Core store: add, front on re-drop, cap 8, remove, undo, parse filter, serialize (`shelf`)
- [x] 2. Core type tag from the extension and the two-line name split (`shelf/kind`, `shelf/name`)
- [x] 3. Core wagon layout for 0 to 8 items and hit targets (`shelf/layout`)
- [x] 4. Core drag hover state machine: dwell 150 ms, grace 380 ms, own drag flag (`shelf/hover`)
- [x] 5. Shelf page from items, hot slot, footer, undo; actions `ShelfWagon`, `ShelfRemove`, `ShelfUndo`
- [x] 6. Eight-language strings and fit places
- [x] 7. App shelf file (`shelf.txt`, atomic save, load filter)
- [x] 8. Feed holds the shelf, rebuilds the shelf page, saves on change
- [x] 9. Shell: `IDropTarget` on the island window, events, mouse state reset (compiles; the OLE path is unproven until a real drag)
- [x] 10. Shell: drag out with a shell data object and `IDropSource` (compiles; only the verdict is unit tested)
- [x] 11. App wiring: hover machine, open on dwell, drop, press and drag out, `drag` log line
- [x] 12. Offscreen renders 0, 1, 4, 5, 8 items and hot slot in `docs/design/r5/`
- [x] 13. Checks: fmt, clippy, tests, structure-check

## Progress log
- 2026-11-24 14:00 core store, tag, name split, layout, hover machine and the shelf page done; plith-core suite 597 passed (48 shelf tests). Shell side and app wiring written, waiting on the machine lock for the first compile.
- 2026-11-24 14:55 shell drop target and drag out, app wiring, shelf file, renders (docs/design/r5, 10 PNGs). Checks: fmt clean, clippy -D warnings clean, workspace tests 1336 passed 0 failed, structure-check 816 files 0 violations. A ticked box means the step was taken: nothing here was driven by a real drag. Open: drag image over the DirectComposition window, hover feel (150 ms / 380 ms), IDragSourceHelper with a null window, UIPI, drop onto Explorer and a browser.
- 2026-11-24 review fixes: I1 (GetKeyState, logical button), I2 (rearm_leave after drop and after a drag out ending inside), I3 (KLASOR/DOSYA tags in 8 languages plus fit places), I4 (newest() cuts a drop to 8 before probing, read cap 64), I5 (load on a thread, only NotFound drops a line, no save until the load arrived), M2 Agile=false, M3 ptd freed, M4 tymed check, M5 OsString paths and no lossy save, WM_CLOSE deferred during the drag out, drag log gained effect, point and enters (open_ms kept across a grace return), empty shelf with undo is 198 high with the word under the siding.
- Deferred minor items: M1 events queue (Wake and Timer are not coalesced) while the drag out runs, app is frozen for its length; M6 exists() and SHParseDisplayName on the UI thread for an offline wagon; M7 Esc during a drag-in starts the grace collapse under a still pointer; M9 IDropTargetHelper is created at start, not lazily; M10 Layout::undo is dead and differs from the real target; M11 footer splits GERI AL out of the sentence (design change to confirm with Berke); M12 shelf file is saved on the UI thread.
