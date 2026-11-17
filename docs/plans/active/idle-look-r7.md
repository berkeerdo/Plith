# R7a Idle look (Karot) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The resting pill gets selectable looks (Karot, Saat, Çalan, Ses, Sade, Gizli) and a
click opens a welcome view: today's run with songs played above the axis and the hourly
weather below it, read at the pointer.

**Architecture:** Pure models live in `plith-core`: settings and their text format, the play
log, Open-Meteo parsing, pill looks and the welcome display list. A new crate `plith-weather`
fetches the forecast over WinHTTP and finds the location. The app crate stores the settings
and play log files, arms the minute clock and routes pointer moves. The welcome view is a
fifth page that has no tab.

**Tech Stack:** Rust 2024, `windows` crate (WinHTTP, `Windows.Devices.Geolocation`), no
third-party crates.

**Spec:** `docs/specs/2026-11-17-idle-look-direction.md`. Mock:
`docs/design/r7/karot.html`; the reference render is `docs/design/r7/renders-3/sheet.png`.

Branch `feature/idle-look`, worktree `.claude/worktrees/idle-look`. The bar is the five checks
(fmt, clippy `-D warnings`, tests, `structure-check.mjs`, `check-deps.mjs`), plus offscreen
renders that the master reviews against the mock. On-screen checks wait for Berke at the
console.

**Out of scope:** the settings window and the island's quick-settings picker are a separate R7
plan. Until that plan lands, looks are changed in `settings.ini` or with `--idle-look`. Light
themes, the 12-hour clock layout and snow renders come later.

## Global Constraints
- **Language:** all code and comments in English. Only UI strings are Turkish, upper-cased with
  Turkish rules (ADR 0012). Song titles and artists are foreign names and are not cased.
- **Size limits:** 500 lines per source file, 80 per function, 800 per test file. Follow
  `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`. Do not grow `plith/src/services.rs` (414
  lines) or `plith-media/src/service.rs` (497).
- **Lime (`Ink::Flow`):** used only for the interval playing now and the Çalan station. The
  time, weather and toggles use ink tones.
- **G2 at rest:** no timer while the island is suppressed. Collapsed, the only timer is the
  minute clock, and only for looks that show the time (Karot, Saat).
- **Weather is off by default.** No network call happens while it is off. Coordinates leave
  the PC only with weather on.
- **Dependency rules:** `plith-core` gets no `windows`. `plith-weather` gets `windows` and
  `plith-core` only. No third-party crates.
- **Two languages (Berke 2026-11-17: "ingilizce ve türkçe olsun bence uygulama"):** every UI
  string this plan adds comes from the text table in Task 1b, never from a literal in a page
  function. Turkish copy is upper-cased with Turkish rules, English copy with invariant rules.
  Song titles and artists are foreign names and are not cased.
- **Copy (TR / EN):**

  | Turkish | English |
  |---|---|
  | KAROT | CORE |
  | SAAT | CLOCK |
  | ÇALAN | PLAYING |
  | SES | SOUND |
  | SADE | PLAIN |
  | GİZLİ | HIDDEN |
  | BUGÜN ÇALAN YOK | NOTHING PLAYED TODAY |
  | OKUMA hh:mm | READ hh:mm |
  | ESKİ | OLD |
  | TAHMİN | FORECAST |
  | GEÇMİŞ | EARLIER |
  | DURDU | PAUSED |
  | n ŞARKI | n SONGS (1 SONG) |
  | AÇIK | CLEAR |
  | BULUTLU | CLOUDY |
  | YAĞMUR | RAIN |
  | KAR | SNOW |

  The date reads `SAL 17 KAS` in Turkish and `TUE 17 NOV` in English.
- **Commands:** builds and full test runs go through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
  Never move the mouse from a script.

## Decisions taken here (ADR 0014 at merge)
1. **A click opens the welcome view.** ADR 0011 stays as it is: there is no hover-open. The
   spec's "hover opens" becomes "click opens". `open_on = Welcome` (the default) opens on the
   welcome view every time. `LastPage` keeps today's behaviour.
2. **Hover runs the probe.** Pointer moves count only while the island is open. They redraw
   only when the probed hour changes, at most once per hour cell.
