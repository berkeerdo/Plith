# 0004 Hit testing

Date: 2026-11-14. Status: accepted. Drafted by lane `engine` from the R1 console session
(release build, RTX 5080, 240 Hz, `%LOCALAPPDATA%\Plith\plith.log`); numbers copied from the log.

**Decision:** one DirectComposition window (`WS_EX_NOREDIRECTIONBITMAP`) with a `SetWindowRgn`
round-rect region **3 px outside** the drawn shape, applied on the UI thread from a shape the
render thread posts (coalesced) every painted frame. Oyenza: two windows and a 30 Hz mask.

**Measured:**
- Region at 1 px outset (21:05): clicks inside toggle, clicks outside reach the window below,
  but Berke: "köşeler pürüzsüz değil" (corners not smooth); the GDI region clips the
  anti-aliased edge.
- Region at 3 px outset (21:14): Berke: "evet koşler ürüzssüz evet sıkıntı yok açılıp
  kapanıyor her soru evet çalışıyor" (corners smooth, outside clicks pass, toggle works).
- UI-thread cost per region update, 14 runs from 20:23 to 21:30: mean 0.43 to 0.78 ms per run,
  per-run max 0.5 to 6.8 ms, except 33.2 ms once in the lock run (most likely the session
  switch around lock or unlock; the render thread is separate and that run had 0 hitches);
  0 errors in 30,229 updates.
- Split (separate input window, layered click-through visual): Berke saw no island in its one
  run; the run may have closed before he looked, so it is unverified and dropped. Mask uses the
  same layered visual window and was not run.
- Trade: clicks within 3 px outside the edge count as the island. Pen and touch: not tested
  (no pen or touch screen used).
