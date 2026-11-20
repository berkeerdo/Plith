# R7 Settings Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A settings window (Genel, Boşta, OSD, Hakkında) and a quick layer in the island, every
change applied live and saved at once.

**Architecture:** Pure sheet model (layout, hit test, focus, text field, change to effect
mapping) in `plith-core`. The window is a second top-level HWND in `plith-shell` with its own
class and keyboard input. It is drawn by a **dedicated sheet render thread** in `plith-render`
that lives only while the window is open, so the island's render loop and pacing are untouched.
The island gets `PageId::Quick` behind a gear mark. The UI thread owns the one `Settings` and
applies each change through setters on the feed, the weather link and a new OSD slot.

**Tech Stack:** Rust 2024 workspace, `windows` crate (Win32, D3D11, Direct2D, DirectWrite,
DirectComposition, DWM).

**Spec:** `docs/specs/2026-11-20-r7-settings-window-design.md`

## Global Constraints
- Branch `feature/settings-window` in worktree `.claude/worktrees/settings`, base `main` @ the
  commit that adds this plan.
- Limits: 500 source, 800 test, 80 per function; `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`.
  Tests live in `foo/tests.rs`.
- Files near the limit do not grow by more than a few lines: `plith-shell/src/window.rs` (435),
  `plith/src/app/feed.rs` (390), `plith-render/src/list.rs` (422), `plith-render/src/text.rs`
  (422), `plith-core/src/draw.rs` (374). New code goes in new files.
- Pure logic in `plith-core`, no Windows code there. The UI thread never draws.
- Every commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  focused `cargo test -p <crate>`, `node scripts/structure-check.mjs`; builds and test runs
  through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- English code and docs, Conventional Commits (subject max 50, bullet body), no em dash, no AI
  mention. Only user-facing strings are in another language.
- No script moves the mouse. Never call `bash` from PowerShell. Delete only literal paths.
- Spec values: window client 840 x 640 DIP, not resizable; sidebar 156 DIP; heading 20 DIP; city
  field max 64 chars; preview of the pill at 200 x 32 scaled down; gear 16 x 16 DIP; new keys
  `osd=on|off` (default on) and `flyout=off|hide` (default off); `--flyout` wins over the file.
- Lime (`Ink::Flow`) only for "sound flows now". Toggles in ink: filled station = on.

## Review Focus
1. Two switches of the OSD toggle in quick succession (off, on before the old helper stopped)
   must end with exactly one running helper and no 1.5 s UI stall. Test in Task 9.
2. Typing in the city field must not start a fetch or a save per key; Esc while the field has
   focus leaves the field (restoring the saved city), it does not close the window. Task 5.
3. A language switch while the island is expanded on a page must redraw that page in the new
   language without collapsing, and the OSD word follows on the next level. Task 9.
