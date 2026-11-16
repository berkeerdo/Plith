# R3 visual direction: Ray Paneli

Date: 2026-11-14. Status: picked by Berke 2026-11-14 ("önerin ne ise onu uygula"), before any
render. First render: `docs/design/r3/renders-6/sheet.png` (source `docs/design/r3/ray-paneli.html`,
HTML mock at 1:1 DIP, not the engine). Process: skill `praxvon-direction`, logged in its taste log.

## Concept
Source: the British Rail NX (entrance-exit) panel at York, 1951: a track diagram where pressing an
entrance and then an exit sets a route, and the set route lights up.

- **Metaphor:** the audio page is a track diagram. Sources (microphone, line in, Windows, Discord,
  Voicemeeter strips) on the left, outputs (A1..A3, B1..B2, devices) on the right.
- **Rule: the visible route is the real route.** Lime means one thing: sound flows here now.
  Selection, the active tab and brightness use ink, never lime.
- **Signature interaction:** tap a source, then an output: the route is set (Voicemeeter bus
  assignment or endpoint pinning) and the light walks from entrance to exit.
- Mixed in from the other two concepts:
  - From Sessiz Seyir (LOFARgram): each source carries its last 8 s as a quiet trace; a clip stays
    as a lime notch for 8 s. In an exclusive fullscreen game the island hides, only the OSD shows.
  - From Tuğra: the pill and the game OSD are one seal with three fixed slots (output, route,
    value). Nothing appears or reflows, slots only fill.

Rejected as core ideas (the attractor): blob morph, the labelled active-tab pill (Oyenza), album
art colour bleed, bouncing EQ bars, a giant percentage, fader strips, RGB, scanlines, glass.

## Language (Loft, praxvon-ui ADR 0014)
- Floor `#0B0B0A` to `#151412`, border `#2B2824`, ink `#ECE9E2`, muted `#A39E94`, faint
  `#8A847A`, track `#4A463F`, lime `#CAFF33` (dark only for now).
- Public Sans for names and titles, Martian Mono (tabular) for labels and every changing number.
  Uppercase mono labels at 9 to 10 px, ruler ticks under every scale. No `+` corner marks
  (Berke, 2026-11-14: they do not look good on the island).
- Themes (ADR 0007): colours only, rendered in `docs/design/r3/renders-6/themes.png`: Gece,
  Kağıt, York, Sodyum. The palette above is Gece.
- Lines: 1 px track, 2 px lit route. Every line ends on an 8 DIP station square (a 7 DIP rect with a 1 DIP stroke, as the mock
  draws it): filled lime = live,
  filled ink = selected or a muted source, hollow = reachable or idle. Routes run horizontal and
  turn at 45 degrees, like a signal box diagram.
- Shape: square top (flush with the screen edge), rounded bottom. Mock uses radius 24 for pages and
  16 (full) for the pill; the engine slice still uses 40 (open item).

## Surfaces
| Surface | Size (DIP) | Content |
|---|---|---|
| Pill | 200 x 32 | Seal: output short name, route segment (lit, cut when muted, hollow when idle), value |
| Tab bar | in every page, 620 wide | One hairline track with four stations SES, MEDYA, RAF, SİSTEM; active = filled ink square + ink label; clock at the right end. Four tabs, never more |
| Audio | 620 x 300 | Track diagram; muted source = cut line + `SESSİZ`; pick mode: `[ Mikrofon ]` in lime brackets, candidate outputs as dashed lime routes |
| Media | 620 x 232 | Art, title, artist, seek on a ruler (played part lit), transport; output picker is a route `SPOTIFY > A1 · G733` with reachable outputs as hollow stations |
| Shelf | 620 x 214 | A siding: files are wagons (type tag, full two-line name), a dashed drop slot, wagons sit on stations on one track |
| System | 620 x 178 | Brightness on a ruler in ink; microphone as a route to its bus, cut with `KAPALI` when muted |
| Game OSD | 300 x 40 | The same seal; the route slot becomes the volume ruler with a lit fill |

Motion: values settle without overshoot (Loft `spring.smooth`); a route lights from entrance to
exit in about 180 ms; reduced motion jumps. Nothing draws at rest (G2): the 8 s traces and meters
run only while the island is open.

## Quality pass (impeccable, 2026-11-14)
- Contrast measured per theme against both floor stops: the faint tone failed 4.5:1 in all four
  (3.4 to 4.1). Raised to Gece `#8A847A`, Kağıt `#6B665E`, York `#86958E`, Sodyum `#86868D`
  (4.6 to 5.2). Track lines stay at about 1.9:1: they are rails, every state they carry also has
  a label or a station mark that passes.
- Smallest text is 10 px (labels were 9). Desktop copy: "tıkla", not "dokun". The tab line runs
  only between the four stations. Output values share the name's row.
- Empty and degraded states drawn: no Voicemeeter (Windows endpoints only, a click sets the
  default), nothing playing, empty shelf, a monitor without DDC/CI brightness.
- Not covered by a mock: keyboard focus (the island takes no activation; decide in R4), long
  device names (truncate in the middle), RTL is out of scope.

## Open items (decide in R4 or with Berke)
1. Bottom radius: 24 (mock) or the engine's 40.
2. Density: Voicemeeter Potato has 8 strips and 8 buses; draw only lit routes plus the picked
   source's candidates, or the diagram becomes a net. Needs a render with Potato data.
3. Pill at 200 wide is tight when the value is `KAPALI`; a mute glyph may replace the word.
4. Meter data per strip and the 8 s trace need the Voicemeeter levels API from the audio service.
5. Setting a route writes Voicemeeter parameters: hardware writes need Berke's yes when tested.
6. High contrast is not drawn (it keeps the Windows scheme, as in WPF).
7. Next: R4 builds it on the engine; the mock stays the visual reference.
