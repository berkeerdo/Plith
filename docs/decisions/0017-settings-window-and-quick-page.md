# 0017 Settings live in two layers: a quick page in the island and a full window

Date: 2026-11-20. Status: accepted. Amends 0016 for the tab row (the gear mark and the clock).
Decided in the R7 settings window design (`specs/2026-11-20-r7-settings-window-design.md`,
sections 5 and 6).

## Context
Every setting so far lived in a text file. The look of the resting island, the language and the
welcome view's options need a place a person can reach without leaving the notch, and the full
set needs room that a 620 DIP island page cannot give.

## Decision
1. **Two layers.** A quick page inside the island holds the one setting people change by eye: the
   six resting looks as small true previews, and a row that opens the full settings window.
   Everything else lives in the window.
2. **The gear mark mirrors the home mark.** A gear glyph (16 x 16 DIP, station stroke weight) sits
   at the right end of the tab row, centred on x 590 and on the home mark's y; its hit area is
   the home area's size. The clock moves left by 24 DIP (right edge 572). The gear is not a
   station. A click on it turns to `PageId::Quick` through `Action::Tab`; on the quick page it
   draws in full ink and a click turns back to the welcome view, like the home mark.
3. **`PageId::Quick` is a page without a tab station.** `PageId::TABS` stays four, `PAGE_COUNT`
   becomes six and the quick page has index 5. The page cache slot of the pill is now derived
   from `PAGE_COUNT`. `open_on = LastPage` never records the quick page: an opening after the
   island collapsed on it lands on the welcome view.
4. **The window has its own render thread** that lives only while the window is open, so a
   closed window costs nothing. The island and the window do not share a render context.
5. **Every change applies live and saves at once.** There is no apply or cancel button: a look
   click on the quick page, like a row in the window, changes the running app and writes the
   settings file in the same step.

## Consequences
- Every page has six tab-row targets (home, four tabs and the gear); tests that pinned the
  clock position or the target count moved with them.
- Whenever the pill is rebuilt the quick page is too, because it shows the picked look and live
  previews of every look. While the island is closed the quick page waits with the other pages.
- The quick page's previews are the pill's own lists at scale 0.4, built from the same idle
  facts, so a preview cannot drift from the real pill.
- The look click and the settings row reach the same change path as the window: the setting is
  applied at once while running and saved.
- The UI thread measures text with its own DirectWrite factory (`TextMeasure`), only for the
  city field's caret; it never draws, the sheet's own render thread does.
- Unverified until the console session: the gear glyph weight at 100 and 200 percent scale, and
  the click with a real pointer.
