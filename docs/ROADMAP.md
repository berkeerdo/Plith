# Plith — Roadmap (Phase 5+)

Forward-looking companion to `PLAN.md` (which covers Phase 1–4). Phases 1–4
shipped an audio-first OSD; Phase 5+ is the strategic pivot from
"audio OSD" to **the beautiful edge of Windows** — a unified feedback
and control surface that replaces the chunky rectangles Windows shows
for every hardware key press, and layers optional cards on top.

---

## 1. Vision

Windows shows a different, ugly OSD for every kind of feedback (volume,
brightness, keyboard backlight, mic mute, Caps Lock, airplane mode,
battery), and every OEM utility (Logitech G HUB, Corsair iCUE, Razer
Synapse) piles their own OSD on top. The result is a fragmented,
dated feedback layer that hasn't meaningfully evolved since Windows 7.

**Plith becomes the single, themeable surface that catches all of it.**
Every key press that today spawns a chunky Windows rectangle instead
flows through Plith's overlay, in one design language, with themeable
accent, positioning, and behaviour.

**Tagline:** *the beautiful edge of Windows.*

---

## 2. Positioning

- **Not** "another audio OSD" — that market is small and served
  (ModernFlyouts, FancyOSD).
- **Is** the Windows equivalent of the macOS "boring notch" category
  (Boring Notch, DynamicNotch, NotchNook, MediaMate) — a category
  Windows has no strong entrant in.
- Single narrative for launch: *"Windows doesn't have a notch, but it
  deserves one."*
- Keeps the OSD-first behaviour as a legacy mode for users who don't
  want persistent surface presence.

---

## 3. Preset modes

Four shipped modes; every one tweakable further under Custom.

| Mode | Behaviour | Target user |
|---|---|---|
| **Classic OSD** | Today's Plith unchanged. Edge-positioned, appears on event, hides after ~2 s. | Users who want zero background presence. |
| **Ambient Notch** | 4–6 px thin strip top-center, always visible. Expands on hover or on event, holds ~2 s, collapses. | Users who like a subtle background presence. |
| **Full Notch** | ~28 px persistent strip top-center with 2–3 always-on cards (clock / next track / mic status). Hover expands into a full panel. | Everything-in-one-place users. |
| **Custom** | User picks position, height, which cards are visible, which events trigger expansion. | Power users. |

Preset choice sets the shell behaviour. Individual cards additionally
carry their own micro-config (Audio → "hover opens mixer", Media →
"tint background by album art", etc.).

---

## 4. Card system

Every feature is a **card** rendered inside the notch/OSD. Cards are
first-class, isolatable units:

- Each card owns: its ViewModel, its View (a UserControl), its data
  service, and its trigger conditions (`OnAudioEndpointVolumeChange`,
  `OnMediaSessionAdvance`, `OnKeyEventVK_BRIGHTNESSUP`, `OnBatteryLow`).
- Cards register with a `CardHost` service that decides which card is
  visible at any moment (event-driven for OSD/Ambient, always-on-set
  for Full, user-set for Custom).
- Cards can request expansion (compact → hover-panel state) via a
  common contract.
- Themeable via the existing `AccentTheme` service; every card
  inherits accent, surface, and typography tokens.

**Shipped cards, in priority order:**

1. **Audio** (existing — Voicemeeter + Windows endpoints).
2. **Media** (existing — SMTC now-playing + transport).
3. **System Controls** — brightness, keyboard backlight, mic mute,
   Caps/Num/Scroll Lock indicators, airplane / Wi-Fi / Bluetooth
   toggles. This is the card that replaces Windows' fragmented native
   OSDs and unifies OEM utilities.
4. ~~**Battery** — laptop only. Low / critical / full-charge alerts,
   percent readout on hover.~~ **Dropped as a card.** The readout lives on the notch's
   "now" page beside the time, and collapses on a machine without a battery. What is
   genuinely not covered there is the ALERT half — low, critical, fully charged — which
   is an event rather than a readout and belongs wherever the interception layer handles
   events, not in a card of its own. Recorded here rather than deleted so the distinction
   survives: the number is done, the alerts were never started.
5. **Notifications** — a peek at the last N Windows notifications,
   plus quick dismiss. Windows' Action Center is ugly and slow; a
   Plith notification card is a natural extension.
