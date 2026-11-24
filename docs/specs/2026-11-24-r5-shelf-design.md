# R5 Shelf: in-process drag in and out

Date: 2026-11-24. Status: design, my call under Berke's "önerin ne ise devam edelim"
(2026-11-24); he can veto any point. Facts: `research/2026-11-24-r5-shelf.md`. Look: R3
direction (`specs/2026-11-14-r3-visual-direction.md:48`, mock `design/r3/ray-paneli.html:219-241`).
Input: ADR 0011 as amended by ADR 0018.

## 1. Problem
The WPF shelf "feels wrong" (a second process, a window swap, a blank page during the turn, one
line names). The Rust shelf page is an empty display list; nothing accepts or gives a drag.

## 2. Goals, non-goals
Goals
- Drag files or folders in from Explorer (and any app that offers `CF_HDROP`) onto the island;
  a 150 ms hover opens the shelf (ADR 0018). Copy by default, Link with Alt, never Move: the
  shelf stores references, it never copies, moves or deletes a file.
- Drag wagons out to Explorer, a browser upload field, chat apps: a real shell data object
  (`SHParseDisplayName` to an `IShellItemArray`, `BHID_DataObject`), so Windows draws the image.
- Look as the mock: wagons with a type tag from the extension, the full name on two lines, on
  stations on one track; a dashed slot `BIRAK` after the last wagon (hot while a drag hovers);
  footer `N DOSYA · SÜRÜKLE, BIRAK, GERİ AL`. Empty shelf as built today.
- Capacity 8, two rows of 4; the page grows by one row when a fifth item lands (no wheel, ADR
  0011). A ninth drop pushes the oldest out. A re-drop moves the item to the front, no copies.
- Remove: a click on a wagon's station removes it from the shelf (the file is untouched);
  `GERİ AL` in the footer brings the last removal back for this run.
- Store: `%LOCALAPPDATA%\Plith\shelf.txt`, one rooted path per line, newest first, the WPF
  format, so an existing WPF shelf carries over. Load drops lines whose path no longer exists.
  Atomic save like `plays.txt`.
Non-goals
- Browser images, text, URLs, Outlook attachments, files inside zips (no path: needs a payload
  folder; a later phase if Berke wants it).
- Thumbnails (the mock has none), multi-select, a hint for elevated sources (documented only).

## 3. Shape
- `plith-core`: shelf store logic (add, front on re-drop, cap, remove, undo, load filter,
  format), type tag from the extension, wagon layout for 1 to 8 items, hit targets, the drag
  hover state machine (dwell 150 ms, grace 380 ms, own-drag flag), all unit tested.
- `plith-shell`: `IDropTarget` registered on the island window on the UI thread (OLE is already
  initialised there), emitting shell events with island DIP points; `DoDragDrop` from a wagon
  press after the drag slop, with `ReleaseCapture` first, a live-button check and a re-entrancy
  flag; every COM body inside `catch_unwind`. Mouse state reset on `DragLeave` and `Drop`.
- `plith`: the shelf file, wiring events to the core state, the page facts.
- Log: one `drag` line per drag (direction, formats, effect, hover to open ms), like `motion`.

## 4. Proof
- Unit tests for every pure rule; five checks green; offscreen renders of 0, 1, 4, 5 and 8 items
  and the hot slot.
- On screen (console, Berke present): drag in from Explorer, hover open feel, drop, drag out to
  Explorer and a browser, remove and undo, restart keeps the shelf, motion lines 0 to 1 hitches
  at 240 Hz, idle check still 0 wakes at rest. RDP can show drag in and out but not motion.
