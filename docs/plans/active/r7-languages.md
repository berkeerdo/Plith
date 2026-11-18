# R7 languages Implementation Plan

> A ticked box means the step was taken, not that nothing deviated.
> Deviations, deliberate omissions and decisions are in the progress log, ADR 0015 and STATUS.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every string Plith writes comes from the text table, in eight languages, with the
percent formatted per language and every own string proven to fit its box and font.

**Architecture:** `plith-core::lang` grows from two to eight languages, each a file with an
exhaustive `match`. The R4 page builders take a `Lang` and read the table instead of
`pages/copy.rs`. The OSD protocol carries the muted word, so the helper stays language-free.

**Tech Stack:** Rust 2024, `windows` crate (DirectWrite for the glyph test), no new dependency.

**Spec:** `docs/specs/2026-11-18-r7-languages-design.md`

## Global Constraints
- Languages and codes: `en tr de es fr pt it pl`; English is the fallback for an unknown code.
- Own copy is stored upper-cased; Turkish under `tr_upper` rules, the rest under
  `to_uppercase`; German upper-case copy writes `SS`, never `ß`.
- Turkish strings and renders do not change. English keeps its R7a strings.
- Percent: Turkish `%62`; `en pt it pl` `62%`; `de fr es` `62 %` (plain space).
- No ellipsis in own copy in any language: shorten the translation, never the box.
- OSD muted word: at most 6 characters in every language; the OSD number has no percent sign.
- `PROTOCOL_VERSION` becomes 2 in this plan; the helper reinstall needs Berke's yes.
- Code and comments in English; never the em dash; files under 500 lines, functions under 80.
- Builds and test runs through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`; never `bash` from
  PowerShell; never `rm` a variable or glob path.

## Review Focus
1. A Windows UI language with a region the table lacks (`pt-PT`, `de-CH`, `fr-CA`): it must
   resolve by primary code to `pt`, `de`, `fr`, not to English. Test in Task 1.
2. Polish counts at 12 to 14 and 22 to 24 (`12 UTWORÓW`, `22 UTWORY`): the teen rule. Test in
   Task 1.
3. A run before the idle wiring sets a language (demo, fixture, idle check): pages must still
   build in the resolved language, not silently in English or Turkish. Test in Task 2.
4. A 100 % volume in the longest-percent language (`100 %` in German) in the pill and the
   system ruler: still fits. Test in Task 3.
5. An old helper (protocol 1) meeting the new client: the client fails closed, no panic, the
   notch keeps running. Test in Task 4.

---

### Task 1: Eight languages in the text table

**Files:**
- Modify: `crates/plith-core/src/lang.rs`, `crates/plith-core/src/lang/en.rs`,
  `crates/plith-core/src/lang/tr.rs`, `crates/plith-core/src/lang/tests.rs`
- Create: `crates/plith-core/src/lang/{de,es,fr,pt,it,pl}.rs`; if `lang.rs` passes 500 lines,
  move `songs` and `percent` into `crates/plith-core/src/lang/count.rs`

**Interfaces:**
- Produces: `Lang::{En, Tr, De, Es, Fr, Pt, It, Pl}`, `Lang::ALL: [Lang; 8]`, `Lang::code`;
  `text(t: Text, lang: Lang) -> &'static str` (exhaustive per language, no `Option`);
  `percent(v: u8, lang: Lang) -> String`; `songs(n: usize, lang: Lang) -> String`;
  `date_short(..)`, `upper(s, lang)` for all eight.