6. **Shelf** (Dropover-clone). Persistent floating drop target on the
   notch's underside. Drag files in from anywhere, multi-select
   staging, drag out to any destination. Windows has no good
   equivalent → market gap.
7. **Weather** — small always-on temperature + condition, hover for
   next 12 h.
8. **Timer / Pomodoro** — start/pause from notch, hover for full
   controls.
9. **Clipboard peek** — last N text clips, hover to expand.

Cards 3–5 land in Phase 6. Cards 6–9 land in Phase 7+ as separate
milestones — every card is independently shippable, so the roadmap
doesn't gate on all of them landing together.

---

## 5. Interception layer

The card system needs a clean layer that catches Windows events and
routes them to cards. Most of this reuses infrastructure Plith already
has (`WH_KEYBOARD_LL` hook for the summon hotkey, `MMDeviceEnumerator`
for audio endpoints, SMTC session manager for media). Missing pieces:

| Signal | Source |
|---|---|
| Volume / mute keys | `WH_KEYBOARD_LL` (already wired). |
| Media transport keys | `WH_KEYBOARD_LL` VK_MEDIA_*. |
| Brightness up/down | `WH_KEYBOARD_LL` VK_BRIGHTNESS_*, apply via DDC/CI (`dxva2.dll`) — see the Phase 6 note; WMI reaches laptop panels only. |
| Keyboard backlight | Vendor SDKs (Razer Chroma, Corsair iCUE, Logitech G HUB) — start with Microsoft Precision + laptop-native, add vendor SDKs later. |
| Mic mute | Vendor keys → intercept + broadcast via Windows.Devices.Enumeration. |
| Caps / Num / Scroll Lock | `WH_KEYBOARD_LL`, read state via `GetKeyState`. |
| Airplane / Wi-Fi / BT | `RadioManager` COM API. |
| Battery events | `RegisterPowerSettingNotification` + `GUID_BATTERY_*`. |
| Notifications | `UserNotificationListener` (WinRT). |
| Shelf drag targets | `IDropTarget` implementation, Explorer-integration API. |

Each becomes a `IEventSource` service; cards subscribe to the sources
they care about.

---

## 6. Phase breakdown

### Phase 5 — Consolidation + tech debt — **code-complete, unmerged**

Branch `feature/phase-5-cardhost`. Design: `docs/superpowers/specs/2026-09-02-cardhost-design.md`.

Shipped:

- **CardHost** — owns which cards are visible and is the single authority for when
  the OSD appears. Holds no reference to any WPF window, so the show policy is
  unit-tested with no `Application` and no HWND. Today's OSD is an Audio card
  (`Order 20`) plus a Media card (`Order 10`); orders are spaced by 10 so Phase 6
  cards slot in without renumbering. `OsdViewModel` deleted; `OsdOrchestrator`
  reduced to a pure audio/media source driver holding no display authority.
- **Fullscreen-video auto-hide** — on by default, with a Settings toggle and a
  user-editable process list. Fails toward showing in every ambiguous case.
- **Accessibility** — screen-reader names across the OSD and Settings, OSD live
  regions, accent swatches converted from `Border` to `Button` so they are
  keyboard-reachable, focus indicators restored to six styles that had stripped
  them, a high-contrast palette, and `scripts/check-a11y.ps1` as the guard.

**The success metric is not yet met.** "Identical to 0.1.5 from the user's side" has
no automated evidence — see `docs/PHASE5-VERIFICATION.md`.

Two findings worth carrying forward, both caught only because something was measured
rather than reasoned about:

- A `DataTrigger` binding `RelativeSource AncestorType=ContentPresenter` inside
  `DataTemplate.Triggers` does **not** resolve to the item container; it walks past it
  to an outer presenter where `AlternationIndex` defaults to 0, so the trigger matched
  every item. The fix routes the index through the item root's `Tag`. The warning is
  recorded in both the spec and the plan.
- Windows 11's Fullscreen Optimizations (default on) converts most "exclusive
  fullscreen" games into borderless flip-model windows reporting `QUNS_BUSY`, so the
  D3D veto never fires for them and game safety rests entirely on the media-session
  predicate. Any future change to that predicate is a game-safety change.

Found during post-code-complete verification, fixed on the branch. All four came out of
reading the live UI Automation tree or the WPF source; none was catchable by the build,
the tests or the lint as they stood:

