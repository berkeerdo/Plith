# 0018 A file drag over the island opens the shelf

Date: 2026-11-24. Status: proposed (my call under Berke's "önerin ne ise devam edelim",
2026-11-24); the dwell and grace are measured in the R5 console run and may change.
Amends ADR 0011 ("there is no hover-to-expand").

**Context:** during a foreign drag OLE owns the mouse, so the island gets no `WM_MOUSEMOVE`, no
click and no `WM_MOUSELEAVE`. The only signal is `IDropTarget` on the island window. Without an
exception to ADR 0011 a file could never reach the shelf, because the pill would never open
under a held file. Oyenza opens its shelf the same way (380 ms grace,
`research/2026-11-14-oyenza-engine.md:158`).

**Decision:** a drag whose data object offers `CF_HDROP` and stays over the island region for
**150 ms** opens the island on the shelf page. While it hovers the island stays open and does
not turn pages. `DragLeave` starts a **380 ms** grace, then the normal collapse; a drop starts
the normal 400 ms leave collapse once the pointer is out. Any other drag (text, URL, elevated
source the UIPI blocks) gets `DROPEFFECT_NONE` and opens nothing. Mouse hover without a drag
still does nothing.

**Consequences:**
- The shell's mouse state (tracking, press, capture) is reset on `DragLeave` and `Drop`, since
  OLE may have swallowed the leave and the release.
- Plith's own drag out of the shelf is not re-added when it passes over the island.
- Elevated sources cannot drop (UIPI): documented, no hint in R5.
