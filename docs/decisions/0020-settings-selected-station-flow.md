# 0020 The settings window's selected section is marked in the theme's flow colour

Date: 2026-11-25. Status: accepted (Berke approved the identity round 3 spec, "önerinle devam
edelim lütfen"). Widens the "lime only where sound flows now" rule of the settings window
(`specs/2026-11-20-r7-settings-window-design.md`, anti-goals) a second time after ADR 0019.

## Context
Identity round 3 (`specs/2026-11-24-identity-r3-design.md`) derives the settings window from the
app icon. The picked variant S1 keeps the window's Ray Paneli language and adds brand marks only:
the icon at the top of Hakkında, the icon on the window class, and one colour spot that ties the
window to the icon.

## Decision
- The filled station beside the selected section in the sidebar is drawn in `Ink::Flow`: lime in
  Gece, dark green in Kağıt, lamp white in York, amber in Sodyum. Its place and size do not change.
- It is the only flow-coloured node of the sheet besides the icon raster. The test
  `flow_only_on_the_selected_station` holds that line.
- On the island the flow colour keeps its meaning (sound flows here now).

## Consequences
- In the window the flow colour now also means "you are here". The two never meet: the island
  and the window are separate surfaces.
- In York the lamp white station sits close to the ink text; it stays distinct by hue and shape
  (measured on `design/identity/r3/renders/york`).