3. **The R3 seal stays as the look `Ses`.** No look is lost. The default look is `Karot`.
4. **Settings live in `%LOCALAPPDATA%\Plith\settings.ini`**, in `key=value` lines. An unknown
   key or bad value is ignored and logged, and the default is used. The play log lives in
   `plays.txt` in the same folder and holds today only.
5. **Weather sources:**
   - Forecast: `api.open-meteo.com` (hourly temperature, weather code, precipitation
     probability, `timezone=auto`), fetched every 30 min and on a settings change.
   - Location: Windows location first. When it is denied or unavailable, IP location from
     `ipapi.co/json`. With `location=City`, the Open-Meteo geocoding API resolves the city.
   - A reading older than 90 min is stale (WPF `WeatherMaxAgeMinutes`).
6. **Languages (Berke 2026-11-17: "ingilizce ve türkçe olsun", then "ya da bilindik dilleri
   ekleyebilirsin").**
   - The table is built for any number of languages, one file per language. A missing string
     falls back to English.
   - `lang=auto`, the default, matches the Windows UI language by its primary code (`tr-TR`
     gives `tr`). It falls back to English. A language code in settings overrides it.
   - This plan ships Turkish and English.
   - The follow-up plan "R7 languages" moves the R4 pages' Turkish `copy.rs` into the table
     and adds the further languages Berke picks. That plan comes after this one and before
     the settings window.
   - Bundled Public Sans and Martian Mono cover Latin scripts. Cyrillic, Greek and CJK need a
     font fallback, which must be checked before those languages are added.

## Review Focus
1. Midnight: the run spans two days. Songs from yesterday after 00:00 leave the log, and the
   axis labels wrap (`22 00 02`). Task 2 and Task 5 tests.
2. No network or a failed fetch: the last forecast stays with its old `OKUMA` and turns stale.
   The view never blanks and never shows a spinner. Task 3 and Task 8 tests.
3. Very long titles or artist names, and an empty artist: trim at the end. No overlap with the
   time or the weather head. Task 4 and Task 5 tests.
4. A track replayed, paused and resumed, or skipped in under 10 s: one interval per continuous
   play. A skip under 10 s is not logged. Task 2 tests.
5. A game goes fullscreen while the island is closed: no minute timer and no redraw until it
   ends, then one catch-up draw. Task 9 test.

---

### Task 1: Settings model and text format

**Files:**
- Create: `crates/plith-core/src/settings.rs`, `crates/plith-core/src/settings/tests.rs`
- Modify: `crates/plith-core/src/lib.rs` (add `pub mod settings;`)

**Interfaces:**
- Produces:
  - `pub enum IdleLook { Karot, Clock, Playing, Seal, Plain, Hidden }`, default `Karot`.
  - `pub enum RunSpan { Three, Six, Day }`, default `Six`.
  - `pub enum LocationMode { Windows, Ip, City }`, default `Windows`.
  - `pub enum TempUnit { C, F }`.
  - `pub enum OpenOn { Welcome, LastPage }`.
  - `pub enum HeadSlot { Auto, Weather, Sound, Playing, Empty }`, default `Auto`. `Auto` means
    Weather while weather is on and Sound otherwise. File key `head`, with values `auto`,
    `weather`, `sound`, `playing` and `empty`. Berke 2026-11-17: the top right of the welcome
    view must not stay empty.
  - `pub enum LangChoice { Auto, Code(String) }`, default `Auto`. The file key is `lang`,
    with the value `auto` or a lower-case primary language code (`tr`, `en`). An unknown code
    is kept and resolves to English.
  - `pub struct Settings { look, span, plays: bool, weather: bool, location: LocationMode, city: String, unit, clock24: bool, open_on, lang: LangChoice, head: HeadSlot }`.
    `Default` gives `plays: true`, `weather: false`, `clock24: true`, `open_on: Welcome` and
    `lang: Auto`.
  - `pub fn parse(text: &str) -> (Settings, Vec<String>)`: the second value lists problems,
    one per bad line.
  - `pub fn serialize(s: &Settings) -> String`.
  - `IdleLook::from_key(&str) -> Option<IdleLook>` and `IdleLook::key(self) -> &'static str`.
    The keys are `karot`, `clock`, `playing`, `seal`, `plain` and `hidden`, shared by the file
    and `--idle-look`.

- [ ] **Step 1: Write the failing tests** in `settings/tests.rs`:
  - `defaults_are_karot_six_hours_weather_off`.
  - `round_trip`: `parse(&serialize(&s)).0 == s` for a non-default `s` with
    `city = "İstanbul"`.
  - `unknown_key_and_bad_value_are_reported_and_ignored`: parsing
    `"look=eyes\nfoo=1\nweather=on\n"` gives the default look, `weather == true` and 2
    problems.
  - `blank_lines_and_hash_comments_are_skipped`.
  - `keys_are_trimmed_and_case_sensitive`.
- [ ] **Step 2:** `node ~/.claude/bin/heavy-run.mjs -- cargo test -p plith-core settings`.
  Expected: FAIL (module missing).
- [ ] **Step 3:** Implement. Booleans read `on` or `off`. The file keys are `look`, `span`,
  `plays`, `weather`, `location`, `city`, `unit`, `clock24` and `open_on`.
- [ ] **Step 4:** Run the same command. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): idle look settings and file format`.

### Task 1b: Languages, text table

**Files:**
- Create: `crates/plith-core/src/lang.rs` (`Lang`, `resolve`, `text` dispatch, `upper`),
  `crates/plith-core/src/lang/tr.rs`, `crates/plith-core/src/lang/en.rs` (one
  `fn text(t: Text) -> Option<&'static str>` each), `crates/plith-core/src/lang/tests.rs`

