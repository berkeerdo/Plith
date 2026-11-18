# R7 languages: every UI string from the text table, eight languages

Date 2026-11-18. Follows ADR 0014 Decision 6 and the R7a plan (`plans/active/idle-look-r7.md`,
"Languages"). Comes before the R7 settings window, which will offer the language choice.

## Intent
Berke (2026-11-17): "ingilizce ve türkçe olsun", then "ya da bilindik dilleri ekleyebilirsin".
On 2026-11-18 he left the set to the master's pick ("önerin hangisi ise uygula"). Plith is
for people who care about sound and play games, not only in Turkey; a notch that speaks the
user's language is table stakes next to Oyenza and FluentFlyout.

## Goals
1. Every string we write comes from the text table: R7a already does this, the R4 pages
   (`pages/copy.rs`), `UNUSED_NAME` and the OSD mute word do not yet.
2. The percent is formatted per language (`%62` Turkish, `62%` English, `62 %` German and French).
3. Eight languages: English, Turkish, German, Spanish, French, Portuguese (Brazil), Italian,
   Polish. `lang=auto` picks them from the Windows UI language by primary code.
4. Every fixed text box fits its longest translation, proven by tests per language.

## Non-goals
- Cyrillic, Greek and CJK languages (font fallback, double-width glyphs): a later plan.
- The settings window and a live language switch: the R7 settings plan. A language change
  still takes a restart.
- Translating names from Windows, Voicemeeter or media (devices, apps, titles): never.

## Anti-goals
- Turkish looks different afterwards: its strings and renders stay as they are. English keeps
  its R7a strings; the R4 pages, which show Turkish to everyone today, gain English copy, and
  the English percent turns from `%62` to `62%`.
- An ellipsis in our own copy in any language. A translation that does not fit is shortened;
  the box does not grow and the layout does not move.
- A string that silently shows in the wrong language.

## Approaches
1. **Per-language Rust files with an exhaustive `match` (picked).** The R7a pattern:
   `lang/<code>.rs` with `fn text(t: Text) -> &'static str`. The compiler forces every
   language to carry every string, so the English fallback never triggers for a missing one;
   no I/O, no new dependency, testable in `plith-core`.
2. Runtime resource files (Fluent `.ftl` or JSON next to the exe). Lets outsiders translate
   without a build, but adds a parser dependency, file I/O at start and missing-key handling.
   Not worth it before there is a community.
3. Keep `copy.rs` and add a language parameter to each constant site. Two tables for one job.

## Design
- **Text table.** Each `copy.rs` constant becomes a `Text` variant (tab labels, muted, DDC/CI
  note, shelf hint, default caption, other outputs, no output, the three footers,
  `Kullanılmıyor`). Variants that exist already are reused, not doubled: `OFF_LABEL` is
  `Text::Off`, `KIND_*` are `Text::Kind*`. `DEFAULT_CAPTION` (`VARSAYILAN`) stays a separate
  variant from `Text::DefaultOutput` (`VARSAYILAN ÇIKIŞ`). `copy.rs` keeps only `tr_upper`
  (or it moves into `lang`); callers get the `Lang` through the facts or inputs they already
  receive. `OWN_LABELS` becomes a test over `Text::ALL` for every language.
- **Lookup.** Each language file's `text` becomes an exhaustive `match` returning
  `&'static str`, so a language with a missing string does not compile; the `Option` and its
  English fallback go. English stays the fallback only for an unknown language code.
- **Counts and dates.** `songs(n, lang)` gets a rule per language (Polish: 1, 2 to 4 except
  12 to 14, and 5+ forms; the others singular and plural). Weekday and month abbreviations get
  a table per language, upper-cased, three letters where the language has a usual short form.
- **Casing.** Own copy is stored upper-cased in each file (as `tr.rs` does). A test checks
  every string equals its own upper-casing: `tr_upper` for Turkish, `to_uppercase` for the
  rest (German keeps `ß` out of upper-case copy: write `SS`).
- **Percent.** One `percent(v: u8, lang) -> String` in `lang`, used by the system ruler, the
  welcome sound head and the pill's `sound_words`. Turkish `%62`; English, Portuguese,
  Italian, Polish `62%`; German, French, Spanish `62 %` (a plain space).
- **OSD.** `Message::Level` gains `muted_word: String` (bounded by `MAX_LABEL`), filled by
  `plith` from the table; the helper draws it instead of `copy::MUTED_LABEL`. The fallback
  label (`TAB_AUDIO` in `core/osd.rs`) also comes from the table. `PROTOCOL_VERSION` goes to 2
  in the same merge, so the helper is rebuilt and reinstalled (`build-osd.ps1`,
  `install-osd.ps1` under sudo, Berke's yes) before the next normal run; until then the old
  helper rejects the new client, which already fails closed (no OSD, the notch still works).
  The OSD value slot stays 48 DIP: the muted word is at most six characters in every language
  (for example `STUMM`, `MUDO`, `MUET`), and the OSD number keeps no percent sign.
- **Auto.** `lang=auto` maps the Windows UI language's primary code (`de-AT` gives `de`,
  `pt-PT` gives the Brazilian `pt` text). Codes in `settings.ini`: `en tr de es fr pt it pl`.
- **Fit.** For each place with own copy (tab labels, pill right side, footers, DDC note,
  shelf hint, outputs captions, OSD value slot, welcome labels), a test per language checks
  the string against its budget with a per-style advance constant (as `VALUE_ADVANCE` does;
  only two exist today, the builder measures the rest from the bundled fonts). The budget is
  the box width where the layout states one, else the room the layout leaves (for a tab
  label, the tab's width; for a footer, the page width minus its margins), else the current
  Turkish string's width, which the R4 renders proved. A failing translation is shortened,
  never the box.
- **Glyphs.** A test in `plith-render` asks DirectWrite for the glyph index of every character
  of every string in the bundled Public Sans and Martian Mono; none may be 0 (`.notdef`).
- **Renders.** The render example gains `--lang <code>`; one sheet per new language under
  `docs/design/r7/lang-<code>/`, read by the master before merge.

## Left to the builder
- Where `Lang` enters each R4 page builder (inputs struct vs parameter), within the 80-line
  function and 500-line file limits.
- Exact advance constants per text style, measured from the bundled fonts, if a style has none.
- Translation wording, short and in the register of the Turkish and English copy (terse,
  imperative footers, nouns for labels).

## Verification
Five checks (fmt, clippy `-D warnings`, tests, structure-check, check-deps), the fit and glyph
tests green for all eight languages, renders compared by eye, and on screen at the next
console session (with the OSD after the helper reinstall).
