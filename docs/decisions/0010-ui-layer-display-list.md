# 0010 UI layer: a display list built on the UI thread, drawn from cached page bitmaps

Date: 2026-11-16. Status: accepted in code; draw p99 and motion over page bitmaps are measured by
the R4 console run. Drafted by lane `pages` (P5), filed by the master at the R4 merge (`b6bd801`).
Answers spec section 11 ("the UI layer's shape, retained or immediate"), deferred from R1.

**Decision:** pages are pure functions in `plith-core` from view state to a **display list**
(filled rects, strokes, station squares, text runs, images) in island DIP, together with the hit
targets of the same layout. The UI thread builds the lists and publishes them with the scene
through the latest-wins mailbox (ADR 0006), which skips a publish that changes nothing. The
render thread draws each page's list once into a cached bitmap and rebuilds it only when that
list changes; every frame composites the cached pages with their slide offset and group opacity
inside the island clip.

Values that change at frame rate (meters, the 8 s traces, the seek fill, the elapsed time) are
**live nodes**: the list only places them, and the render thread draws them from sampled values
on an ambient tick that paints every `round(hz / 60)`-th vblank (about 60 fps), only while the
island is open on a page that has them.

Changed page bitmaps are re-rastered outside motion frames (**prewarm**): at rest while open,
and on a motion that opens or turns the island only for the pages it shows, before its first
clock wait. Each prewarm writes a `prewarm:` log line with its cost. A collapsed island rasters
nothing.

**Compared:**

| | Retained widget tree | Immediate mode on the render thread | Display list (chosen) |
|---|---|---|---|
| Where layout runs | widget objects on the render thread | render thread, every frame | UI thread, pure `plith-core` |
| Hit testing | tree must be reachable from the UI thread | round trip or a layout copy | the same layout, a local call |
| Text cost | cached per widget | re-laid per frame unless cached by hand | rasterized once per content change |
| Testable without Windows | no | no | layout and lists are unit tested |
| Cost per frame | tree walk | full draw | one blit per visible page + live nodes |
| Main risk | invalidation and event code | text per frame breaks draw under 1 ms | values at frame rate need their own path |

**Consequences:**
- `plith-core` owns the page model, layout, hit targets and theme palettes; the render thread
  draws lists and never decides anything about a page.
- Spec section 6 (content that moves during a morph is an offscreen layer with group opacity) is
  the default path, not a special case.
- A content change at rest costs one raster and one present; an unchanged publish costs nothing.
- The ambient tick writes its own `ambient` log line; the `motion` line and `MotionKind` are
  unchanged.
- To measure: draw p99 under 1 ms with page bitmaps and live nodes; 240 fps, 0 to 1 hitches per
  motion; the prewarm cost between click and first frame.
