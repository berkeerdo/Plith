# R7 idle look direction: Karot

Date: 2026-11-17. Status: picked by Berke 2026-11-17 ("önerinle devam edelim"): A Karot with C's
reading time on the weather. First render: `design/r7/renders-1/sheet.png` (source
`design/r7/karot.html`, HTML mock at 1:1 DIP, not the engine). Process: skill `praxvon-direction`.
Inputs: Oyenza settings research (`research/2026-11-17-oyenza-settings-and-hud.md`), Berke's
notes in STATUS ("bizimki daha iyi olmalı"; the WPF welcome view with clock, animated weather and
the playing song in small type, editable in settings), R3 direction Ray Paneli (language binding).

## 1. Product truth
- Two surfaces: the **resting pill** (on screen all day, glanced at from the corner of the eye,
  often during a game) and the **welcome view** (what a deliberate hover opens before any tab:
  clock, weather, the song).
- The claim: the notch is worth its place even when nothing happens, and it costs nothing at
  rest (G2: the idle check measures 0 wakes; a clock adds exactly one redraw a minute).
- Who decides: a player who keeps the notch only if it never steals attention and shows
  something Windows' own clock does not.

## 2. The attractor (banned as core ideas)
- A centered clock in a capsule; Oyenza's set Clock / Eyes / Minimal / Hidden.
- A mascot or eyes that follow the cursor.
- Weather as an emoji or icon plus a temperature; animated sun, drifting cloud, falling drops
  (the WPF `WeatherMark` did this).
- Album art thumbnail with a scrolling marquee title; EQ bars or a waveform while playing.
- A carousel that rotates clock, weather and song on a timer.
- Settings as an iOS-style widget gallery with drag and drop, theme cards with live preview.
- From fingerprints: count-up numbers, text scramble, scanlines, dot matrix.
- Language that stays (not attractor, already decided): Loft palette, Martian Mono numbers,
  rulers with ticks, stations, lime means only "sound flows now".

## 3. Ten directions (drawn: glaciology, sports field markings, apothecary jars, core samples)
| # | Domain | Reference | Idea | p |
|---|---|---|---|---|
| 1 | Core samples | IODP core section photo layout: 1.5 m sections side by side, depth blocks | The day is one core run; left of now is logged fact, right of now is forecast | 0.04 |
| 2 | Core samples | Lithologic strip log, column width = grain size | Weather as a strip whose width tracks rain | 0.06 |
| 3 | Sports markings | FINA backstroke flags, 5 m before the wall | The pill speaks only before something changes | 0.05 |
| 4 | Sports markings | Relay exchange zone (20 m, triangles) | One item at a time, handover only on events | 0.15, dropped (close to carousel) |
| 5 | Glaciology | Storglaciären mass-balance stakes, read on schedule since 1946 | Values change only at reading times and carry their reading time | 0.06 |
| 6 | Glaciology | Firn line seen from the valley | A line in the pill marks daylight left | 0.12, dropped |
| 7 | Apothecary | Albarello cartouche labels in fixed shelf niches | Fixed niches, editable shelf | 0.18, dropped (widget gallery) |
| 8 | Apothecary | Jar shape by contents (albarello, chevrette, carboy) | Pill silhouette tells what it carries | 0.20, dropped (blob morph) |
| 9 | Free | Train describer berths in a signal box | Items step through berths now, next, later | 0.08, overlaps 3 |
| 10 | Free | Split-flap departure board | Flipping letters | 0.35, dropped |

Survivors: 1, 3, 5 (all three from drawn domains).

## 4. Concepts

### A. Karot (recommended)
| Field | Content |
|---|---|
| Source | IODP core section photographs: 1.5 m sections laid side by side with depth blocks between runs, the bit at the bottom of the hole |
| Metaphor | The welcome view is today's core run; the drill bit is now |
| Rule | **Left of now is what happened, drawn solid; right of now is forecast, drawn dashed.** Lime only on the interval playing now |
| First screen | Hover: a 620 DIP run from 6 hours back to 6 hours ahead, hour ticks as depth blocks, the bit (a station square) at now with the time beside it in large mono. Above the run, past songs as ink segments (the one playing now lit lime, title and artist in small type), below it the weather as lithology hatches: blank clear, horizontal dashes overcast, dots rain, crosses snow; dashed past the bit. Temperature at now and the next turn ("16:00 YAĞMUR · 9°") |
| Signature | Point at any hour on the run: the past shows what played then, the future shows the forecast for then. "My notch shows my day: what I heard, and when the rain comes" |
| Resting pill looks | **Karot**: time, a 48 DIP strip of the next 3 hours' hatches, temperature. **Saat**: time only. **Çalan**: the playing song's title in small type, the time when nothing plays. **Sade**: an empty pill. **Gizli**: an edge strip only. The seal (output, route, value) still owns the pill whenever the OSD or a change fills it |
| Type and color | Martian Mono for time and every number, Public Sans for titles; ink for the past, faint dashed for the forecast, lime for the playing interval only. Hatches are 1 px lines snapped to pixels, like a strip log |
| Motion | None at rest. The bit advances one step a minute (the one allowed redraw). On hover the run draws from the bit outward in 180 ms; reduced motion jumps |
| Settings | Full window, page "Boşta": the pill looks as true-size previews of this PC's real data (not stock pictures), the run's span (±3 h, ±6 h, the whole day), which bands show (songs, weather), units, location (Windows location, IP fallback, or a typed city). Quick settings in the island: the look picker only |
| Swap test | No: the past/forecast split on one run, songs as cored intervals and weather as lithology are Plith's alone; Oyenza shows a time and nothing else |
| Build reality | Engine display list, no new technique. New: an in-memory play log for the day (from the media service), a weather service (Open-Meteo hourly, no key, fetched off-thread every 30 min; WPF already has a client and location chain to port). Risk: hatches at 10 DIP read as noise; needs a render at 1x and 1.5x. Coordinates go to an external service: off until the user turns weather on |