- Both card views set `AutomationProperties.Name`/`LiveSetting` on a bare `<Grid>`. WPF
  creates no automation peer for a panel, so the OSD's live region reached nothing at all.
  Properties moved to the `UserControl` roots; `LiveRegionAnnouncer` raises
  `LiveRegionChanged`, which WPF does not raise on its own.
- The OSD's `ItemsControl` container named itself from the bound item's `ToString()`, so
  cards announced `Plith.Cards.AudioCard`. `ICard.AccessibleName` is now required and each
  card's `ToString()` returns it.
- A focused accent swatch ignored Enter. `ButtonBase.OnKeyDown` handles `Key.Enter` only
  when `KeyboardNavigation.AcceptsReturn` is set, and it defaults to false — the swatch
  code carried a comment claiming otherwise.
- Settings' content `ScrollViewer` was a nameless tab stop announcing only "pane"; WPF
  makes `ScrollViewer` focusable by default. Named rather than made unfocusable, to keep
  keyboard scrolling of a long page.

`scripts/check-a11y.ps1` now catches the first, second and fourth shapes, and each new
check was validated in both directions against the tree from before the fix. The third
needs a key press and has no static equivalent.

Also measured, no defect found: the OSD show path does not leak. It was driven through 160
volume changes in two runs. The first run grew (+8 GDI objects, +21 handles, +3 threads);
the second, on the already-warmed process, held GDI objects at exactly 22 and ended with
*fewer* handles and threads than it started with, while private bytes oscillated with GC in
both directions. So the first run's growth was one-time initialisation on first show, not
per-cycle accumulation — which matters because this is the path every volume key press
takes, and Phase 5 moved it behind an `ItemsControl`.

Measured while a game was actually running, which the earlier notes could only predict:
a current title reported `QUNS_BUSY` (2) rather than `QUNS_RUNNING_D3D_FULL_SCREEN` (3)
while covering the whole monitor, and the OSD drew over it correctly. So Windows 11's
Fullscreen Optimizations really do put most games on the composited path, and the D3D veto
really is dead code for them — game safety rests on the media-session predicate exactly as
recorded.

One title looked like an exception: the OSD did not appear over Valorant in its Fullscreen
mode, while it worked over the same machine's other games and over Valorant in borderless.

**The most likely cause is a testing artefact, and it is worth ruling out before anything
else.** That session ran a Debug build out of `bin\Debug`, and `app.manifest` sets
`uiAccess="false"` there on purpose — only `app.release.manifest` requests `uiAccess="true"`,
and Windows honours it only for a signed binary in a trusted location. Without UIAccess
`BandWindow` cannot use `CreateWindowInBand` and falls back to `CreateWindowEx`, i.e. an
ordinary topmost window. That predicts exactly what was seen: an ordinary topmost window
composites over borderless and Fullscreen-Optimizations games, and cannot cover a true
exclusive-fullscreen swapchain. **Retest with the installed Program Files build before
treating this as a product defect.**

If it still fails there, two candidates remain and they call for different fixes: Valorant
takes true exclusive fullscreen, where the display is scanned out from the game's own
swapchain and nothing composites over it however correct the OSD is — in which case
`README.md`'s "draws above exclusive fullscreen" is the claim that is wrong — or Riot's
Vanguard anticheat blocks the overlay. `SHQueryUserNotificationState` returning 2 or 3
during that mode separates them in one reading, and `OsdHost` now logs each show transition,
so the log also distinguishes "never asked to show" from "shown and something on top of it
won".

Environment fact worth keeping: **the OSD cannot be screenshotted over RDP.** The band
window is layered and drawn with `UpdateLayeredWindow` — absent from a plain `BitBlt`,
absent with `CAPTUREBLT`, and `PrintWindow` with `PW_RENDERFULLCONTENT` returns solid
black — while the window is provably on screen and correctly positioned. Anything
pixel-based has to run from the physical console. UI Automation works fine over RDP.

Deferred to Phase 6 (recorded so they are not rediscovered):

- Accent swatches should be `RadioButton`s, not `Button`s: that brings the UIA
  `SelectionItem` pattern and arrow-key group navigation, and would remove the nine
  extra tab stops the current row costs. Selection state is currently conveyed by
  `AutomationProperties.ItemStatus` as a stopgap. It would also retire the
  `KeyboardNavigation.AcceptsReturn` line each swatch now carries, since a radio button
  in a group is driven by arrow keys rather than by Enter.
