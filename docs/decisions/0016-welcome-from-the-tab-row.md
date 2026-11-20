# 0016 The welcome view is reachable from the tab row

Date: 2026-11-19. Status: accepted, amended by 0017 for the tab row (the gear mark at the right end
and the clock 24 DIP left). Amends 0014 (point 1 and the "fifth page without a tab"
consequence). Decided in the console fix round design
(`specs/2026-11-19-console-fix-round-design.md`, section 1).

## Context
ADR 0014 made the welcome view a fifth page without a tab: a click on the collapsed island opens
it, but once the user turned to a tab page there was no way back to it short of collapsing and
opening again. The console session of 2026-11-18 hit this at once.

## Decision
1. **The tab row carries a home mark** at the left end, a small house glyph (16 x 16 DIP, strokes
   in the station weight) centred on x 30 and on the band between the labels and the line. A click
   on it turns to `PageId::Welcome` through the same `Action::Tab` path as any station.
2. **The four stations move right by 24 DIP** (`FIRST_X = MARGIN_LEFT + 24`): centres 54, 166,
   278, 390 and the track from 54 to 390. The mark sits left of the track, not on it.
3. **The home target is pushed first.** `pages::hit` returns the first target under a point and
   grows each rect by 4 DIP; the home rect (x 18 to 43, y 12 to 40) therefore wins its own area and
   the first station keeps everything from its label on.
4. **The mark follows the active page.** `tabbar(Some(PageId::Welcome), ..)` draws it in full ink
   with every station hollow; any other page draws it faint. The welcome page passes
   `Some(PageId::Welcome)`. `PageId::TABS` stays at four: the mark is not a station.

## Consequences
- Every page now has five tab-row targets (home plus four tabs); tests that pinned the old
  station x positions or the target count moved with them.
- The welcome view keeps no filled station, as in 0014, but it now has one active mark.
- Unverified until the console session: the click on the mark with a real pointer, and the glyph
  weight at 100 percent and 200 percent scale.