### B. Sırtüstü Bayrakları (backstroke flags)
| Field | Content |
|---|---|
| Source | FINA backstroke turn indicators: a line of flags 5 m before the wall so a swimmer who cannot see ahead knows the end is coming |
| Metaphor | The pill is the flag line: it warns before the wall |
| Rule | **The pill says nothing about now, only about what changes next.** At rest it shows the time; a flag appears only ahead of a change |
| First screen | Pill: `14:32`. Ten seconds before a song ends, a row of tick flags and the next title (`SIRADAKİ · Artist`). Forty minutes before rain: `YAĞMUR 40 DK`. Hover welcome view: the next three walls on one ruler (song end, rain, sunset), each with its distance |
| Signature | In a game you see the rain or the end of the album coming without opening anything |
| Settings | Which walls count (song end, rain, sunset, battery), how far ahead each flag rises |
| Swap test | No: forward-only warnings are a different job from a clock |
| Build reality | Same services as A plus a next-track source (not every player exposes it through SMTC). Risk: few walls in a day, so most of the time it is just a clock; flags rising mid-game may read as notifications |

### C. Okuma Saati (stake reading)
| Field | Content |
|---|---|
| Source | Storglaciären (Tarfala) mass-balance stakes, read on a fixed schedule since 1946, every value logged with its reading date |
| Metaphor | The pill is a reading, not a live feed |
| Rule | **Nothing changes between readings, and every value shows when it was read.** Time each minute, weather each half hour (`9° · 14:00`), the song only when it changes |
| First screen | Pill: `14:32 · 9°`. Welcome view: three readings in a row, each with its reading time under it in faint mono, a stale reading turns hollow instead of guessing |
| Signature | The calmest notch on Windows: provably no motion and an honest age on every number |
| Settings | Which readings, reading interval for weather, units, location |
| Swap test | Partly: honesty about age is close to our own Accession Label (Isochron); weakest of the three |
| Build reality | Cheapest; same weather service. Risk: reads as plain text; little to show off against Oyenza |

## 5. Recommendation
A, Karot, with C's reading time on the weather (`· 14:00`) as a detail. It answers all three of
Berke's items in one shape, gives the welcome view something no competitor has, keeps the pill
at one redraw a minute, and reuses the track language (stations, rulers, dashed = not yet).

## First render notes (2026-11-17, `renders-1`)
- Welcome view grew to 620 x 264 so the hover readout fits under the hour labels; without a
  readout that band is empty. Decide: keep 264, or show the readout in the head instead.
- At 1x the cloud dashes and rain dots are told apart, but past (muted) and forecast (faint)
  differ mostly by the dashed outline. The 60 DIP strip in the Karot pill reads as noise at 1x,
  as the risk said: try two plain intervals with a word (`YAĞMUR 15:30`) or drop the strip.
- Not drawn yet: the settings page, light themes (Kağıt, York, Sodyum), snow, 12-hour clock.

## Second render (2026-11-17, `renders-2`, Berke: "önerinle devam")
- Welcome view back to **620 x 232**. Rule: **the head reads whatever the pointer points at; at
  rest it reads now** (weather now, next turn, reading time). A probe on a past hour reads what
  played, on a future hour the forecast.
- Logged weather sections get a filled floor (`#24221E` on Gece); the forecast is open ground with
  a dashed outline.
- Karot pill: time left, the next weather turn within 6 hours as words right (`YAĞMUR 15:30`);
  with none coming, temperature and sky now (`9° BULUTLU`). The hatch strip is gone from the pill.
- Settings window section **Boşta** drawn (840 x 580, like Oyenza's window): look picker with
  real previews, Şerit (±3 SA / ±6 SA / TÜM GÜN), Bugün çalanlar, Hava (off by default, sub line
  names Open-Meteo), Konum (WINDOWS / IP / ŞEHİR with a city field), Birim, 24 saat. Toggles in
  ink: filled station = on. Lime stays only for sound.
- Still not drawn: light themes, snow, 12-hour clock, the island quick-settings picker.

## Third render (2026-11-17, `renders-3`)
- Berke: the top right must not stay empty when weather is off. The head right is now a slot:
  **Otomatik** (weather when on, else sound), Hava, Ses (output, `%62 · KULAKLIK`,
  `VARSAYILAN ÇIKIŞ`, a lime station only while sound flows), Çalan (title, artist,
  `1:12 / 2:05`; the run's song label hides), Boş. Settings row `Sağ üst`.
- A click opens the welcome view (ADR 0011 has no hover-open); captions fixed.
- UI language: Turkish and English first, table built for more (plan Decision 6).
