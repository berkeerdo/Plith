# R7 settings window design

Date: 2026-11-20. Status: approved by Berke 2026-11-20 ("onaylıyorum her şeyi"), builder check done.
Inputs: the drawn section Boşta (`design/r7/renders-3/sheet.png`, source `design/r7/karot.html`
`settings()`), Oyenza research (`research/2026-11-17-oyenza-settings-and-hud.md` section 1),
idle look direction (`specs/2026-11-17-idle-look-direction.md`, ADR 0014), languages (ADR 0015),
the R7 background in `journal/2026-11.md` (2026-11-17).

## 1. Intent
Today every setting lives in `%LOCALAPPDATA%\Plith\settings.ini`, which is read once at start, and
the OSD is steered only by `--flyout`. A user cannot change the idle look, turn weather on, pick a
language or turn the OSD off without editing a file and restarting. R7 gives these a home: two
layers like Oyenza (a small quick layer in the island, a full window), and Berke's bar for it:
"bizimki daha iyi olmalı" than Oyenza's Appearance page. The window speaks the same language as
the island (Ray Paneli: Loft palette, Martian Mono numbers, rulers, stations), and every change
shows at once on the island. It also gives `WeatherService::reconfigure` its first caller.

## 2. Goals, non-goals, anti-goals
Goals
- A settings window with four sections: **Genel, Boşta, OSD, Hakkında** (Berke 2026-11-20,
  option A: only sections that hold real settings).
- A quick layer in the island: the look picker plus a row that opens the window.
- Every change applies live and is saved at once. No Save or Apply button.
- The language switch redraws the island, the window and the OSD word in the new language.

Non-goals (this round)
- Sections Görünüm (themes; nothing calls `set_palette` yet), Sekmeler (no tab hiding yet), Ada
  (open delay, width, display choice). They join the sidebar when their feature exists.
- Start with Windows, a tray icon, global shortcuts, the 12 hour clock (the `clock24` key exists
  but nothing draws a 12 hour clock, so no row for it this round).
- Watching `settings.ini` for hand edits while running. The file is read at start; while the app
  runs the window is the writer and its last write wins.
- High contrast mode, screen reader support (UI Automation). Keyboard use is in.

Anti-goals (a failure even if it works)
- Cost at rest. With the window closed the app must wake exactly as before (idle check 0 wakes
  on the UI and render threads). Closing the window frees its swap chain and text caches.
- Stealing focus. The window opens only on a user click, never by itself, never topmost.
- The banned attractor from the idle direction: an iOS-style widget gallery, theme cards with
  live preview as decoration, drag and drop.
- Lime for anything but "sound flows now". Toggles are in ink: a filled station is on.
- A switch that does nothing. A row exists only if its setting changes something on screen.

## 3. Constraints
- Rust, our engine only (Direct2D, DirectWrite, DirectComposition through `windows`). The render
  thread owns every graphics object; the UI thread never draws.
- `plith.exe` stays medium integrity; the window lives in `plith.exe`, not in the UIAccess helper.
- Structure limits: 500 source, 80 per function. Pure logic (layout, hit test, focus order,
  apply mapping, text field editing) in `plith-core` with unit tests.
- User-facing strings come from the language table (ADR 0015): every new string in all eight
  languages, upper case rules per ADR 0012 for captions.
- Motion, if any, writes a `motion` line and is measured on the console, not over RDP.

## 4. Approaches
1. **Same process, second window, same engine (picked).** A second top-level HWND on the UI
   thread; the render thread gets a second target and draws the window from a display list like
   the pages. Previews use the real pill drawing with live data; a change is a direct call into
   the running app. Cost: the render thread serves two targets, and the app gets keyboard input
   for the first time.
2. **Separate process `plith.exe --settings`**, writing `settings.ini` that the island watches.
   Crash isolation and hand edits become live for free, but real-data previews need a data
   channel or show fake data, the single-instance rule must change, and the switch waits on a
   file watcher.
3. **WebView2 hosting the HTML mock.** Closest to the mock fastest, but a large runtime
   dependency, a JS bridge and a second visual system next to the engine; against the point of
   the Rust rewrite.
Dropped: Win32 common controls (cannot carry the Loft look). Berke handed the how over
(2026-11-20, "önerin ne ise onaylıyorum"); approach 1 is my call.

## 5. Opening and closing
- **Quick layer.** A gear mark at the right end of the tab row, mirroring the home mark of ADR
  0016 (16 x 16 DIP, station stroke weight, not a station; `PageId::TABS` stays four). A click
  turns to a page `PageId::Quick` in the island: caption `KAPALI ADA`, the six looks as small
  true previews in one row, and under a rule one row `Tüm ayarlar` with an open glyph. A look
  click applies at once. The gear draws full ink while the quick page is open, like the home mark.
- **Window.** Opened by the `Tüm ayarlar` row. One window only: a second open restores and
  focuses the existing one. Centered on the monitor the island is on. Normal top-level window with
  a taskbar button, the standard Windows caption in dark mode (`DWMWA_USE_IMMERSIVE_DARK_MODE`),
  title "Plith · Ayarlar" localized. Client area 840 x 640 DIP as drawn, not resizable, per
  monitor DPI aware (rescales on `WM_DPICHANGED`). Opens on the section last shown in this run,
  Boşta the first time.
- Closing (caption button, Alt+F4, Esc) destroys the window and releases its target. The island
  keeps running. App exit closes the window first.

