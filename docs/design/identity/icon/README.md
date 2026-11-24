# Plith app icon (identity round 2)

Date: 2026-11-24. Decision: ADR 0019. Master: `plith-master-1024.png` (1024 px, transparent).
Build the `.ico`: `pwsh -File scripts/build-icon.ps1` (writes `crates/plith/assets/plith.ico`,
sizes 16, 20, 24, 32, 40, 48, 64, 256).

## How it was found
| Step | What | Result |
|---|---|---|
| Board | 24 real icons: notch apps, Win11 Fluent, sound and gaming, macOS craft | notch apps all use a black pill, purple glass or a mascot |
| Round A to C | 12 drafts: lime route, P monogram, one dark mark on lime | P rejected ("p olayı kötü"), lime tile picked |
| Lime tile | 23 marks on lime | "sound spreading from the top" (L03) |
| Composition | 8 recompositions of L03 | R6, tactile with carved ripples |
| Material | 8 tactile materials, then 4 fabric + ripple blends | M3 picked; tray check at true 16 and 24 px |
| Vector | M3 redrawn in SVG, then a Loft flat version | rejected: three black arcs on green read as Spotify, coded fabric looked like graph paper |
| Final | 4 high-quality drafts with the LA3 composition, transparent | F1 |

## Lessons
- Show every visual on a page Berke can open (he works over RDP from a Mac); never ask for a
  pick from a local path.
- Real materials come from the image model; code draws flat, exact shapes well and textures
  badly.
- Black stripes on a green field read as Spotify whatever their direction.
- Check every candidate at true tray size (16 px) on a dark and a light taskbar before a pick.

The drafts live in the session scratchpad and on Berke's private review pages; only the master
is kept here.