4. Opening the window from the island click (inside the app's borrowed event handler) must not
   re-enter the `App` borrow; window messages during `CreateWindowExW` are queued. Task 10.
5. Closing the window while a save is in flight, or quitting the app with the window open, must
   still write the last settings and join the sheet render thread. Task 10.

---

### Task 1: New keys, change mapping, atomic save

**Files:**
- Modify: `crates/plith-core/src/settings.rs` (fields, `KEYS`, `apply`, `serialize`)
- Create: `crates/plith-core/src/settings/change.rs`
- Modify: `crates/plith-core/src/launch.rs` (`Launch.flyout: Option<FlyoutMode>`)
- Modify: `crates/plith/src/settings_file.rs` (`save_at` atomic), callers of `launch.flyout`
  (`crates/plith/src/app.rs:291`)
- Test: `crates/plith-core/src/settings/tests.rs`, `settings/change/tests.rs`,
  `crates/plith/src/settings_file/tests.rs`, `crates/plith-core/src/launch/tests.rs`

**Interfaces:**
- Produces: `Settings.osd: bool` (default `true`), `Settings.flyout: FlyoutPref` with
  `enum FlyoutPref { #[default] Off, Hide }` (keys `off`/`hide`).
- Produces in `settings/change.rs`:
  `pub enum SettingChange { Look(IdleLook), Span(RunSpan), Head(HeadSlot), Plays(bool),
  Weather(bool), Location(LocationMode), City(String), Unit(TempUnit), OpenOn(OpenOn),
  Lang(LangChoice), Osd(bool), Flyout(FlyoutPref) }`
  `pub struct Effects { pub pill: bool, pub welcome: bool, pub weather: bool, pub lang: bool,
  pub osd: bool }` (derive `Default, PartialEq, Debug`)
  `pub fn apply_change(settings: &mut Settings, change: SettingChange) -> Effects` (returns
  `Effects::default()` when the value did not change).
- Produces in `launch.rs`: `pub fn flyout_mode(cli: Option<FlyoutMode>, pref: FlyoutPref) ->
  FlyoutMode` (cli wins; `Off`→`Off`, `Hide`→`Hide`).
- Produces in `settings_file.rs`: `save_at` writes `settings.ini.tmp` then `fs::rename`.

- [x] **Step 1: Write the failing tests.**
  `osd_and_flyout_round_trip`: `parse("osd=off\nflyout=hide\n")` gives `osd == false`,
  `flyout == FlyoutPref::Hide`, no warnings; `serialize` of it contains `osd=off` and
  `flyout=hide`; defaults serialize `osd=on`, `flyout=off`.
  `apply_change_effects` (table test): `Look(Clock)` → `pill`; `Span`, `Head`, `Plays`, `Unit`,
  `OpenOn` → `welcome` (Unit also `pill`); `Weather`, `Location`, `City` → `weather`; `Lang` →
  `lang`; `Osd`, `Flyout` → `osd`; applying the current value → `Effects::default()`.
  `cli_flyout_wins`: `flyout_mode(Some(DryRun), Hide) == DryRun`; `flyout_mode(None, Hide) == Hide`.
  `save_is_atomic`: after `save_at`, no `settings.ini.tmp` remains and `load_at` returns the
  saved value; a second `save_at` over an existing file succeeds.
- [x] **Step 2: Run them, expect compile failures.**
  `node ~/.claude/bin/heavy-run.mjs -- cargo test -p plith-core settings launch` and `-p plith settings_file`.
- [x] **Step 3: Implement** the fields, `FlyoutPref`, `change.rs`, `flyout_mode`, atomic save;
  `app.rs` passes `flyout_mode(launch.flyout, settings.flyout)` to `OsdSide::start`.
- [x] **Step 4: Run the tests and the workspace checks, expect PASS.**
- [x] **Step 5: Commit** `feat(core): add osd keys and change effects`.

### Task 2: Sheet strings in eight languages

**Files:**
- Create: `crates/plith-core/src/lang/sheet.rs`, `crates/plith-core/src/lang/sheet/{tr,en,de,es,fr,pt,it,pl}.rs`
- Modify: `crates/plith-core/src/lang.rs` (one `pub mod sheet;` line), `crates/plith-core/src/lang/fit.rs` or a new `lang/sheet/fit.rs` (budgets)
- Test: `crates/plith-core/src/lang/sheet/tests.rs`, `crates/plith-render/tests/fit.rs`

**Interfaces:**
- Produces: `pub enum SheetText { .. }` with `pub const ALL: [SheetText; N]` and
  `pub fn sheet_text(t: SheetText, lang: Lang) -> &'static str`; `pub fn lang_name(lang: Lang)
  -> &'static str` (each language named in itself: Türkçe, English, Deutsch, Español, Français,
  Português, Italiano, Polski).
- Turkish copy (exact; other languages translated with the same tone, captions upper case):
  window title `Plith · Ayarlar`; caption `AYARLAR`; sections `Genel`, `Boşta`, `OSD`, `Hakkında`;
  section lines: Genel `Dil ve adanın nasıl açılacağı.`, Boşta `Ada kapalıyken ne gösterir,
  tıklayınca ne okursun.`, OSD `Ses ve parlaklık göstergesi, oyunların üstünde.`, Hakkında
  `Sürüm ve dosyalar.`; rows (label / sub): `Dil` / `Otomatik, Windows dilini izler`,
  `Ada açılınca` / `Tıklayınca ilk görünen`, `KAPALI ADA` (caption), preview note `Önizleme
  küçültülmüş; ada üstündeki hali 200 x 32.`, `Şerit` / `Karşılamada kaç saat görünür`,
  `Sağ üst` / `Karşılamanın sağ üstünde ne durur`, `Bugün çalanlar` / `Şeridin üstünde, yalnızca
  bu bilgisayarda tutulur`, `Hava` / `Açıkken konum Open-Meteo'ya gider`, `Konum` / `Windows
  konumu, olmazsa IP, ya da elle`, `Birim`, `Ekran göstergesi` / `Ses ve parlaklık değişince,
  oyunların üstünde`, `Windows göstergesini gizle` / `Plith'in göstergesi çalışırken Windows'un
  kendi göstergesini gizler`, `Sürüm`, `Ayar dosyası`, `Veri` / `Hava Open-Meteo'dan, IP konumu
  ipapi.co'dan (yalnızca seçilirse)`; words `OTOMATİK`, `KARŞILAMA`, `SON SAYFA`, `±3 SA`,
  `±6 SA`, `TÜM GÜN`, `HAVA`, `SES`, `ÇALAN`, `BOŞ`, `WINDOWS`, `IP`, `ŞEHİR`, `°C`, `°F`,
  `AÇIK`, `KAPALI`, `AÇ`, `TÜM AYARLAR`; save error `Ayarlar kaydedilemedi, bu oturumda geçerli.`
  Look names reuse `Text::Look*`.

- [x] **Step 1: Write the failing tests:** `every_sheet_text_in_every_language` (no empty
  string for any `(SheetText::ALL, Lang::ALL)` pair); `turkish_copy_exact` (spot-check five
  strings above verbatim); `captions_are_upper` (each caption word equals `upper(word, lang)`).
  Fit: German row labels fit 300 DIP, choice words fit their cell (budgets in the fit test).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement** the enum and the eight tables.
- [x] **Step 4: Run tests and checks, expect PASS.**
- [x] **Step 5: Commit** `feat(core): add settings sheet strings`.

### Task 3: Inset node (a scaled display list inside another)

**Files:**
- Modify: `crates/plith-core/src/draw.rs` (one variant)
- Create: `crates/plith-render/src/list/inset.rs` (drawing with a transform)
- Modify: `crates/plith-render/src/list.rs` (one match arm calling `inset::draw`)
- Test: `crates/plith-render/tests/paint.rs` (offscreen)

**Interfaces:**
- Produces: `Node::Inset { list: Arc<DisplayList>, x: f64, y: f64, scale: f64 }`: draws `list`
  with its origin at `(x, y)` scaled by `scale`, composing with the current transform and
  restoring it afterwards; `Live` nodes inside are skipped as today; art ids are collected.

- [x] **Step 1: Write the failing test** `inset_draws_scaled`: an offscreen frame whose list
  holds `Node::Inset` of a 200 x 32 list with one `Fill` of `Ink::Ink` over its whole size, at
  `(10, 10)`, scale `0.5`: the pixel at DIP `(59, 25)` is ink, `(111, 25)` is floor.
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement** with `ID2D1DeviceContext::GetTransform`/`SetTransform`
  (`Matrix3x2::scale * translation * current`).
- [x] **Step 4: Run, expect PASS.**
- [x] **Step 5: Commit** `feat(render): draw a scaled inset list`.

### Task 4: Sheet model: sections, layout, hit test

**Files:**
- Create: `crates/plith-core/src/sheet.rs`, `sheet/{layout.rs,general.rs,idle.rs,osd.rs,about.rs,controls.rs}`
- Test: `crates/plith-core/src/sheet/tests.rs` (split per section if it nears 800)

**Interfaces:**
- Produces: `pub enum Section { General, Idle, Osd, About }` (`Section::ALL`, default `Idle`).
- Produces: `pub struct SheetFacts { pub settings: Settings, pub lang: Lang, pub previews:
  [Arc<DisplayList>; 6], pub version: &'static str, pub settings_path: String,
  pub save_failed: bool }` (previews in `IdleLook` order Karot, Clock, Playing, Seal, Plain,
  Hidden, each the real `idle_pill` list at 200 x 32).
- Produces: `pub struct SheetView { pub section: Section, pub focus: Option<FocusId>,
  pub hover: Option<FocusId>, pub field: TextField }`; this task creates `sheet/field.rs` with
  only `pub struct TextField { pub text: String, pub caret: usize, pub all: bool }` (Task 5 adds
  its editing).
- Produces: `pub enum SheetAction { Section(Section), Change(SettingChange), FocusField,
  OpenFolder }`; `pub struct SheetTarget { pub rect: Rect, pub id: FocusId, pub action:
  SheetAction }`; `pub enum FocusId { Section(Section), Row(u8, u8) }` (row, control index).
- Produces: `pub fn sheet_build(facts: &SheetFacts, view: &SheetView) -> (DisplayList,
  Vec<SheetTarget>)` (list size 840 x 640) and `pub fn sheet_hit(targets: &[SheetTarget],
  x: f64, y: f64) -> Option<&SheetTarget>` (no slack, first match).
- Layout from the drawn Boşta section (`design/r7/karot.html` `settings()`): sidebar 156 wide
  with `AYARLAR` and section names (active marked by a filled square); content x 184 to 812;
  heading 20 DIP at y 44, its line under it; rows separated by `Ink::Border` rules; label + sub
  left, control right-aligned at 812. The flyout row is dim (`Ink::Faint`) with no target while
  `osd` is off. The city field shows only for `LocationMode::City`. The save error line replaces
  the heading line while `save_failed`. Each Boşta preview card is an `Inset` at scale 0.45 with
  the rest shape's floor under it; Hidden draws a faint line.

- [x] **Step 1: Write the failing tests:** `four_sections_in_sidebar` (targets for the four
  sections, `Section::Idle` marked); `idle_rows_in_order` (labels Kapalı ada, Şerit, Sağ üst,
  Bugün çalanlar, Hava, Konum, Birim in Turkish); `six_preview_insets` (six `Node::Inset` with
  scale 0.45 on Boşta); `click_span_changes` (hit at the `TÜM GÜN` word gives
  `Change(Span(Day))`); `toggle_flips` (hit on the Hava toggle with weather off gives
  `Change(Weather(true))`); `flyout_row_dead_while_osd_off`; `city_field_only_for_city`;
  `no_lime_in_sheet` (no node uses `Ink::Flow` except inside previews); `fits_840_by_640` (all
  nodes inside the list size for every section in German).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement** the builders; one file per section, shared controls in `controls.rs`.
- [x] **Step 4: Run tests and checks, expect PASS.**
- [x] **Step 5: Commit** `feat(core): build the settings sheet`.

### Task 5: Focus, keys and the text field

**Files:**
- Create: `crates/plith-core/src/sheet/keys.rs`; modify `crates/plith-core/src/sheet/field.rs`
- Test: `crates/plith-core/src/sheet/keys/tests.rs`, `sheet/field/tests.rs`

**Interfaces:**
- Produces: `pub enum SheetKey { Tab, BackTab, Left, Right, Up, Down, Home, End, Backspace,
  Delete, Enter, Space, Escape, SelectAll, Copy, Paste(String), Char(char) }`.
- Produces: on `TextField` (Task 4) `pub fn edit(&mut self, key: &SheetKey) -> FieldOutcome` and `pub enum FieldOutcome { Edited,
  Moved, Commit(String), Cancel, Copy(String), Ignored }`; max 64 chars (paste is cut).
- Produces: `pub fn on_key(facts: &SheetFacts, view: &mut SheetView, key: SheetKey) -> KeyOutcome`
  with `pub enum KeyOutcome { Redraw, Act(SheetAction), Copy(String), Close, Nothing }`.
- Rules: Tab/BackTab walk the targets in build order (sidebar, then rows); Left/Right inside a
  choice group pick the neighbour (`Act(Change(..))`); Space/Enter act on the focused control;
  Escape closes unless the field is focused (then `Cancel` restores the saved city and leaves the
  field); Enter in the field `Commit`s; leaving the field by Tab or click also commits.

- [x] **Step 1: Write the failing tests:** `field_basic_editing` (type "İst", Backspace, Home,
  Char, caret positions in chars, not bytes); `field_paste_cut_at_64`; `field_escape_cancels`;
  `tab_walks_in_order`; `arrows_pick_in_group`; `escape_closes_outside_field`;
  `enter_in_field_commits_once` (Commit carries the text; no `Act` per typed char).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement.**
- [x] **Step 4: Run, expect PASS.**
- [x] **Step 5: Commit** `feat(core): add sheet keys and text field`.

### Task 6: Sheet render thread (model: opus)

**Files:**
- Create: `crates/plith-render/src/sheet.rs`, `crates/plith-render/src/sheet/{paint.rs,target.rs}`
- Modify: `crates/plith-render/src/lib.rs` (exports)
- Modify: `crates/plith-render/examples/render_pages.rs` (or a new `examples/render_sheet.rs`)
- Test: `crates/plith-render/tests/sheet.rs` (offscreen)

**Interfaces:**
- Consumes: `DisplayList`, `Node::Inset` (Task 3), `Gpu::new`, `ListKit`/`ListEnv`/`draw_list`,
  `TextSystem`, `Palette`.
- Produces: `pub struct SheetConfig { pub hwnd: isize, pub width_px: u32, pub height_px: u32,
  pub scale: f64, pub palette: Palette }`;
  `pub struct SheetRender` with `pub fn spawn(config: SheetConfig, on_fatal: Box<dyn Fn(String) +
  Send>) -> Result<SheetRender, RenderError>`, `pub fn publish(&self, list: Arc<DisplayList>)`
  (latest wins), `pub fn resize(&self, width_px: u32, height_px: u32, scale: f64)`,
  `pub fn stop_and_join(self)`; `Drop` stops and joins.
- Produces: `pub fn render_sheet_offscreen(list: &DisplayList, scale: f64, palette: Palette) ->
  Result<Vec<u8>, RenderError>` (BGRA) for renders and tests.
- Rules: the thread owns its own `Gpu`, D2D context, composition swap chain sized to the client
  rect (opaque floor, no island shape) and `TextSystem`. It blocks on its own condvar and paints
  only when a newer list or a resize arrives: no clock, no loop at rest. Device loss rebuilds
  the stack once, then reports `on_fatal`. Thread name `plith-sheet`.

- [x] **Step 1: Write the failing tests:** `sheet_offscreen_size` (840 x 640 list at scale 2 gives
  1680 x 1280 x 4 bytes); `sheet_floor_is_opaque` (corner pixel alpha 255);
  `inset_preview_visible` (a Task 4 Boşta build has ink inside the first preview card).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement**; add the example output: each section in `tr` and `de` at scale 1
  and 2 as PNG into a fresh folder `docs/design/r7/settings-1/`.
- [x] **Step 4: Run tests; run the example; open the PNGs and compare with `renders-3/sheet.png`
  (Boşta): rows, rules, controls, previews readable at 1x.**
- [x] **Step 5: Commit** `feat(render): add the settings sheet renderer`.

### Task 7: Settings window in the shell

**Files:**
- Create: `crates/plith-shell/src/sheet_window.rs`, `sheet_window/{proc.rs,keys.rs,clipboard.rs}`
- Modify: `crates/plith-shell/src/lib.rs` (exports only)
- Test: `crates/plith-shell/src/sheet_window/keys/tests.rs` (virtual key to `SheetKey` mapping)

**Interfaces:**
- Produces: `pub enum SheetEvent { Move(f64, f64), Leave, Click(f64, f64), Key(SheetKey),
  Resized { width_px: u32, height_px: u32, scale: f64 }, Closed }` (coordinates in DIP).
- Produces: `pub struct SheetWindow` with `pub fn open(monitor: isize, title: &str, on_event:
  Box<dyn FnMut(SheetEvent)>) -> Result<SheetWindow, ShellError>`, `pub fn hwnd(&self) -> isize`,
  `pub fn client_px(&self) -> (u32, u32, f64)`, `pub fn raise(&self)`, `pub fn set_title(&self,
  title: &str)`, `pub fn close(self)`; `pub fn set_clipboard(text: &str)`.
- Rules: own class `Plith.Settings` and wndproc (never the island's `Inner`); style
  `WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX`, ex `WS_EX_NOREDIRECTIONBITMAP`;
  `DWMWA_USE_IMMERSIVE_DARK_MODE` on; client 840 x 640 DIP at the monitor's scale, centered on
  that monitor's work area; `WM_DPICHANGED` resizes to the suggested rect and emits `Resized`;
  `WM_KEYDOWN`/`WM_CHAR` map to `SheetKey` (Ctrl+A/C/V, Shift+Tab); `WM_CLOSE` destroys and
  emits `Closed`; `SetForegroundWindow` on open, `FlashWindowEx` if refused. The `on_event`
  callback must not be called re-entrantly from inside `open` (queue events until `open`
  returns).

- [x] **Step 1: Write the failing test** `keys_map` (VK_TAB+shift → BackTab, 'A'+ctrl →
  SelectAll, VK_ESCAPE → Escape, WM_CHAR 'ş' → Char('ş'), control chars below 0x20 ignored).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement.**
- [x] **Step 4: Run tests and checks, expect PASS.**
- [x] **Step 5: Commit** `feat(shell): add the settings window`.

### Task 8: Quick page and gear mark (+ ADR 0017)

**Files:**
- Modify: `crates/plith-core/src/pages.rs` (`PageId::Quick`, `PAGE_COUNT = 6`, new `Action`s),
  `pages/tabbar.rs` (gear, clock moves left 24 DIP), `pages/glyphs.rs` (`gear`),
  `crates/plith-render/src/pages.rs` (`PILL` slot from `PAGE_COUNT`), `crates/plith/src/pages.rs`
  (build dispatch, `Pages::new`), `crates/plith-core/src/scene.rs` if it pins 5
- Create: `crates/plith-core/src/pages/quick.rs`
- Create: `docs/decisions/0017-settings-window-and-quick-page.md` (two layers, gear mark,
  quick page, sheet render thread; amends 0016 for the tab row)
- Test: `pages/quick/tests.rs`, `pages/tabbar/tests.rs`, tests that pin `PAGE_COUNT` or slot 4

**Interfaces:**
- Produces: `Action::Look(IdleLook)`, `Action::OpenSettings`; gear target `Action::Tab(PageId::Quick)`
  on any page, `Action::Tab(PageId::Welcome)` while on Quick.
- Produces: `pub fn gear(x: f64, y: f64, ink: Ink) -> Vec<Node>` (16 x 16 box) and
  `pub fn quick_page(facts: &QuickFacts) -> PageBuild` with `pub struct QuickFacts { pub look:
  IdleLook, pub previews: [Arc<DisplayList>; 6], pub lang: Lang }`; `PageId::TABS` stays four.
- Layout: tab row as on every page, gear centered on x 590 and the home mark's y; caption
  `KAPALI ADA`; six preview cells in one row across 560 DIP (scale 0.4 insets, look word under
  each, picked one with a filled square); a rule; row `TÜM AYARLAR` with an open glyph.
  `open_on = LastPage` never records `Quick`.

- [x] **Step 1: Write the failing tests:** `gear_target_last` (gear rect at x 590, action
  `Tab(Quick)`; on Quick it is `Tab(Welcome)` and the gear draws `Ink::Ink`);
  `clock_moves_for_gear`; `quick_has_six_looks` (six `Action::Look` targets in `IdleLook`
  order, six insets); `quick_open_settings_row`; `page_count_six`.
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement**, fix every pinned 5 (`rg "PAGE_COUNT|\[_; 5\]|PILL" crates`).
- [x] **Step 4: Run the full plith-core, plith-render and plith tests, expect PASS.**
- [x] **Step 5: Commit** `feat(pages): add the quick page and gear`.

### Task 9: Runtime setters: language, idle settings, weather, OSD slot (model: opus)

**Files:**
- Modify: `crates/plith/src/app/feed.rs` (two thin methods only), `app/idle_facts.rs`
  (`set_settings`, `set_lang`), `app/weather_link.rs` (`reconfigure`)
- Create: `crates/plith/src/app/feed/settings.rs` (`Feed::apply_settings`),
  `crates/plith/src/app/osd_slot.rs`
- Test: `app/feed/settings/tests.rs`, `app/osd_slot/tests.rs`, `app/weather_link/tests.rs`

**Interfaces:**
- Consumes: `Effects`, `Settings` (Task 1).
- Produces: `impl Feed { pub fn apply_settings(&mut self, settings: &Settings, lang: Lang,
  effects: Effects) }`: sets `lang` and the idle settings, rebuilds dirty pages (all pages and
  the pill for `lang`; welcome and pill otherwise), keeps the current page and expansion.
- Produces: `impl WeatherLink { pub fn reconfigure(&self, config: WeatherConfig) }`; turning
  weather off hands the link to a short thread that calls `stop()`; on builds a new link with
  `weather_link::start` and `feed.set_weather`.
- Produces: `pub struct OsdSlot<S: OsdStart>` with states `Off`, `Running(S::Side)`,
  `Stopping { join: JoinHandle<()>, then_start: bool }`; `pub fn set(&mut self, on: bool, mode:
  FlyoutMode)`, `pub fn restart(&mut self, mode: FlyoutMode)`, `pub fn poll(&mut self)` (called
  on Wake: a finished stop starts the pending side). Stopping drops the side on a spawned thread
  that wakes the UI when done. `trait OsdStart { type Side: Send + 'static; fn start(&self,
  mode: FlyoutMode) -> Option<Self::Side>; fn sink(side: &Self::Side) -> OsdSink; }`, real impl
  over `OsdSide`, fake in tests.

- [x] **Step 1: Write the failing tests:** `off_on_quickly_starts_once` (fake: set(false), set(true)
  before the stop finishes → one start after `poll` sees the join done, never two live sides);
  `restart_changes_mode`; `set_never_blocks` (fake side whose drop sleeps 1 s: `set(false)`
  returns in under 50 ms); `lang_switch_keeps_page` (feed on Audio, expanded, `apply_settings`
  with `lang` → still Audio and expanded, the page list differs from before);
  `unit_change_rebuilds_welcome_only`; `weather_reconfigure_reaches_service` (link test with the
  existing service fake).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement.** `app.rs` moves the `OsdSide` local into an `OsdSlot` owned by `App`.
- [x] **Step 4: Run tests and checks, expect PASS.**
- [x] **Step 5: Commit** `feat(app): apply settings while running`.

### Task 10: App wiring: open, act, save, close

**Files:**
- Create: `crates/plith/src/app/sheet_side.rs` (window + render + view), `app/saver.rs`
- Modify: `crates/plith/src/app/input.rs` (`Action::Look`, `Action::OpenSettings`),
  `app/state.rs` (drain the sheet queue on `Wake`), `app.rs` (owner `settings`, saver, shutdown order)
- Test: `app/saver/tests.rs`, `app/sheet_side/tests.rs` (pure parts: event to action routing)

**Interfaces:**
- Consumes: Tasks 1 to 9.
- Produces: `App.settings: Settings` (the one owner), `fn change(&mut self, change: SettingChange)`
  → `apply_change` → `feed.apply_settings` / weather / `OsdSlot` → `saver.save(settings.clone())`.
- Produces: `pub struct Saver` with `pub fn spawn(path: PathBuf, waker: UiWaker) -> Saver`,
  `pub fn save(&self, settings: Settings)` (latest wins, one worker blocked on a channel),
  `pub fn failed(&self) -> bool`, `pub fn flush_and_join(self)` (writes the pending value).
- Produces: `SheetSide` holds `SheetWindow`, `SheetRender`, `SheetView`; window events go into an
  `Rc<RefCell<VecDeque<SheetEvent>>>` plus `UiWaker::wake()`, drained in `App::on_event(Wake)`;
  each drain rebuilds the list only if the view or facts changed, then `publish`es. Opening a
  second time `raise`s. `Closed` drops the render (join) and the window. The Karot preview list
  is republished when the pill rebuilds (once a minute).
- The window opens on the section last shown in this run (`Section::Idle` the first time);
  `SheetAction::OpenFolder` runs `explorer.exe` on the settings folder; a language change calls
  `set_title`; facts carry `env!("CARGO_PKG_VERSION")` and the settings path.
- Shutdown order in `app.rs`: close the sheet, `saver.flush_and_join()`, then the existing order.
- Only a normal run (`idle_wiring::is_normal`) saves; probing runs never write the file.

- [x] **Step 1: Write the failing tests:** `saver_latest_wins` (three saves before the worker
  runs → file holds the third); `saver_flush_writes_pending`; `saver_reports_failure` (path in a
  missing read-only dir → `failed()` true, waker called); `sheet_events_route` (Click on a
  target → `change` called with its `SettingChange`; `Key(Escape)` → close).
- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement.**
- [x] **Step 4: Run tests; build release; start `plith.exe` on this PC (no mouse from scripts):
  check the log for a clean start, then close with `WM_CLOSE` to `Plith.Notch`, exit 0.**
- [x] **Step 5: Commit** `feat(app): wire the settings window`.

### Task 11: Records and merge review

**Files:** `docs/STATUS.md`, `docs/journal/2026-11.md`, `docs/ROADMAP.md`, this plan's progress log.

- [ ] **Step 1:** Idle check after opening and closing the window: `plith.exe --idle-check 60`
  equivalent per STATUS how-to, wakes on UI and render threads as before the change (record the
  number from the run's own output).
- [ ] **Step 2:** Whole-branch review (`superpowers:requesting-code-review`, opus), fix findings.
- [ ] **Step 3:** Five checks on a trial merge (fmt, clippy, tests via heavy-run or the office VM,
  structure-check, release build).
- [ ] **Step 4:** STATUS rewritten (settings merged, console owed), ROADMAP row with actual
  duration. Merge to `main` only with Berke's yes.

### Task 12: Console session (Berke present, console not RDP)

- [ ] Gear opens the quick page; a look click changes the pill at once; `TÜM AYARLAR` opens the
  window in front.
- [ ] Every row changes the island live; Tab, arrows, Space, Esc work; the city field types
  Turkish letters, Enter saves, a real weather fetch reaches `OKUMA`.
- [ ] Language to Deutsch and back: island, window and OSD word follow.
- [ ] OSD off: volume key shows no OSD and the log shows the helper stopped; on again: OSD shows.
  Flyout hide only with his separate yes.
- [ ] Window at 100 and 200 percent scale (or a second monitor if present) reads right.

## Progress log
(written while the plan runs, never back-filled)

The live log ran in the git-ignored SDD ledger (`.superpowers/sdd/r7-settings/progress.md`); this
is its summary, written 2026-11-20 at the end of Task 10 and the final review.
- Tasks 1 to 10 done on 2026-11-20, each with a task review (fix rounds: T4, T5, T7, T9, T10
  one round each; T6, T8 one follow-up round). Commits `6d68820` to `8f5306b`.
- Final whole-branch review (opus): ready to merge, 0 Critical, 0 Important, 10 Minor; fix wave
  `22541c0`, `4e6798a`, `de6025d` for Minors 1, 2, 3, 4, 5, 7, 8, 9.
- Deviations from the plan text (rulings): heading style `TextStyle::Heading` 20 DIP added (T4);
  off toggle = two hollow stations (T4); city field on its own line under Konum (T4); caret and
  selection drawn in T5, measured by a UI-thread `TextMeasure` (T6) and filled in T10; preview
  pills raised with an outline (T6); `Click` on release with capture (T7); the settings window no
  longer destroys itself on `WM_CLOSE`, the app stops the render thread first (T10); the stop
  thread waits for the old OSD pipe to vanish before a restart (T9); the island's minute clock
  runs while the window is open so the Karot preview keeps time (T10); the render page cache
  `PILL` slot was 4, shared with Welcome, now `PAGE_COUNT` (T8).
- Not measured yet (Task 11 Step 1 and Task 12): idle wakes, the window opened from a real click,
  the click-to-window delay, a real save, the caret blink, an OSD restart on the helper.
