# Theme switch in settings

Date: 2026-11-24. Status: design, my call under Berke's "her işi yapsana" (2026-11-24); he can
veto any point. Builds on ADR 0007 (four fixed themes) and the settings window spec
(`specs/2026-11-20-r7-settings-window-design.md`, section 2 named Görünüm as the section that
joins "when `set_palette` is called").

## 1. Problem
`plith-core::theme` holds four palettes (Gece, Kağıt, York, Sodyum) that pass the contrast test,
and the render thread has `set_palette`, but nothing calls it: every run is Gece.

## 2. Goals and non-goals
Goals
- A fifth settings section **Görünüm** between Genel and Boşta, one row **Tema**, a choice group
  `GECE / KAĞIT / YORK / SODYUM` (the product names, upper case with Turkish rules: `KAĞIT`).
- New key `theme=gece|kagit|york|sodyum` in `settings.ini`, default `gece`, unknown value falls
  back to `gece` and is logged like other unknown values.
- Live apply: the island (pill, pages, cached page bitmaps rebuilt as on a language switch) and
  the settings window itself redraw in the new palette at once; saved at once (atomic save).
- `--theme <key>` on the command line, like `--idle-look`, wins over the file (demo and renders).
Non-goals
- The OSD helper keeps its fixed colours: theming it needs a protocol bump and a reinstall under
  sudo (Berke's yes). Recorded as an open item.
- High contrast (ADR 0007 says it bypasses themes; settings spec puts it out of scope too).
- Previews or theme cards (banned attractor in the settings spec, section 2).

## 3. Shape
- Pure logic in `plith-core`: `Theme` key parse and format, the settings field, the section and
  row in the sheet model, hit test and focus order like the other choice groups.
- `plith`: apply mapping sends the palette to the render thread for the island and the window.
- Strings: section name and row label in all eight languages (ADR 0015). Theme names are proper
  names and stay the same in every language.

## 4. Proof
- Unit tests: key round trip, unknown key fallback, the section order, the row's hit test, a
  theme change asks for a page bitmap rebuild.
- Offscreen renders of the window and a page in each theme (existing render examples).
- On screen (console, Berke present): pick each theme, the island and the window follow, 0
  render wakes at rest afterwards. RDP shows colours too, but no motion numbers from RDP.