**Interfaces:**
- Consumes: `LangChoice` (Task 1).
- Produces:
  - `pub enum Lang { En, Tr }`. A new language adds a variant, a file and a `code()` arm.
  - `Lang::code(self) -> &'static str` and `Lang::ALL`.
  - `pub fn resolve(choice: &LangChoice, windows_ui: &str /* "tr-TR" */) -> Lang`.
  - `pub enum Text { LookKarot, LookClock, LookPlaying, LookSeal, LookPlain, LookHidden, NothingPlayed, Read, Old, Forecast, Earlier, Paused, SkyClear, SkyCloud, SkyRain, SkySnow }`.
  - `pub fn text(t: Text, lang: Lang) -> &'static str`.
  - `pub fn songs(n: usize, lang: Lang) -> String`: `3 ŞARKI`, `3 SONGS`, `1 SONG`.
  - `pub fn date_short(weekday: u8 /*0 = Monday*/, day: u16, month: u16, lang: Lang) -> String`:
    `SAL 17 KAS` or `TUE 17 NOV`.
  - `pub fn upper(s: &str, lang: Lang) -> String`: Turkish rules (`i` to `İ`, `ı` to `I`) or
    invariant rules.

- [ ] **Step 1: Failing tests:**
  - `every_text_has_every_language`: loop over all `Text` and `Lang::ALL`. Each string is
    non-empty, already upper-case under its own rules, and served without fallback.
  - `auto_follows_windows`: `tr-TR` gives Tr, `de-DE` gives En (fallback), and
    `Code("en")` with `tr-TR` gives En.
  - `songs_plural_english`.
  - `dates_both_languages`.
  - `upper_turkish_dotted_i`: `upper("çalan şarkı", Tr) == "ÇALAN ŞARKI"` and
    `upper("display", En) == "DISPLAY"`.
