# Plith vs Oyenza Studio: design and motion review

Date: 2026-11-14. Input for the Rust rewrite session. Companion file:
`2026-11-14-oyenza-engine.md` (Oyenza's engine read from its source).

## 1. Oyenza measured on this PC (not read from docs)

Oyenza Studio 0.2.30, installed per-user at `%LOCALAPPDATA%\Programs\Poni\poni.exe`. Its own log
(`%LOCALAPPDATA%\Poni\poni.log`) prints a `motion` line per animation. Measured 2026-11-14 on the
RTX 5080 at 240 Hz, 2560x1440:

| Motion | Duration | Frames | fps at 240 Hz | Hitches | Worst gap |
|---|---|---|---|---|---|
| pill (first show) | 625 ms | 150 | 238.4 | 1 | 8.5 ms |
| expand | 729 to 750 ms | 176 to 181 | 240.0 | 0 | 4.6 to 5.2 ms |
| collapse | 658 ms | 159 | 240.0 | 0 | 4.7 ms |
| page turn | 263 to 546 ms | 64 to 131 | 237.8 to 240.0 | 0 or 1 | 4.2 to 8.3 ms |
| resize (page height change) | 296 to 521 ms | 72 to 126 | 220.8 to 240.9 | 0 or 1 | 4.2 ms, once 45.9 ms (first glass screen copy) |

Per frame: draw 0.2 to 1.0 ms, present 0.1 ms, a ~1 ms "mask" step every 8th frame. Renderer line:
`GPU (hardware), DirectComposition, 692x352 px`. Frame clock: `display vblank at 240.0 Hz`.
Settings defaults worth noting: `hover_delay_ms` 220, `collapse_delay_ms` 0, `expanded_width` 620.

**This is the bar.** A Plith motion that drops frames at 240 Hz, or takes longer than these durations
without a reason, loses the side-by-side comparison.

## 2. Plith today (WPF, 0.3.1, rendered 2026-11-14 with `scripts/render-widgets.ps1`)

Every page lives in one fixed 356 x 164 DIP frame.

What works:
- The Loft floor: zinc ground, light ink, lime only as a marker (volume fill, focus ring). Calm and
  coherent, more restrained than Oyenza's black.
- Media page: clean hierarchy (art, title, artist, seek with both times, transport).
- The audio page names the real device and route (`Logitech G733`, `Voicemeeter | A1`). No
  competitor shows this.

What does not:
1. **The frame is too small and the same size for every page.** 356 x 164 against Oyenza's 620 wide.
   The shelf fits 5 tiles per row and truncates names in the middle (`Proje...ssets`); the audio
   page is half empty. Pages should size to their content and the frame should morph between sizes.
2. **No visible navigation.** Pages are reached by paging with a dot rail. Oyenza shows a tab bar
   (icon tabs, the active one a pill with its label, clock at the right): you always know where you
   are and can jump directly.
3. **The shelf reads as a file grid, not a notch surface.** The hovered filename sits on top of the
   frame and is clipped by the edge, tiles are flat squares with a thick lime outline, the count and
   rail float in the footer. It is also two processes (DropCatcher) because Release runs at high
   integrity, which costs a window swap on every page turn onto it.
4. **Weather breaks the language.** It is the only page with a photographic gradient; every other
   page is on the Loft floor. Pick one: either every page can carry an ambient backdrop or none does.
5. **The audio page undersells the one thing nobody else has.** A big `62%` and a thin slider. For
   Voicemeeter users it should show the buses or strips they care about, levels/meters, mute, the
   active output, at a glance.
6. **Mixed type.** Mono numerals for the clock and volume, a grotesk for labels, an em dash in the
   now-playing line. Fine individually, not yet a system.

## 3. Oyenza's design, read from its README images

- The island is pure black and meets the top edge of the screen, so it reads as part of the bezel.
- Expanded: ~620 wide, a top tab bar (music, calendar, shelf, tasks, timer, mascot, add-ons; pin,
  settings, date and time on the right). Pages change height and the island resizes to fit.
- Resting pill: one line of live state (lyric line with a beat meter, a timer ring, volume bar,
  headphone battery, prayer countdown). Each "glance" is a small purpose-built layout.
- Themes: Classic (black), Oyen (cream, orange accent), Glass light/dark (screen copy blur), Pill,
  Diorama (pixel-art scene that follows the clock).

Its weaknesses, which are Plith's opening:
- **Feature soup.** Prayer times, caffeine curve, IELTS, bank notifications, PSU wattage, a mascot.
  The tab bar keeps growing; it reads as a hobby toolbox, not a focused product.
- **Nothing for audio or games.** No Voicemeeter, no per-bus control, no device routing, no display
  over exclusive fullscreen (it hides in fullscreen: `hide_in_fullscreen: true`).
- **Shelf is sparse.** Three items in a 620-wide page, labels truncated with an ellipsis.
- Typography is loose: wide letter-spacing in places, mixed weights.

## 4. Direction for the rewrite (recommendation, owner decides)

- **Positioning:** the notch for people who care about sound and play games. Fewer pages, each one
  excellent. Audio, media, shelf, system controls (brightness, mic mute) first. No toolbox.
- **Engine parity first:** one fixed, oversized per-pixel-alpha window; DirectComposition + Direct2D
  on D3D11; a vblank frame clock that only runs while something moves; springs that retarget
  mid-flight. Ship nothing visual before a `motion` log line like Oyenza's exists, so the bar is
  measured, not eyeballed.
- **Integrity split, inverted from today:** the notch (shelf included) runs at medium integrity so
  OLE drag-drop just works in-process; only the over-fullscreen-game OSD is a small UIAccess helper.
- **Visual language:** keep Loft (zinc floor, lime marker, ADR 0014 from praxvon-ui); size pages to
  content; add a tab bar; one icon set; one type system with tabular numerals.
