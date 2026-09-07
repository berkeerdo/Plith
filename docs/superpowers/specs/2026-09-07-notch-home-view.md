# Phase 6 slice 2 — Notch home view (ambient status on hover)

**Status:** design agreed 2026-09-07. Implements the "other parts" of the notch-app
category the user named when asking for Alcove parity, minus notifications (explicitly
not wanted) and minus the file shelf (deferred to slice 3 behind a feasibility spike).

**Depends on:** slice 1 (`feature/phase-6-notch-shell`), specifically the notch morph
committed as `ec0e4d1` and the hover poller from Task 7.

---

## Problem statement

Slice 1 gave the notch a shape and a motion. It has no reason to exist when nothing is
happening. Hovering it today opens exactly the same panel a volume key opens — the audio
card, plus the media card if a session is playing — so a deliberate hover is rewarded with
information the user did not ask for and already had.

Every app in this category solves that the same way: the notch has **two different open
contents**. An *event* opens the HUD for whatever changed. A *hover* opens a small ambient
home view — time, weather, battery, what's playing. That distinction is the feature.

## Goals

1. A deliberate hover on the resting notch opens a compact ambient row: clock, weather,
   battery — alongside whatever cards were already going to render.
2. An event (volume key, media change) does **not** show the ambient row. It shows what
   changed, exactly as today.
3. Classic OSD is completely unaffected. The ambient row is notch-only.
4. Weather degrades silently and completely: no location permission, no network, no
   provider response — the column disappears, nothing else changes, nothing is logged
   at warning level on a schedule.

## Non-goals

- Notifications. Explicitly declined by the user.
- The file shelf. Slice 3, and gated on a spike (see "Deferred" below).
- Making the audio card conditional. `AudioCard.IsVisible` is a constant today and Classic
  depends on that; changing it is a Phase 5 behaviour change and out of scope here.
- A weather forecast, hourly data, or a second location. Current conditions only.

---

## §1 — One card, not three

The obvious decomposition is a `ClockCard`, a `WeatherCard` and a `BatteryCard`. It is
wrong, and the reason is layout rather than architecture.

`OsdContent`'s card stack is an `ItemsControl` with a vertical `StackPanel` panel template,
and every non-first card renders a divider above it. Three separate cards would therefore
produce three stacked rows and two dividers before the media card is even reached — a tall
column of one-line facts. The reference apps all render this content as a single compact
**row**: time on the left, conditions in the middle, battery on the right.

So: **one `AmbientCard`** whose view is a three-column `Grid`. Each column has its own
independent presence rule and collapses on its own:

| Column | Present when |
|---|---|
| Clock | always (while home is open) |
| Weather | a snapshot has been fetched successfully and is not stale beyond `WeatherMaxAgeMinutes` |
| Battery | `GetSystemPowerStatus` reports a battery is present (desktops collapse it) |

If every column collapses the card reports `IsVisible == false` and disappears entirely,
which on a desktop with no network is the whole card.

`Order = 5` — above `MediaCard` (10) and `AudioCard` (20), so the ambient row reads at the
top of the panel where a notch's home content belongs.

## §2 — What "home" means, and when it is open

New pure type `NotchHomeState`: a bool and a change event, no WPF dependency.

```
IsOpen : bool
Changed : event Action
Open()   // idempotent
Close()  // idempotent
```

`AmbientCard.IsVisible` is `_home.IsOpen && (clock || weather || battery has content)`.
Since clock is always content, in practice it is `_home.IsOpen`.

**Opened by** `OsdHost.OnNotchHoverChanged(inside: true)`, before it calls `ShowOsd`.
The ordering matters: `CardHost.RecomputeVisibleCards` must have added the card before the
panel is measured, or the notch expands to a panel height computed without the ambient row
and the row is clipped for one show.

**Closed by** `OsdHost`, in three places: whenever `ShowOsd` runs for a reason other than
hover (an audio or media event, so the row never shows for those); whenever the notch finishes
collapsing back to rest (the `AnimateToRest` completion callback, for the ordinary
uninterrupted case); and whenever the presentation is rebuilt (both branches of
`ApplyPresentationMode`, so a mode switch always settles fully closed). The first of these
exists because `IsAtRest` treats an in-flight collapse as already at rest, so an event
arriving mid-collapse can replace the collapse's animation clock before it ever completes —
WPF raises no `Completed` for a clock replaced that way, so the `AnimateToRest` close alone is
not reliable. Not on cursor exit: leaving the resting rectangle is the normal way to move
*onto* the open panel (slice 1 §3 already relies on this), so closing there would take the
row away the instant the user reached for it.

**Never opened by** anything else. `ShowOsd` from an audio or media event does not touch it.

**Classic:** `ApplyPresentationMode` closes it when it builds a `ClassicPresentation`, which
covers both the settings switch and the covered-monitor fallback. A Classic user never sees
the row because it is never opened, not because the card checks the mode — the card knows
nothing about presentations.

### Why a shared flag rather than a second card list

