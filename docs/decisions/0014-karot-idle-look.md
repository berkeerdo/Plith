# 0014 Karot idle look: click opens the welcome view, settings and weather sources

Date: 2026-11-17. Status: accepted. Direction picked by Berke (spec
`specs/2026-11-17-idle-look-direction.md`); the points below were decided in the plan
`plans/done/idle-look-r7.md` and during its execution by the master.

## Context
R7a gives the collapsed island an idle look and the open island a welcome view: one Karot run
of the day (songs above the axis, weather as lithology hatches below, the bit at now). The spec
said "hover opens"; ADR 0011 says click to expand, leave to collapse. The work also needed a
settings file, a play log and a weather source, none of which existed in the Rust app.

## Decision
1. **A click opens the welcome view.** ADR 0011 stands: no hover-open. `open_on = Welcome`
   (default) opens on the welcome view every time; `LastPage` keeps the old behaviour. Demo
   `Expand` steps ignore `open_on` so measured demos stay comparable.
2. **Hover runs the probe.** The shell reports pointer moves inside the shape; the app uses
   them only while open and on the welcome page, and redraws only when the probed hour cell
   changes. Leave and collapse clear the probe.
3. **Looks:** Karot (default), Clock, Playing, Ses (the R3 seal, kept), Plain, Hidden. Only the
   time looks (Karot, Clock) arm the minute clock while collapsed, never while suppressed; the
   idle check allows `secs / 60 + 1` wakes for them and 0 for the others.
4. **Files in `%LOCALAPPDATA%\Plith\`:** `settings.ini` (`key=value`; unknown key or bad value
   is ignored and logged by key and length, never the raw value; a leading BOM is stripped; a
   missing file gives defaults and writes one) and `plays.txt` (today only, written atomically
   on a track change and at exit; a play under 10 s is never written). `plays=off` turns the
   history off entirely: no observe, no file write, no song band.
5. **Weather, off by default; no network call while off.**
   - Forecast: `api.open-meteo.com` hourly temperature, weather code and precipitation
     probability, `timezone=auto`, coordinates at 2 decimals (about 1 km). Every 30 min, at
     once on a settings change; a failed fetch keeps the last forecast and retries after 2, 5
     and 10 min, logging the failure without coordinates, city, IP or query.
   - Location: Windows location after an access check; when denied or unavailable, IP location
     from `ipapi.co/json`; with `location=City`, Open-Meteo geocoding. The resolved location is
     cached per configuration, not looked up on every fetch, so a laptop that moves keeps its
     old location until a restart or a settings change.
   - A reading older than 90 min is stale (`OKUMA hh:mm · ESKİ`).
   - The service runs on its own worker over WinHTTP (`plith-weather`, HTTPS only, 10 s
     timeouts); the app never stops it on the UI thread, it only drops it.
6. **Languages.** A text table with one file per language, English fallback. `lang=auto`
   follows the Windows UI language (not the regional format). This plan ships Turkish and
   English; the "R7 languages" plan moves the R4 pages' `copy.rs` into the table, localizes
   the percent format and adds more languages before the settings window.
7. **Song titles and artists are never cased**, also where the plan named the upper-casing
   `Label` style: artists use `Value` (same metrics, no casing). The mock's `APHEX TWIN`
   becomes `Aphex Twin`.
8. **The Playing head shows title and artist only.** The elapsed `1:12 / 2:05` line was
   dropped: drawn from a snapshot it froze between minute ticks, and the live media node does
   not fit the one-string head. It returns if the head gets a live node.

## Consequences
- The welcome view is a fifth page without a tab (`PageId::Welcome`, 620 x 232); the resting
  shape follows the look (`Scene.rest`), Hidden is 200 x 3.
- While suppressed, media and forecast wakes do not publish the pill; unsuppress publishes once.
- Unverified until the console session: on-screen motion and wakes of a real
  `--idle-check 120 --idle-look karot`, the probe with a real pointer, the Windows location
  path (an unpackaged app may always be denied, then IP is the real path), a real weather fetch
  with Berke's yes, and `lang=auto` on a machine whose UI language differs from its region.
- No settings window yet: settings change by editing `settings.ini` and restarting;
  `WeatherService::reconfigure` has no caller until the window exists.
