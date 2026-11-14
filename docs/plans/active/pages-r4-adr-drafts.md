# R4 ADR drafts for the master to file

Lane `pages`, 2026-11-15.

## P5 UI layer shape: a display list built on the UI thread, drawn retained

Status: draft. Builder call from spec section 11 ("the UI layer's shape, retained or
immediate"), deferred from R1 (`plans/done/engine-r1-adr-drafts.md` P5).

**Decision:** pages are pure functions in `plith-core` from view state to a **display list**
(filled rects, strokes, station squares, text runs, images) in island DIP, together with the hit
targets of the same layout. The UI thread builds the lists and publishes them, with the scene,
through the latest-wins mailbox (ADR 0006), which skips a publish that changes nothing. The render
thread draws each page's list once into a cached bitmap and rebuilds it only when that list
changes; every frame composites the cached pages with their slide offset and group opacity inside
the island clip. Values that change at frame rate (meters, the 8 s traces, the seek fill) are
**live nodes**: the list only places them, and the render thread draws them each frame from
sampled values on an ambient tick at `hz / 4`, only while the island is open on a page that has
them.

**Compared:**

| | Retained widget tree | Immediate mode on the render thread | Display list (chosen) |
|---|---|---|---|
| Where layout runs | widget objects on the render thread | render thread, every frame | UI thread, pure `plith-core` |
| Hit testing | tree must be reachable from the UI thread | needs a round trip or a layout copy | the same layout, a local call |
| Text cost | cached per widget | re-laid per frame unless cached by hand | rasterized once per content change |
| Testable without Windows | no | no | layout and lists are unit tested |
| Cost per frame | tree walk | full draw | one blit per visible page + live nodes |
| Main risk | invalidation and event code | text per frame breaks draw under 1 ms | values at frame rate need their own path |

**Consequences:**
- `plith-core` owns the page model, layout, hit targets and the theme palettes; the render thread
  draws lists and never decides anything about a page.
- Spec section 6 "content that moves during a morph is drawn into an offscreen layer and
  composited with group opacity, so text is rasterized once" is the default path, not a special
  case.
- A content change at rest costs one frame (rebuild one page bitmap, present once). An unchanged
  publish costs nothing (review L12).
- The ambient tick writes its own `ambient` log line (frames, hitches, worst gap, draw p99); the
  `motion` line and `MotionKind` are unchanged.
- To measure in R4: draw p99 under 1 ms per frame with two page bitmaps and live nodes, and
  240 fps, 0 to 1 hitches per motion.
