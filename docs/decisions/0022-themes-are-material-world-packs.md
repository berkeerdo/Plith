# 0022 Themes are material worlds in data packs

Date: 2026-11-26. Status: accepted (Berke approved
`specs/2026-11-26-theme-worlds-design.md` the same day). Supersedes 0007.

**Context:** ADR 0007 made themes fixed palettes over one language with no colour picker. Berke,
2026-11-26: "temalarımız kötü"; he liked comps r2, where each theme is a material (Halka,
Firn, Kablo, Şerit) with its own ink and opening motion. He also wants to fit Plith to his
desk: "vurgu rengini seçebileyim", and the background in some themes.

**Decision:**
- A theme is a **pack**: a `theme.ini` manifest plus rasters, read by one `plith-core` loader.
  Built-in packs are embedded; a user folder can later use the same format.
- Built-ins: Halka, Firn, Kablo, Şerit, Gece. York, Sodyum and Kağıt retire; their colours
  survive as Gece settings.
- A pack may let the user pick the accent; only plain (material-free) packs let the user pick
  the floor. Recolouring a material is reserved (`material.tintable`), not built.
- The contrast bar stays and tightens: ink 7, muted 4.5, faint and flow 3, per pixel in the
  pack's text zone. User colours are solved to pass (hue kept, lightness moved).
- Shape, type, spacing and the flow rule ("flow means sound flows here now") stay common;
  material, ink, mark style and opening motion belong to the pack.

**Consequences:**
- The `Theme` enum becomes a pack registry; settings gain `accent.<id>` and `floor.<id>`.
- Görünüm returns as a card picker (overrides the old no-cards anti-goal).
- The Halka island plan's own ADR 0022 is folded into this one.
- High contrast and the OSD helper still ignore themes.