- [ ] **Step 2:** Run `cargo test -p plith-core lang`. Expected: FAIL.
- [ ] **Step 3:** Implement, with one `match` per language in `text`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): two-language text table`.

From here on, every page function this plan adds takes `lang: Lang`. `IdleFacts` and
`WelcomeFacts` carry it. `sky_word(Sky)` in Task 3 becomes `text(Text::Sky*, lang)`, so
Task 3 drops `sky_word`. The app (Task 9) reads the Windows UI language name with
`GetUserDefaultLocaleName` or `GetUserPreferredUILanguages` (check with context7), and
resolves `Lang` once at start and on a settings change. Each task's tests run the Turkish case and at least one English
case.

### Task 2: Play log

**Files:**
- Create: `crates/plith-core/src/plays.rs`, `crates/plith-core/src/plays/tests.rs`

**Interfaces:**
- Produces:
  - `pub struct Play { start_min: u32, end_min: u32, title: String, artist: String }`.
    Minutes count from local midnight of `day`, so an interval that passes midnight is split.
  - `pub struct PlayLog { day: (u16, u16, u16), plays: Vec<Play>, open: Option<...> }`.
  - `PlayLog::observe(&mut self, day: (u16,u16,u16), minute_of_day: u32, second: u32, track: Option<(&str, &str)>, playing: bool)`.
  - `PlayLog::current(&self) -> Option<&Play>`: the open interval, if one is playing.
  - `PlayLog::in_window(&self, from_min: i32, to_min: i32) -> Vec<&Play>`.
  - `serialize(&self) -> String` and `parse(text: &str) -> PlayLog`. Each line reads
    `YYYY-MM-DD start end title<TAB>artist`.
  - `pub const MIN_PLAY_S: u32 = 10`.

- [ ] **Step 1: Failing tests:**
  - `a_track_played_then_paused_is_one_interval`.
  - `pause_and_resume_same_track_within_a_minute_merges`.
  - `skip_under_ten_seconds_is_dropped`.
  - `track_change_closes_and_opens`.
  - `day_change_clears_yesterday`: observing on a new `day` keeps only an open interval,
    restarted at minute 0.
  - `round_trip_with_tab_in_title_is_sanitised`: a tab in a title becomes a space.
  - `in_window_handles_negative_from` (a run that starts before midnight).
- [ ] **Step 2:** Run `cargo test -p plith-core plays`. Expected: FAIL.
- [ ] **Step 3:** Implement. Store seconds internally so that `MIN_PLAY_S` holds.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): play log for the welcome run`.

### Task 3: Weather model and Open-Meteo parsing

**Files:**
- Create: `crates/plith-core/src/weather.rs`, `crates/plith-core/src/weather/json.rs`
  (minimal JSON reader: objects, arrays, numbers, strings, null), and
  `crates/plith-core/src/weather/tests.rs` with fixture strings inline.

**Interfaces:**
- Produces:
  - `pub enum Sky { Clear, Cloud, Rain, Snow }`.
  - `pub fn sky_of(wmo: u16) -> Sky`. Codes 0 to 1 are Clear. 2, 3, 45 and 48 are Cloud.
    51 to 67, 80 to 82 and 95 to 99 are Rain. 71 to 77, 85 and 86 are Snow. Any other code is
    Cloud.
  - Sky words come from `lang::text` (Task 1b), not from this module.
  - `pub struct Hour { day: (u16,u16,u16), hour: u8, sky: Sky, temp_c: f32, rain_pct: Option<u8> }`.
  - `pub struct Forecast { hours: Vec<Hour>, read_at: (u16,u16,u16,u8,u8) }`.
  - `pub fn parse_forecast(body: &str, read_at: ...) -> Result<Forecast, String>`. It reads
    `hourly.time` (`"2026-11-17T14:00"`), `temperature_2m`, `weather_code` and
    `precipitation_probability`. The arrays must have equal lengths.
  - `pub fn parse_ip_location(body: &str) -> Option<(f64, f64)>` reads `latitude` and
    `longitude`.
  - `pub fn parse_geocode(body: &str) -> Option<(f64, f64, String)>` reads the first entry of
    `results`.
  - `pub fn next_turn(f: &Forecast, now: (day, hour), within_h: u8) -> Option<(u8 /*hour*/, Sky)>`:
    the first hour whose sky differs from the current hour's sky.
  - `pub fn is_stale(read_at, now) -> bool`: true past 90 min.
  - `pub fn temp_text(c: f32, unit: TempUnit) -> String`: `9°`, rounded.

- [ ] **Step 1: Failing tests:**
  - `wmo_codes_map_to_four_skies`, covering every code boundary.
  - `parses_a_trimmed_open_meteo_body`, using a 6-hour fixture.
  - `mismatched_array_lengths_is_an_error`.
  - `null_precipitation_is_none`.
  - `next_turn_finds_rain_at_15`: cloud through 14 and rain at 15 gives `(15, Rain)`.
  - `next_turn_none_when_sky_holds`.
  - `stale_after_ninety_minutes`, also across midnight.
  - `ip_and_geocode_bodies_parse`.
  - `fahrenheit_text`.
