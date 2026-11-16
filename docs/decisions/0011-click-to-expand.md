# 0011 Input on the island: click to expand, leave to collapse, no keyboard

Date: 2026-11-16. Status: accepted in code; mouse leave and click feel are measured by the R4
console run. Drafted by lane `pages` (P6), filed by the master at the R4 merge (`b6bd801`).
Decided by Berke on 2026-11-16 from R4 plan questions 10 to 12.

**Decision:** the island never takes activation and has no keyboard. A click on the pill
expands; leaving the island for 400 ms collapses; a click on empty page space does nothing;
there is no hover-to-expand. Pick mode on the audio page cancels by clicking the picked source
again or empty diagram space (the mock's `ESC VAZGEÇER` became `KAYNAĞA TEKRAR TIKLA: VAZGEÇ`).
Values change by a click on their ruler only: no drag, no wheel.

**How:**
- A click needs both halves on the island: `WM_LBUTTONDOWN` inside the drawn shape records the
  press and takes the capture; `WM_LBUTTONUP` inside the shape emits `Click { x, y }` (island
  DIP, release point); a lost capture or a leave forgets the press. A drag that starts in another
  window and ends on the island is not a click (merge review I1: such a release could otherwise
  write hardware). `WM_MOUSEACTIVATE` answers `MA_NOACTIVATE`.
- `Enter`/`Leave` come from `TrackMouseEvent(TME_LEAVE)`; the window region (ADR 0004) makes
  leaving the region equal to leaving the window.
- Core `route_click` picks the page on screen (the incoming layer once its opacity reaches 0.5)
  and runs `pages::hit`, which resolves ruler targets into values. A due collapse also applies
  while the island is suppressed, so it never reappears open.
- Hardware writes run on one `plith-writes` thread through the `Services` trait, never on the UI
  thread; fixture, demo and idle-check runs never write.

**Not measured:** real `WM_MOUSELEAVE` delivery with a per-frame region and the feel of a click
during a page turn (R4 console run).