`CardHost` is documented as "the single authority for when the OSD appears" and its contract
is that each card owns its own `IsVisible`. A second list, or a mode parameter on
`RecomputeVisibleCards`, would put presentation state inside the one type that is
deliberately free of it. A card reading a shared state object is the existing pattern —
`MediaCard` already reads `SettingsService` for `CompactMode` the same way.

## §3 — Data sources

Each source splits gather-from-the-world / decide, mirroring
`FullscreenVideoWatcher` / `FullscreenVideoDetector`. Only the decide half is unit-tested;
that split is what made slice 1's predicates testable on the headless suite.

### 3.1 Clock

`DispatcherTimer` at 1 Hz owned by the card, started in `Activate()`. Formatting is a pure
static (`AmbientFormatter.FormatClock`) taking a `DateTime` and a `CultureInfo` so the tests
do not depend on the machine's locale or on the current time.

The timer runs only while `Activate()`d, which is startup to shutdown — so it ticks while
the notch is closed. That is 60 no-op property writes a minute and is not worth gating; the
alternative couples the card to presentation state it otherwise never reads.

### 3.2 Battery

`GetSystemPowerStatus` (kernel32), polled on the same 1 Hz tick. No WinRT and no
`System.Windows.Forms` reference — the project uses `WpfScreenHelper` precisely to avoid
pulling WinForms in, and adding it for one struct would be a regression.

`SYSTEM_POWER_STATUS.BatteryFlag` bit 128 means "no system battery"; `BatteryLifePercent`
is 255 when unknown. Both map to "collapse the column" in the pure formatter, which is where
they are tested — a desktop and a laptop cannot both be exercised on one machine, so the
decision must be reachable without one.

### 3.3 Weather

Provider: **Open-Meteo** (`https://api.open-meteo.com/v1/forecast?...&current=temperature_2m,weather_code`).
Keyless, no signup, no account, free for non-commercial use. Chosen over any keyed provider
because a key would have to ship in the binary or be entered by the user, and neither is
acceptable for a column that is decoration.

Location, in strict priority order — this order was the user's call:

1. **Manual override** from Settings, if non-empty. Geocoded once via Open-Meteo's own
   geocoding endpoint and cached in `config.ini` as lat/lon so no lookup runs at startup.
2. **Windows Location API** (`Windows.Devices.Geolocation.Geolocator`). Tried first among
   the automatic options, deliberately: it is the accurate one, and asking for it first is
   the only way the permission prompt ever appears. If access is `Denied`, `Unspecified`,
   or the call throws, fall through — **do not retry it on a schedule**, or a user who said
   no gets asked forever.
3. **IP geolocation** as the fallback for a denied or unavailable location service.

Refresh every `WeatherRefreshMinutes` (default 15) on a background timer, not on hover — a
hover must never wait on a network call. A snapshot older than `WeatherMaxAgeMinutes`
(default 90) is treated as absent rather than shown stale.

All failures are silent to the user and logged once per transition, not once per attempt.

## §4 — Settings

| Control | Default | Notes |
|---|---|---|
| "Show ambient info on hover" toggle | on | Notch-only; the row is hidden entirely when off. Row is hidden in Settings while Classic is selected, following `StripHeightRow`'s existing rule. |
| "Weather location" text box | empty | Empty means automatic (Windows Location → IP). A city name here overrides both. |
| "Show weather" toggle | on | Off disables every network call and the location lookup outright. Present because "I want the clock and battery but nothing phoning home" is a reasonable position and the alternative is disabling the whole row. |

Persisted keys: `ShowAmbientOnHover`, `ShowWeather`, `WeatherLocation`,
`WeatherLatitude`, `WeatherLongitude` (the geocode cache). Same `[Osd]` section and the same
`ParseDouble`/clamp treatment `NotchStripHeightDip` gets.

## §5 — Accessibility

`AmbientCard.AccessibleName` = "Ambient status", and `ToString()` returns it, per the
`ICard` contract's note about `ItemAutomationPeer` naming containers from the bound item.

Each column carries its own `AutomationProperties.Name` on an element that can surface it —
`scripts/check-a11y.ps1` enforces both halves of that and is a release gate. The clock reads
as a time, not as a bare number.

## §6 — Risks

| Risk | Handling |
|---|---|
| `Geolocator` behaviour inside a UIAccess process is unmeasured | Wrapped in try/catch with a fall-through to IP; the fallback path is the tested one. Worth an explicit manual check. |
| A 1 Hz `DispatcherTimer` on a window that is never hidden | Slice 1's ledger §7 already has an idle-resource measurement open for exactly this window; the ambient timer is added to that check rather than measured separately. |
| Panel height changes when the row appears | `OnContentMeasured` re-runs on every show, and the resting shape no longer depends on the measurement (slice 1). The failure this would once have caused is gone. |
| Network call from a UIAccess process | Already true today — `UpdateCheckService` calls GitHub. No new category. |

## Deferred to slice 3 — the file shelf

Not planned yet, on purpose. The shelf needs the notch to accept an OLE drag while it is
click-through, and a window with `WS_EX_TRANSPARENT` does not receive `WM_DRAGENTER` at all.
Whether that can be worked around — and at what cost — decides the entire shape of the
feature, so slice 3 opens with a spike, not a plan.