- [ ] **Step 2:** Run `cargo test -p plith-core weather`. Expected: FAIL.
- [ ] **Step 3:** Implement. The JSON reader is a small recursive descent over `&str`
  returning `enum Json { Null, Bool, Num(f64), Str(String), Arr(Vec), Obj(Vec<(String, Json)>) }`.
  It handles `\uXXXX` escapes, so `İstanbul` survives.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): forecast model and Open-Meteo parse`.

### Task 4: Pill looks

**Files:**
- Create: `crates/plith-core/src/pages/idle_pill.rs`, `crates/plith-core/src/pages/idle_pill/tests.rs`
- Modify: `crates/plith-core/src/shapes.rs` (add
  `REST_HIDDEN: IslandShape` = 200 x 3, radius 0), `crates/plith-core/src/pages/copy.rs`
  (new copy).

**Interfaces:**
- Consumes: `IdleLook` (Task 1), `Lang` and `text` (Task 1b), `Sky` and `temp_text`
  (Task 3), and the existing `pill(&PillFacts)` for `Seal`.
- Produces:
  - `pub struct IdleFacts { time: String, turn: Option<String>, now_sky: Option<String>, playing: Option<(String, String)>, seal: PillFacts }`.
    `turn` is preformatted, for example `YAĞMUR 15:30`. `now_sky` looks like `9° BULUTLU`.
  - `pub fn idle_pill(look: IdleLook, f: &IdleFacts) -> DisplayList`.
  - `pub fn rest_shape(look: IdleLook) -> IslandShape`: `REST_HIDDEN` for Hidden, `REST`
    otherwise.

- [ ] **Step 1: Failing tests.** Positions come from the mock's `pill()`:
  - `karot_time_left_turn_right`: time `Clock` at x 16, turn `Value` aligned End with its right
    edge at 184.
  - `karot_falls_back_to_now_sky_then_nothing`.
  - `clock_is_centered`.
  - `playing_has_one_flow_station_and_trimmed_text`: one `Ink::Flow` node, the text has
    `Trim::End`, and its rect ends at or before 184.
  - `playing_without_track_shows_time`.
  - `seal_equals_pill`.
  - `plain_and_hidden_have_no_text`.
  - `no_node_uses_flow_except_playing_station`.
- [ ] **Step 2:** Run `cargo test -p plith-core idle_pill`. Expected: FAIL.
- [ ] **Step 3:** Implement with the `kit` helpers (`text`, `text_trim`, `station`, `rect`).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): resting pill looks`.

### Task 5: Welcome view

**Files:**
- Create: `crates/plith-core/src/pages/welcome.rs` (page and head),
  `crates/plith-core/src/pages/welcome/run.rs` (axis, depth blocks, song band, weather band,
  hatches), `crates/plith-core/src/pages/welcome/tests.rs`
- Modify: `crates/plith-core/src/draw.rs` (`TextStyle::Hero`: Mono, 30 DIP, 400, not upper,
  `MONO_TRACKING`), `crates/plith-core/src/shapes.rs` (`WELCOME`: 620 x 232, radius 24).

**Interfaces:**
- Consumes: `PlayLog` (Task 2), `Forecast`, `Sky`, `is_stale` and `temp_text` (Task 3),
  `RunSpan` and `TempUnit` (Task 1), `Lang`, `text`, `songs` and `date_short` (Task 1b), and
  `tabbar`.
- Produces:
  - `pub struct WelcomeFacts { now: (day, minute_of_day), date: String, plays: Vec<Play>, playing: bool, forecast: Option<Forecast>, weather_on: bool, span: RunSpan, unit: TempUnit, head: HeadSlot, sound: Option<SoundHead>, elapsed: Option<(u32, u32) /*s, s*/>, lang: Lang }`.
  - `pub struct SoundHead { output: String, kind: String /*KULAKLIK*/, value: Option<u8>, muted: bool, flowing: bool }`.
  - `pub fn welcome_page(f: &WelcomeFacts, probe: Option<i32 /*hour*/>) -> PageBuild`.
  - `pub fn probe_hour(f: &WelcomeFacts, x: f64, y: f64) -> Option<i32>`: the hour cell under
    the pointer when it is inside the run band (y 126 to 172). `None` otherwise.
