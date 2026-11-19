# Console fix round: design

Date 2026-11-19. Approved by Berke ("önerinle devam", twice). Short design for a small change.

## Intent
Berke ran the Rust notch in the console on 2026-11-18 and found five things that read wrong on
screen. This round fixes them without spending the idle wake budget or the 240 fps bar.

## Goals
1. A way back to the welcome view after a tab turn.
2. The Karot pill's right-hand `%62` reads as volume.
3. A long playing title in the idle pill can be read.
4. The audio volume ruler, the brightness ruler and the media timeline follow a drag.
5. The first OSD show of a run does not hitch.

## Non-goals and anti-goals
- No looping marquee: a title that keeps moving costs wakes forever (idle budget, `idle.rs`).
- No new keyboard path (ADR 0011).
- No seek per pointer move: it floods the media session.
- No change to the WPF app.

## Approaches
- Item 3, title scroll. Render side (picked): a one-shot live scroll node with a fade edge; the
  render thread ticks it and parks again when the offset rests, so 240 fps while moving and 0
  wakes after. UI side (rejected): a timer republishes the pill list each step, which re-rasters
  the cached pill bitmap and wakes the render thread per step: stutter risk.
- Item 1, room for the home mark. Shift the four stations 24 DIP right (picked) or squeeze a
  glyph into x 6 to 22 (rejected: it sits on the island's rounded corner).
- Items 2, 4 and 5 have one sensible way each.

## Design
1. **Home mark.** A 16 DIP home glyph at the left of the tab row; the stations move 24 DIP right
   (30/142/254/366 become 54/166/278/390). Its hit target comes first and carries
   `Action::Tab(PageId::Welcome)`; `turn_to` already accepts any page. Shown on every tab page,
   marked active on welcome. A new ADR amends ADR 0014 (welcome reachable from the tab row).
2. **Speaker glyph.** `glyphs::glyph(DeviceKind::Speaker, ..)` before the percent, only on the
   `sound` branch of the Karot right slot. The fit width in `sound_words` shrinks by the glyph
   plus gap (about 20 DIP). Muted state keeps its own word.
3. **Title scroll.** New draw node: text clipped to a rect with a fade at the trailing edge, and
   a live one-shot horizontal offset. It runs once on a track change and once on hover (pointer
   `Move` over the pill), then rests at the start. Only a title wider than its slot moves. The
   collapsed pill joins `wants_ambient` while the scroll runs; the scroll's end condition parks
   the thread. The idle check must still meet `allowed()`.
4. **Drag.** The shell adds `Press`, `Drag` (move while captured, point not clamped to the
   shape) and `Release`; `Click` stays for presses that did not move. The app holds the dragged
   ruler, recomputes the value per move with the existing pure math (`brightness_at`,
   `seek_fraction`) and shows it at once as an override in the feed until the service reads it
   back. Volume and brightness write during the drag, at most 30 a second, plus a final write on
   release. The timeline seeks once, on release.
5. **OSD prewarm.** After the stack is built, and after every rebuild (relayout, device loss),
   the helper lays out the label, a `%100` value and the muted word and draws one full frame
   while the window is hidden; it logs a `prewarm:` line like the pages do.

## Constraints
- File limits (500 source, 800 test, 80 per function). `plith-shell/src/window.rs` is at 443:
  the new pointer events go in `window/mouse.rs`. `plith-core/src/pages/media/tests.rs` is at
  476: new media tests go in a new test file.
- Pure logic in `plith-core` with unit tests; the five checks green before merge.
- A protocol change is not needed (the OSD prewarm is helper-internal), but the helper is
  rebuilt: reinstalling it under Program Files needs Berke's yes.

## Measured on screen (console, Berke present)
Each item gets a console step in the plan: home mark round trip, glyph look, a long title scroll
with an idle check after it, a drag on all three rulers, the first OSD show's `motion` line.

## Left to the builder
Exact glyph drawings, fade length, scroll speed and pause (motion tune `brisk`), the throttle
mechanism, where the drag override lives in the feed.