- New `Text` variants (Turkish value is today's `copy.rs` text): `TabAudio` (SES),
  `TabMedia` (MEDYA), `TabShelf` (RAF), `TabSystem` (SİSTEM), `Muted` (SESSİZ), `DdcOff`,
  `ShelfHint`, `DefaultCaption` (VARSAYILAN), `OtherOutputs`, `NoOutput`, `FooterNormal`,
  `FooterPicking`, `FooterOutputs`, `Unused` (Kullanılmıyor, the one mixed-case string: it
  stands in for a Voicemeeter bus name). `Off` and `Kind*` are reused. `Text::ALL` lists all.

- [x] **Step 1: Write the failing tests** in `lang/tests.rs`:
  `every_language_has_every_string` (for each `Lang::ALL` and `Text::ALL`, `text` is not
  empty), `own_copy_is_upper_case` (every string but `Unused` equals `upper(s, lang)`; no `ß`
  in German), `turkish_copy_is_unchanged` (each new variant in Turkish equals the old
  `copy.rs` constant), `percent_follows_the_language` (`percent(62, Tr) == "%62"`,
  `En == "62%"`, `De == "62 %"`, `Fr == "62 %"`, `Es == "62 %"`, `Pt`, `It`, `Pl == "62%"`),
  `regions_resolve_by_primary_code` (`pt-PT` gives `Pt`, `de-CH` gives `De`, `fr-CA` gives `Fr`,
  `xx-YY` gives `En`, `pl` code gives `Pl`), `polish_counts_follow_the_teen_rule`
  (1, 2, 4, 5, 12, 14, 22, 25 give the singular, few, many forms with 12 and 14 many),
  `dates_exist_in_every_language` (`date_short(1, 17, 11, lang)` is three non-empty parts).
- [x] **Step 2: Run** `cargo test -p plith-core --lib lang` (via heavy-run). Expected: FAIL to
  compile on the missing variants.
- [x] **Step 3: Implement.** One file per language, exhaustive `match`, `WEEKDAYS` and `MONTHS`
  tables, short and terse wording in the register of the Turkish and English copy. The muted
  word (`Muted`) and `Off` at most 6 characters. `songs` per language (Polish: `UTWÓR`,
  `UTWORY` for 2 to 4 except 12 to 14, `UTWORÓW` otherwise; others singular for 1, plural
  else).
- [x] **Step 4: Run** the same command. Expected: PASS, plus the whole `plith-core` suite.
- [x] **Step 5: Commit** `feat(lang): add six languages to the text table`.

### Task 2: Pages read the table

**Files:**
- Modify: `crates/plith-core/src/pages.rs`, `pages/tabbar.rs`, `pages/audio.rs`,
  `pages/audio/{kind,layout,outputs,model}.rs`, `pages/shelf.rs`, `pages/system.rs`,
  `pages/pill.rs`, `pages/idle_pill.rs`, `pages/welcome/head.rs`, `pages/copy.rs`,
  `pages/fixtures/tests.rs`, `crates/plith/src/pages.rs`, `crates/plith/src/app/feed.rs` (and
  the feed construction site), `crates/plith-render/examples/render_pages*.rs`

**Interfaces:**
- Consumes: Task 1's `text`, `percent`, `Text` variants.
- Produces: `tabbar(active, clock, lang)`, `audio_page(view, clock, lang)`,
  `shelf_page(clock, lang)`, `system_page(facts, clock, lang)`, `pill(facts, lang)`
  (`media_page` writes no own copy and keeps its signature apart from what `tabbar` needs);
  `Inputs.lang: Lang` in `crates/plith/src/pages.rs`;
  `sound_words(value, muted, kind, off, lang)` (its percent comes from `percent`).
  `copy.rs` keeps only `tr_upper`; `OWN_LABELS` and `UNUSED_NAME` go.

- [x] **Step 1: Write the failing tests:** `tabs_follow_the_language` (the tab bar for `De`
  shows the German tab labels, for `Tr` `SES MEDYA RAF SİSTEM`),
  `system_ruler_percent_follows_the_language` (`system_page` with brightness 70 shows `%70`
  in `Tr`, `70%` in `En`), `the_feed_builds_pages_in_its_language` (a feed built with `De` and
  no idle wiring yields a tab bar in German; Review Focus 3), and the existing fixture
  upper-case test rewritten over `Text::ALL` for every language.
- [x] **Step 2: Run** `cargo test --workspace` (via heavy-run). Expected: FAIL to compile.
- [x] **Step 3: Implement.** Replace every `copy::` and `UNUSED_NAME` use with `text(.., lang)`;
  the three `format!("%{..}")` sites use `percent`. The feed holds the `Lang` resolved at
  start (`resolve(&settings.lang, &locale::windows_language())`, as `idle_wiring` does today)
  for every run mode and passes it in `Inputs`. Measurement constants that held a Turkish
  word (`LABEL_TEXT` in `audio/layout.rs`) measure the widest of the eight.
- [x] **Step 4: Run** the five checks. Expected: all green; the existing Turkish render tests
  pass unchanged.
- [x] **Step 5: Commit** `feat(pages): read every own string from the table`.

### Task 3: Fit and glyph proofs

**Files:**
- Create: `crates/plith-core/src/lang/fit.rs` (per-style advances and budgets, test-only or
  `pub(crate)`), `crates/plith-core/src/lang/fit/tests.rs`,
  `crates/plith-render/src/text/tests/glyphs.rs` (or the crate's existing test layout)

**Interfaces:**
- Consumes: Task 1's table, Task 2's layout constants (box widths made `pub(crate)` where needed).
- Produces: `fn advance(style: TextStyle) -> f64` and one budget per own-copy place.

- [x] **Step 1: Measure** the per-character advance of each text style used by own copy from
  the bundled fonts (Martian Mono and Public Sans at their style sizes and tracking; a
  conservative maximum, as `VALUE_ADVANCE = 7.6` is for `Value`). Record the numbers in the
  file's doc comment.
- [x] **Step 2: Write the tests:** `every_own_string_fits_its_place` (for each language, each
  of tab labels, footers, DDC note, shelf hint, outputs captions, welcome labels and the
  `Muted`/`Off` words in the OSD slot of 48 DIP: chars times advance within the budget; budget
  is the stated box width, else the room the layout leaves, else the Turkish string's width),
  `a_full_percent_fits_everywhere` (`percent(100, lang)` in the pill's 112 DIP and the system
  value box, every language; Review Focus 4), and in `plith-render`
  `every_own_string_has_glyphs` (for each language and string, DirectWrite
  `GetGlyphIndices` in the bundled face of its style returns no 0).
- [x] **Step 3: Run** them. Expected: failures name the strings that do not fit; shorten those
  translations in Task 1's files until PASS. Never widen a box.
- [x] **Step 4: Run** the five checks. Expected: green.
- [x] **Step 5: Commit** `test(lang): prove every string fits and has glyphs`.

### Task 4: OSD muted word over the pipe

**Files:**
- Modify: `crates/plith-ipc/src/message.rs` (+ its tests), `crates/plith-core/src/osd.rs`,
  `crates/plith-core/src/osd/tests.rs`, `crates/plith-osd/src/paint.rs`,
  `crates/plith-osd/src/paint/tests.rs`, the OSD frame type that carries `muted`, and the
  `plith` call site that builds `Message::Level`

**Interfaces:**
- Consumes: Task 1's `text(Text::Muted, lang)` and `text(Text::TabAudio, lang)`.
- Produces: `Message::Level { meter, percent, muted, label, muted_word: String }`
  (`muted_word` at most `MAX_LABEL` bytes, cut like `label`); `PROTOCOL_VERSION = 2`;
  `osd::reading(audio, system, lang)` with the fallback label from the table; `value_text`
  draws `frame.muted_word`.

- [x] **Step 1: Write the failing tests:** `level_round_trips_the_muted_word`,
  `a_version_1_frame_is_rejected` (decode of a frame with version byte 1 returns the version
  error), `an_overlong_muted_word_is_cut` (bounded like `label`), `value_text_draws_the_sent_word`
  (`muted` with `muted_word = "STUMM"` gives `STUMM`), `fallback_label_follows_the_language`,
  and on the client side `an_old_helper_fails_closed` (a server answering with protocol 1
  leaves the link given up, no panic; reuse the existing link test rig; Review Focus 5).
- [x] **Step 2: Run** `cargo test -p plith-ipc -p plith-core -p plith-osd` (via heavy-run).
  Expected: FAIL to compile.
- [x] **Step 3: Implement.** Bump the version, add the field to encode and decode, fill it in
  `plith` from the feed's `Lang`, drop `copy::MUTED_LABEL` from `plith-osd`.
- [x] **Step 4: Run** the five checks. Expected: green.
- [x] **Step 5: Commit** `feat(osd): send the muted word with each level`.

### Task 5: Renders and records

**Files:**
- Modify: `crates/plith-render/examples/render_pages.rs` (+ `render_pages/idle_states.rs`),
  `docs/STATUS.md`, this plan's progress log, `docs/decisions/` (ADR 0015)

- [x] **Step 1:** Add `--lang <code>` to the render example; render every page and pill look
  for each new language into a new folder `docs/design/r7/lang-<code>/`, and Turkish into a
  fresh folder to compare with `engine-3/`.
- [x] **Step 2:** The master reads every sheet: no clipped or overlapping text, Turkish
  identical to `engine-3/`. Fix and re-render into a fresh folder if not.
- [x] **Step 3:** ADR 0015 (eight languages, exhaustive table, muted word over the pipe,
  protocol 2). STATUS: owed on screen are a language other than Turkish via `lang=de` in
  `settings.ini`, and the helper reinstall (Berke's yes). Progress log line with the five
  checks' numbers.
- [x] **Step 4: Commit** `docs: record the R7 languages`.

## Progress log
- 2026-11-18, branch `feature/r7-languages` (`4f6cc7f..d1d6611` plus the final-review fix),
  each task by a fresh implementer with a task review; reviews and rulings in the git-ignored
  `.superpowers/sdd/r7-languages/` ledger.
- Task 1 (`4f6cc7f`, `8ee770b`): one review round polished translations (native `Off` words in
  fr, it, pl, pt; es keeps `OFF`; French 0 is singular).
- Task 2 (`c0a6f8e`, `f691050`, `8f4aa87`): tests written after the signature refactor, not
  first. Review found six more hard-coded Turkish strings (shelf drop, Brightness, Microphone,
  media empty state and hint, `Kaynak n`), moved to the table, plus a leak guard test.
- Task 3 (`6033e38`): fit is measured with the renderer's DirectWrite formats instead of
  advance constants (ruling: stronger proof). Every string fit on the first run; the test was
  shown to fail on planted long strings. Tightest: OSD slot 45.6 of 48 DIP, pill right 106.4
  of 112.
- Task 4 (`5214972`, `9c4342a`): review (opus) found the client read "up" after a rejected
  `Hello`, so an old helper could let flyout suppression hide the native flyout with no OSD.
  Fixed with the admission probe (ADR 0015 Decision 7) and a looping fake v1 helper test.
- Task 5 (`d1d6611`): renders `docs/design/r7/lang-*/{pages,idle}`; `lang-tr/idle` is
  byte-identical to `engine-3/`. Master read es, de, it, fr, pl and en sheets: no clipping.
- Final review (opus): 0 Critical, records step open, 4 Minor. Fix wave `96d99f2`: three welcome
  fit places, ipc README, French `JUIN`/`JUIL`; 1049 tests. Turkish page renders from the base
  commit are pixel-identical to `lang-tr/pages` (17 of 17). Subject of `f691050` is 51 chars.
- Not done here: the helper reinstall (protocol 2) and any on-screen check.