- Geometry, from the mock's `welcome()`:
  - The run spans x 30 to 590, axis y 150. `Six` covers now ±6 h, `Three` ±3 h, `Day` 00 to 24.
  - The song band is y 136, height 8. The playing interval is `Flow`, the others `Muted`, with
    a 1 DIP gap between intervals.
  - The weather band is y 156.5, height 14. Logged sections get a `FloorHigh` fill and a solid
    `Track` outline. Forecast sections get a dashed outline and `Faint` hatches.
  - Hatches: cloud is 4 x 1 dashes on a 6 x 4 grid. Rain is 1 x 1 dots at (1,1) and (4,3) on a
    6 x 5 grid. Snow is crosses on an 8 x 8 grid.
  - Depth ticks every hour, ±3 DIP. Hour labels every 2 h at y 202, `Label`, `Faint`, centered.
  - The bit is a vertical `Ink` line from axis −18 to axis +22 with a filled `Ink` station.
  - Temperatures sit at now and at each sky turn: `Value` at axis +34, `Ink` at now and
    `Faint` elsewhere.
  - Head: time `Hero` at (30, 92). The date is `Label` at (32, 110), as in `SAL 17 KAS`. The
    three right-aligned lines end at x 590, at y 78, 96 and 112. At rest they read now (sky,
    next turn, `OKUMA hh:mm` or `OKUMA hh:mm · ESKİ`). On a past probe they read title · artist,
    `hh:00 – hh:00 · n ŞARKI` and `GEÇMİŞ`. On a future probe they read sky · temp · rain %,
    `hh:00 · TAHMİN` and `OKUMA hh:mm`.
  - **Head slot at rest:**
    - `Weather`: the three lines above.
    - `Sound`: the output name (`Name`) with a filled station at its right, `Flow` when sound
      flows and `Ink` otherwise. Below it `%62 · KULAKLIK`, or `KAPALI` when muted, then
      `VARSAYILAN ÇIKIŞ`.
    - `Playing`: the title (`Name`), the artist (`Label`) and `1:12 / 2:05` (`Value`, `Faint`).
      In this slot the run's song label is hidden.
    - `Empty`: nothing.
    - A probe always overrides the slot.
  - Label of the playing song: the title (`Body`) and above it the artist (`Label`, with
    `· DURDU` when paused), right-aligned at the interval's start.
  - Empty states: no plays gives `BUGÜN ÇALAN YOK` in the song band. Weather off or no
    forecast gives no weather band, no head lines and a solid axis to the bit with dashes
    after it.
  - The tab bar has no active station.

- [ ] **Step 1: Failing tests:**
  - `bit_sits_at_now`.
  - `only_the_playing_interval_is_flow`.
  - `paused_has_no_flow_and_says_durdu`.
  - `forecast_sections_are_dashed_logged_are_solid`.
  - `head_reads_probe_past_and_future`.
  - `stale_reading_says_eski`.
  - `weather_off_has_no_weather_nodes`.
  - `no_plays_says_bugun_calan_yok`.
  - `run_across_midnight_labels_wrap`: now 23:10 with `Six` gives labels `18 20 22 00 02 04`.
  - `long_title_is_trimmed_inside_the_head`.
  - `probe_hour_outside_band_is_none`.
  - `auto_head_is_sound_when_weather_off`.
  - `sound_head_station_is_flow_only_when_flowing`.
  - `playing_head_hides_run_song_label`.
  - `probe_overrides_any_head_slot`.
  - `no_node_outside_620x232`.
