# 0015 Eight languages from one exhaustive table, the OSD word over the pipe

Date: 2026-11-18. Status: accepted. Decided by the master; Berke left the language set to its
pick ("önerin hangisi ise uygula"). Spec `specs/2026-11-18-r7-languages-design.md`, plan
`plans/active/r7-languages.md`. Extends ADR 0014 Decision 6.

## Context
R7a shipped Turkish and English in `plith-core::lang`, but the R4 pages, the strip fallback
and the OSD mute word were still hard-coded Turkish in `pages/copy.rs`, so an English user saw
Turkish tabs and footers. The percent was `%62` everywhere. The settings window will offer a
language choice, so every string had to come from one table first.

## Decision
1. **Languages:** `en tr de es fr pt it pl`, Latin script only. Russian, Greek and CJK need a
   font fallback and double-width fit rules: a later plan.
2. **Table:** one Rust file per language with an exhaustive `match` returning `&'static str`.
   A language missing a string does not compile; English is the fallback only for an unknown
   language code. Runtime resource files (Fluent, JSON) were rejected: a parser dependency and
   file I/O for no translator community yet.
3. **Every own string** comes from the table, in all page builders and the feed. The feed
   resolves the language once at start for every run mode. Names from Windows, Voicemeeter or
   media are never translated. Own labels are upper-case per language rules; sentence copy
   (shelf drop, media hint, Brightness, Microphone, Unused) keeps its case.
4. **Percent per language:** Turkish `%62`; `en pt it pl` `62%`; `de fr es` `62 %`.
5. **Fit is measured, not counted:** a test lays out every own string with the renderer's own
   DirectWrite formats and checks it against the box from the layout constants. A translation
   that does not fit is shortened; the box never grows. A glyph test checks every character in
   the bundled face of its style.
6. **OSD:** `Message::Level` carries `muted_word` (bounded like `label`); the helper draws it
   and holds no copy of its own. `PROTOCOL_VERSION` is 2, `MAX_MESSAGE` 144.
7. **Fail closed against an old helper:** the pipe stays inbound-only (no server reply, ADR
   0002 and 0009). After `Hello` the client waits `ADMIT` (500 ms) and writes a `Ping`; only
   when that write succeeds is the link up. A v1 helper rejects and drops the client, so the
   link never reads up and flyout suppression never hides the native flyout with nothing drawn.

## Consequences
- The installed helper speaks protocol 1 until `build-osd.ps1` and `install-osd.ps1` run
  (Berke's yes). Until then there is no OSD; the notch works.
- The first level after each connect waits up to 500 ms; a helper stalled longer than that
  before rejecting can read up briefly until the next write.
- Translations are the master's own, not reviewed by native speakers. Spanish keeps `OFF`
  (no native word fits six characters).
- A language change still takes a restart until the settings window.