- `OsdOrchestrator` has no tests at all, and it is not an oversight that can be fixed by
  writing some: it constructs `VoicemeeterClient` and `WindowsAudioClient` itself and takes
  a `Dispatcher`, so it cannot be built headlessly. That matters more than it looks, because
  it owns all three `AudioCard.ResetBaseline()` calls — the mechanism behind "switching
  sources must not pop the OSD". The card half of that rule is well tested; the half that
  triggers it is verified only by a human running check 6.1. Injecting the two clients
  behind interfaces would close it, and is worth doing before Phase 6 adds more sources.
- `CardHost.Register` now throws on a duplicate, but the check is **reference equality**
  (`List<T>.Contains`). It catches the same instance registered twice; it does not catch
  two distinct instances sharing an `Id`, which is the shape data-driven registration is
  more likely to produce. Deferred rather than guessed at, because the registration model
  Phase 6 wants has not been designed — decide the `Id` contract first, then guard it.
- Switching *between* high-contrast themes (Black → White) leaves stale colours until
  restart, because `{x:Static SystemColors.*}` resolves once at dictionary load.
  Toggling high contrast on and off works correctly.

Cleared after Phase 5 closed, recorded so the reasoning is not re-derived:

- `MediaCommand` moved from `Plith.Views` to `Plith.Cards`, ending an inverted MVVM
  dependency that was documented as transitional when introduced.
- `SettingsPreview`'s media row now binds to its seeded view model instead of carrying
  the same two strings hardcoded alongside it.
- `src/Plith.Installer` gained accessible names on all 15 of its interactive controls,
  so the lint's `-Root src` scope now passes across both projects.
- `scripts/check-a11y.ps1` runs in CI on changes under `src/**`, with a local
  invocation documented in `CONTRIBUTING.md`. Before this it was invoked by nothing.

### Phase 6 — Notch mode + System Controls card (3–4 wk)

The core pivot.

**Slice 1 — code-complete on `feature/phase-6-notch-shell`, not yet merged, not yet
verified on a running build (see `docs/PHASE6-VERIFICATION.md`):**

- ~~Notch positioning geometry: top-center pinned, height configurable, respects
  Windows taskbar auto-hide.~~ **Shipped.** `NotchGeometry` provides pure top-center
  pinning with a configurable 2–24 DIP strip height; `OsdHost.ResolveTargetScreen`
  respects the saved monitor.
- ~~Ship Ambient Notch preset mode.~~ **Shipped**, behind the new `IOsdPresentation`
  seam, as `AmbientNotchPresentation`. Classic OSD stays default for every install,
  including existing `config.ini` files, so nobody is moved to the notch
  automatically.
- Preset picker + strip-height slider in Settings. **Shipped** — the picker disables
  position editing while the notch is active, since the notch is pinned rather than
  freely placed.
- **Full Notch is deferred to a later slice.** It needed always-on content — mic status
  and a clock — and shipping the notch shell without them would have shipped an empty
  strip. Both now exist: the clock is the "now" page and the mic is on it as well as on
  the system page. What Full Notch still lacks is a decision rather than a part — which
  of them earns permanent space on a strip that is 28 px tall.
- **System Controls — a page, not a card. Brightness and mic mute shipped; backlight,
  lock keys and airplane mode not started.**

  The rename is a design correction rather than wording. Cards were the shape the OSD
  had when everything it showed answered something you had just pressed; the notch
  replaced that with two shapes — a HUD that answers a key, and a frame you deliberately
  open. Nobody presses a key to be told their microphone's state, so a card would have
  been a thing that appeared unasked.

  Brightness goes over **DDC/CI**, not `WmiMonitorBrightnessMethods` as the table below
  originally said. The WMI version was written first and thrown away: it reaches a
  laptop's built-in panel and nothing else, so on the desktop this is developed on it was
  code that could never once run. DDC/CI talks to the monitor over the video cable, which
  is the case a desktop actually has — measured before a line was written, and the
  attached panel answers with a 0..100 range. Laptop panels that refuse DDC/CI will need
  the WMI path back as a fallback, added when there is a machine to prove it on.

  The page installs itself only when it has something: a display that refuses DDC/CI and
  a machine with no capture device leave it out of the pager entirely, rather than adding
  a page you can swipe to and be told nothing on.

  **Verified offscreen, not on a running build.** The client was driven end to end against
  the real display (attach, read, write, re-read, dispose) and the page was rendered at
  the frame's exact size; the range arithmetic is covered by `BrightnessMathTests` for the
  coarse and non-zero-floor panels no machine here has. What has NOT been exercised is a
  live drag — the coalescing that keeps a DDC/CI write off the mouse-move path is reasoned
  and structurally sound, not measured.