- [ ] **Step 2:** Run `cargo test -p plith-core welcome`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `welcome.rs` and `run.rs` under 300 lines each.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(core): welcome view on the Karot run`.

### Task 6: Fifth page without a tab

**Files:**
- Modify: `crates/plith-core/src/pages.rs` (`PageId::Welcome` index 4, `PAGE_COUNT = 5`,
  `PageId::ALL: [PageId; 5]`, `PageId::TABS: [PageId; 4]`), `scene.rs`
  (`shapes: [IslandShape; PAGE_COUNT]`, new `rest: IslandShape` used by `target()` when
  collapsed), `shapes.rs` (`PAGES` gains `WELCOME`), `draw.rs` (`PageContent.pages:
  [_; PAGE_COUNT]`), `pages/tabbar.rs` (loop over `TABS`, where `active: Option<PageId>`), and
  every `[_; 4]` page array in `plith-core` and `crates/plith` (`grep -rn "; 4\]" crates`).
- Test: the existing scene, page and tabbar tests, plus new ones in `scene/tests.rs` and
  `pages/tests.rs`.

**Interfaces:**
- Produces: `PageId::Welcome`, `PAGE_COUNT`, `PageId::TABS`, `Scene.rest`, and
  `tabbar(active: Option<PageId>, clock: &str)`.

- [ ] **Step 1: Failing tests:**
  - `collapsed_target_is_scene_rest`, with Hidden giving 200 x 3.
  - `welcome_shape_is_620x232`.
  - `tabbar_on_welcome_has_no_filled_station`.
  - `page_turn_welcome_to_media_and_back`.
- [ ] **Step 2:** Run `cargo test -p plith-core`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement. A tab click from the welcome view turns to that page like any tab.
  Dirty flags and target arrays grow to `PAGE_COUNT`.
- [ ] **Step 4:** Run `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace`. Expected:
  PASS.
- [ ] **Step 5:** Commit `refactor(core): welcome as a page without a tab`.

### Task 7: Pointer moves for the probe

**Files:**
- Modify: `crates/plith-shell/src/window/mouse.rs` (on `WM_MOUSEMOVE` inside the drawn shape,
  emit `ShellEvent::Move { x, y }` in island DIP, only while the window reports open),
  `crates/plith-shell/src/window.rs` (the event variant), `crates/plith/src/app/input.rs`
  (route `Move` to `probe_hour`, store `probe: Option<i32>`, mark the welcome page dirty only
  when the value changes; `Leave` clears it).
- Test: `crates/plith/src/app/state/tests.rs` or a new `app/input/tests.rs` if that file
  passes 700 lines.

**Interfaces:**
- Consumes: `probe_hour` (Task 5).
- Produces: `ShellEvent::Move { x: f64, y: f64 }`.

- [ ] **Step 1: Failing tests:**
  - `moves_within_one_hour_cell_rebuild_once`.
  - `leave_clears_probe`.
  - `moves_while_collapsed_are_ignored`.
- [ ] **Step 2:** Run `cargo test -p plith input`. Expected: FAIL.
- [ ] **Step 3:** Implement. Do not add a hover-open or a timer.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(shell): pointer moves for the welcome probe`.

### Task 8: `plith-weather` crate

**Files:**
- Create: `crates/plith-weather/Cargo.toml` (windows features `Win32_Networking_WinHttp` and
  `Devices_Geolocation`; check the API with context7 first), `src/lib.rs`, `src/http.rs`
  (`get(url) -> Result<String, String>` over WinHTTP, HTTPS only, 10 s timeout, User-Agent
  `Plith`), `src/location.rs` (Windows `Geolocator` with access check, then IP, or the city
  via geocoding), `src/service.rs` (`WeatherService`), and `examples/probe.rs` (fixed
  coordinates 41.01, 28.98, prints the parsed forecast).
- Modify: the workspace `Cargo.toml` members, `scripts/check-deps.mjs` (`'plith-weather':
  { plith: ['plith-core'], windows: true }`, since parsing lives in `plith-core`), `scripts/check-deps.test.mjs`.

**Interfaces:**
- Consumes: `parse_forecast`, `parse_ip_location` and `parse_geocode` (Task 3), and
  `LocationMode` (Task 1).
- Produces:
  - `WeatherService::start(config: WeatherConfig) -> WeatherService`, where
    `WeatherConfig { location: LocationMode, city: String }`.
  - `snapshot() -> Option<Forecast>`, `subscribe() -> Receiver<Forecast>`,
    `set_waker(Box<dyn Fn() + Send>)`, `reconfigure(WeatherConfig)` and `stop()`.
  - The worker fetches at start and every 30 min, and at once after `reconfigure`. A failure
    keeps the last forecast. It never blocks the caller.

- [ ] **Step 1: Failing tests** (no network):
  - `forecast_url_has_hourly_fields_and_timezone_auto`.
  - `geocode_url_escapes_city`: `İstanbul` is percent-encoded UTF-8.
  - `failed_fetch_keeps_last_forecast`, using a fake `Fetch` trait object.
  - `reconfigure_triggers_one_fetch`.
