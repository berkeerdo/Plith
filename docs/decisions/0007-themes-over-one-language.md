# 0007 Themes are palettes over one visual language

Date: 2026-11-14. Status: accepted (Berke, R3 review of the first Ray Paneli render).

**Context:** the rewrite design dropped the accent picker and VISION said "not a theme engine,
one visual language". Seeing the first R3 render, Berke asked for several themes: "birden fazla
tema sunmak lazım bence, renkleri falan onlar belirleyecek".

**Decision:** Plith offers a small fixed set of themes. A theme sets colours only: floor (two
stops), border, ink, muted, faint, track, faint track, and the one **flow** colour. Shape, type,
spacing, station marks, ruler ticks and the Ray Paneli rule ("the flow colour means sound flows
here now, nothing else") are the same in every theme. No free colour picker.

**Candidates rendered** (`docs/design/r3/renders-6/themes.png`): Gece (Loft dark, lime
`#CAFF33`), Kağıt (Loft light, dark green `#3F6E00`), York (1951 NX panel green-grey, lamp white
`#FFE9A8`), Sodyum (black, amber `#FFB224`). Berke picked all four (2026-11-14). Default: Gece,
the Loft palette the direction was built on (not stated by Berke; change it if he says so).

**Consequences:**
- Supersedes the "not a theme engine" line in VISION (now: few themes, one language) and the
  single-accent note in the rewrite design, section 3.
- Every theme must pass the contrast bar: text 4.5:1, flow colour and marks 3:1 on its floor.
  A test in `plith-core` checks each palette, like the WPF accent test did.
- High contrast still bypasses themes and takes the Windows scheme (as in WPF).
- The palette is data in `plith-core`; the render thread swaps it without a new swap chain.