## 6. Sections and rows
Layout follows the drawn Boşta section: sidebar 156 DIP with the caption `AYARLAR` and the
section names (active one marked with a filled square), content column with a 20 DIP heading,
one dim line under it, then rows of label, dim sub line, control right. Controls: **choice
group** (words in caps, the picked one underlined and in full ink), **toggle** (`AÇIK` / `KAPALI`
word plus two stations, filled = on), **text field** (one line), **button** (word in caps).

Genel
| Row | Control | Key | Live effect |
|---|---|---|---|
| Dil | choice list: Otomatik + 8 languages, each named in itself (Türkçe, English, Deutsch...) | `lang` | island, window and OSD word switch at once |
| Ada açılınca | Karşılama / Son sayfa | `open_on` | the next open |

Boşta (as drawn, minus the 24 hour row)
| Row | Control | Key | Live effect |
|---|---|---|---|
| Kapalı ada | six true previews (Karot, Saat, Çalan, Ses, Sade, Gizli; the names `Text::Look*` already use) | `look` | the pill changes shape and content |
| Şerit | ±3 SA / ±6 SA / TÜM GÜN | `span` | welcome view chart |
| Sağ üst | Otomatik / Hava / Ses / Çalan / Boş | `head` | welcome view head |
| Bugün çalanlar | toggle | `plays` | chart band |
| Hava | toggle, sub line names Open-Meteo | `weather` | `WeatherService::reconfigure`, start or stop |
| Konum | Windows / IP / Şehir, city field shown for Şehir | `location`, `city` | reconfigure, fresh fetch |
| Birim | °C / °F | `unit` | every temperature redraws |

OSD (new keys)
| Row | Control | Key | Live effect |
|---|---|---|---|
| Ekran göstergesi | toggle, sub "Ses ve parlaklık değişince, oyunların üstünde" | `osd` (default on) | off: the OSD helper is stopped with its supervisor (no 1 Hz wake); on: started |
| Windows göstergesini gizle | toggle, sub warns it hides Windows' own flyout while Plith's OSD runs | `flyout` (default off, `off`/`hide`) | applies to the connected helper |
`--flyout` on the command line still wins over the file (dry run stays command line only). The
flyout row is disabled (dim, not clickable) while `osd` is off.
A `flyout` change restarts the OSD side with the new mode (my call, builder check 2026-11-20): no
protocol bump. Windows' own flyout may show or hide for a moment while the helper restarts; that
happens only on the user's own switch, so it is accepted.

Hakkında
| Row | Control |
|---|---|
| Sürüm | `0.1.0` and build date, no update check |
| Ayar dosyası | path, button `AÇ` opens the folder in Explorer |
| Veri | weather by Open-Meteo, IP location by ipapi.co (only when chosen) |

## 7. Live apply and saving
- One path: the window (or the quick layer) sends a `SettingChange` to the UI thread; the UI
  thread updates its `Settings`, applies the effect from the tables above, and saves the file.
- Saving is atomic (write a temp file, rename) and runs off the UI thread. The city field saves
  on Enter or when it loses focus, not per key; a fetch waits for the save, not for typing.
- Language today is resolved once at start (`idle_wiring::language_of`). It becomes changeable:
  the feed, the idle model and the window take the new `Lang`, and every cached page bitmap and
  text layout built with the old language is dropped and rebuilt (prewarm, ADR 0010). The OSD
  word follows on the next `Level` message (protocol 2 sends it with each level).
- Weather off stops the service (`stop()`), on builds a new one and wires it to the feed again;
  the weather link owns the service for the whole run (my call, builder check). `stop()` can take
  seconds, so it runs off the UI thread. Konum, Şehir or Birim changes while weather is off only
  update the settings; they reach the service when it starts.
- Leaving the quick page: the gear again, the home mark or a tab. `open_on` "Son sayfa" never
  records the quick page.
- The window takes the foreground with `SetForegroundWindow` after the island click; if Windows
  refuses, the taskbar button flashes, which is accepted.
- A failed save is logged and shown in the window as one dim line under the heading; the change
  stays applied for this run.

## 8. Drawing and input
- The window draws only when something changed: input, hover, focus, a setting, a language, or
  the preview data (the Karot preview's clock, once a minute while the window is open). No
  continuous loop while open.
- Look previews are drawn by the same pill drawing as the island, from the same live facts, at
  the drawn preview size (no stock pictures).
- Mouse: hover marks a row's control, click acts. Keyboard: Tab and Shift+Tab move focus in
  reading order (sidebar, then rows), arrows move inside a choice group, Space or Enter acts, Esc
  closes. The focus mark is a thin ink outline.
- Text field: one line, at most 64 characters; typing, Backspace, Delete, Left, Right, Home, End,
  Ctrl+A, Ctrl+C, Ctrl+V. No IME composition (the eight languages type without one); caret blinks
  only while the field has focus.
- Motion is minimal: station fill and underline change at once or within the engine's existing
  tune; any motion writes its `motion` line.

## 9. Proof
- Unit tests in `plith-core`: section layout and hit test, focus order, the change to effect
  mapping, text field editing, `osd` and `flyout` keys round-tripping through `settings.ini`.
- Renders of each section at 100 and 200 percent scale, in Turkish and German (longest words),
  inspected before done.
- Console session (Berke present): open the quick layer and the window from the island, change
  every row and see the island follow; switch to German and back; weather on with Şehir and a
  real fetch; OSD off then on with a volume key; idle check after the window was opened and
  closed (0 wakes as before).

## 10. Left to the builder
- Pixel layout of Genel, OSD and Hakkında, following the Boşta drawing.
- The gear glyph, and where the six quick previews fit in the island page width.
- How the render thread holds and paces two targets; how the OSD side stops and restarts.
- Where the window code lives (a window module in `plith-shell`, wiring in `crates/plith/src/app/`)
  under the structure limits.