- [ ] **Step 2:** Run `cargo test -p plith-weather`. Expected: FAIL.
- [ ] **Step 3:** Implement. URL building and the fetch loop sit behind `trait Fetch`, so tests
  use a fake. WinHTTP stays in `http.rs` only.
- [ ] **Step 4:** Run the tests and `node scripts/check-deps.mjs`. Expected: PASS and no
  violations.
- [ ] **Step 5:** Run `cargo run -p plith-weather --example probe` (a network GET with fixed
  coordinates, not Berke's location). Expected: 24 or more hours printed.
- [ ] **Step 6:** Commit `feat(weather): Open-Meteo service over WinHTTP`.

### Task 9: App wiring

**Files:**
- Create: `crates/plith/src/settings_file.rs` (load and save `settings.ini`, logging
  problems), `crates/plith/src/plays_file.rs`, `crates/plith/src/app/idle_facts.rs` (builds
  `IdleFacts` and `WelcomeFacts` from services, log and settings).
- Modify:
  - `crates/plith/src/app/feed.rs`: `wanted()` arms `CLOCK_TIMER` when the island is open, or
    when it is collapsed with a time look (`Karot` or `Clock`) and not suppressed.
  - `crates/plith/src/pages.rs`: pill and welcome rebuild through `Dirty`.
  - `crates/plith/src/app/input.rs`: an expand with `open_on = Welcome` targets
    `PageId::Welcome`.
  - `crates/plith-core/src/levers.rs`: `--idle-look <key>`.
  - `crates/plith/src/services/fixture.rs`: the banana fixture gains the mock's five plays and
    its forecast. Move the fixture data to `services/fixture/idle.rs` so `fixture.rs` stays
    under 450 lines.
  - Media subscription: feed `PlayLog::observe` on every snapshot and on the minute tick.
    Save `plays.txt` on a track change and at exit.
  - Start `WeatherService` only with `weather = on`, never in fixture or idle-check runs.

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Failing tests:**
  - `clock_timer_armed_collapsed_for_karot_not_for_plain`.
  - `no_clock_timer_while_suppressed`.
  - `unsuppress_draws_once`.
  - `expand_lands_on_welcome_by_default_and_last_page_when_set`.
  - `weather_off_starts_no_weather_service`.
  - `missing_settings_file_gives_defaults_and_writes_one`.
- [ ] **Step 2:** Run `cargo test -p plith`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace`. Expected:
  PASS.
- [ ] **Step 5:** Commit `feat(app): wire idle looks, welcome and weather`.

### Task 10: Idle check allowance and renders

**Files:**
- Modify: `crates/plith/src/idle.rs`. A pass now means
  `wakes <= allowed(look, secs)`, where `allowed` is `secs / 60 + 1` for time looks and `0`
  otherwise. The log line names the look.
- Modify: the offscreen `render_pages` example, to add the welcome states (rest, past probe,
  future probe, stale and paused, weather off, no plays) and the six pill looks with the
  banana fixture.

- [ ] **Step 1: Failing tests:**
  - `allowance_zero_for_plain`.
  - `allowance_two_for_sixty_seconds_karot`.
- [ ] **Step 2:** Run `cargo test -p plith idle`. Expected: FAIL.
- [ ] **Step 3:** Implement and run the renders into `docs/design/r7/engine-1/`.
- [ ] **Step 4: Master:** compare the renders with `renders-3/sheet.png`, then run the five
  checks (fmt, clippy `-D warnings`, `cargo test --workspace`, `structure-check.mjs`,
  `check-deps.mjs`), all through `heavy-run`.
- [ ] **Step 5:** Commit `test(app): idle allowance and Karot renders`.

### Task 11: Merge review and records
- [ ] Code review (`superpowers:requesting-code-review`). Fix every Important finding.
- [ ] Master writes ADR 0014 from "Decisions taken here", updates STATUS, and asks Berke
  before the merge commit.

### Task 12: Console (Berke)
- [ ] `--idle-check 120 --idle-look karot`: wakes at most 3.
- [ ] Click opens the welcome view, the probe follows the pointer, a tab turns the page.
- [ ] The pill looks on screen. With his yes, turn weather on: a real fetch reaches
  `OKUMA`.

## Progress log
(empty)