- ~~**Battery card** — laptop-first.~~ **Dropped.** The battery is on the notch's "now"
  page, beside the time it belongs with, and it collapses on a machine that has none. A
  card would be a second place for one fact — the same reason the audio widget page was
  removed once the volume HUD already answered a volume key. If a laptop-specific reading
  ever needs more room than a line (time remaining, health, per-app drain), that is a
  different feature and deserves its own entry rather than this one reopened.
- Preset migration: existing installs default to Classic OSD; a
  one-shot "meet the new Plith" nudge lets them try Ambient / Full. **Not started.**
- Success metric: install-to-second-launch retention crosses 60 %
  (currently 0.1.x sits at unknown baseline — instrument this).

### Phase 7 — Shelf + Notifications (3–4 wk)

The two features that Windows has no good answer for.

- **Shelf card** — persistent floating drop target on the notch's
  underside. Drop files in, they stage; drag out to any destination.
  Multi-selection stashing.
- **Notifications card** — last N notifications with quick dismiss.
  Aspires to replace Action Center for people who never open it.
- Notch dynamic sizing: notch grows when shelf has stashed items.
- Success metric: Product Hunt Day 1 launch with the notch + shelf
  as the core narrative.

### Phase 8 — Weather / Timer / Clipboard (2–3 wk)

Small always-on cards that make the notch feel dense with utility.

- Weather card (OpenWeather API or Windows Location + built-in).
- Timer / Pomodoro card.
- Clipboard peek card (respects Win+V exclusion list).
- Cards are opt-in; default preset ships with Audio + Media + System
  Controls only so first-run isn't overwhelming.

### Phase 9 — Extensibility (long, deprioritized)

- Config-driven card catalog: users pick which cards are enabled from
  a Settings gallery.
- Community cards eventually: a small plugin API (probably WPF
  UserControl + a data-source interface). Only if there's community
  pull.
- No plugin API on the roadmap for its own sake — extensibility ships
  when it earns the complexity.

---

## 7. Branding

- **Keep "Plith".** Brand equity, plith.app domain, GitHub, logo, and
  Themes Studio design language are all in place — rebranding wastes
  earned surface area.
- Positioning tagline evolves from *"Modern Windows audio OSD"* to
  *"The beautiful edge of Windows."* Landing page hero rewrite in
  Phase 6.
- "Plith by Praxvon" footer stays; Praxvon is the umbrella.

---

## 8. Success metrics

Instrument before Phase 6 ships so we can compare.

| Metric | 0.1.x baseline | Phase 6 target | Phase 7 target |
|---|---|---|---|
| Install → 2nd-day launch | unknown | 60 % | 75 % |
| Daily active install (per opt-in telemetry) | unknown | +200 % | +500 % |
| GitHub stars | current | +500 | +2 000 |
| Product Hunt Day 1 rank | n/a | n/a | top 5 in Productivity |

Telemetry: opt-in on first launch, no PII, aggregated card-usage
counts only.

---

## 9. Open questions (revisit before starting Phase 5)

- **Card-vs-notch pinning model**: does the notch always show all
  enabled cards side-by-side, or does it show one card at a time and
  cycle by event priority? Ambient/Full modes probably diverge here.
- ~~**Multi-monitor**: notch pins to which monitor? Primary only, or
  per-monitor? Different from OSD which is per-event.~~ **Closed by Phase 6
  slice 1.** The notch pins to the same saved monitor device name Custom
  placement already used (`OsdHost.ResolveTargetScreen`), falling back to the
  primary display when that monitor is unplugged. Not per-monitor — one notch,
  on one chosen display, same as Custom placement.
- **Vendor OSD suppression**: to actually *replace* Logitech/Corsair
  native OSDs we may need to detect them running and suggest their
  OSD toggle be turned off. Do we ship that as onboarding help, or
  actively try to suppress?
- **Store presence**: notch pivot is a strong Microsoft Store hook
  (Store featuring "notch" apps has precedent). Re-attempt Store
  submission with the new positioning post-Phase 6?
