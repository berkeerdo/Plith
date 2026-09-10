# Classic OSD and Settings redesign

**Status:** design agreed 2026-09-10 against two live mockups the user drove directly.

**Mockups:**
- Classic OSD — `docs/superpowers/design/classic-osd.html`
- Settings — `docs/superpowers/design/settings-design.html`

**Related:** `2026-09-07-notch-widget-carousel.md` shares the palette, the icon set and the
marquee rule with this document. Where the two overlap, the carousel spec is about the notch and
this one is about everything else; nothing here changes notch behaviour.

---

## Problem statement

Phase 6 gave the notch a bespoke surface. The Classic OSD did not get one, and it is still the
default on every install — and, because `WantsNotch` drops to Classic whenever a window covers
the monitor, **every Notch user is also a Classic user several times a day**. Leaving Classic
alone would mean half the sessions of the "new" presentation are the old one.

**Checked against the code, 2026-09-10, and one of the three claims below was mine rather than
the code's.** They are corrected in place rather than quietly dropped.

1. ~~**The level track is invisible.** 2 DIP at low opacity.~~ **Withdrawn — false.**
   `AudioCardView.xaml` already draws a **6 DIP** track with a `#22FFFFFF` ground and a
   full-strength fill. It was asserted from the mockup's narrative, not read from the file. The
   user's sentence behind it — *"classic osde ne değişti track göremiyorum"* — reads at least as
   naturally as *"I can't see what changed in Classic"*, which is a request for the pass rather
   than a report of an invisible bar.

   What is left of it is real but smaller: the track has **no threshold tick**, so the colour
   change at 85 % has no visible cause, and the fill has no glow, so it sits flat against the
   card. Those are additions, not a rescue.
2. **The icons are borrowed.** Confirmed: `MediaCardView.xaml` binds
   `Segoe Fluent Icons, Segoe MDL2 Assets`. The set is also not ours to shape — seven of ten
   weather glyphs chosen from it during slice 2 turned out not to exist in the font, which is
   what the `GlyphTypeface.CharacterToGlyphMap` test now exists to catch.
3. **The device name runs on.** Confirmed, and easier to fix than assumed. The card's label comes
   from `device.FriendlyName` raw (`WindowsAudioClient` line ~285), while the settings dropdown
   already runs the same string through `ShortenFriendlyName`. There is nothing to write: the
   card simply does not call the shortener that exists.

**So the honest case for redoing Classic is not a defect list.** It is that Classic never got
the pass the notch got, it is the default on every install, and `WantsNotch` drops every notch
user into it several times a day.

Settings has its own problem, and it is structural rather than cosmetic: it is one long scroll
in config-file order, so a person adjusting how the notch looks scrolls past autostart and
hotkeys to reach it — and every setting on the page changes something they cannot see from the
words alone.

## Goals

1. A level a person can read at a glance.
2. An icon set that belongs to this product and cannot go missing on someone else's machine.
3. Three signal colours that mean three things, kept separate from the accent preference.
4. Settings grouped by what someone came to change, with a live preview.
5. The Notch → Classic fallback stated on the page rather than discovered.

## Non-goals

- Changing when the Classic OSD appears, where it is positioned, or what cards it hosts.
  `CardHost` remains the single authority for visibility.
- A settings search box. The page is not long enough to earn one once it is grouped.
- Per-monitor Classic positions.

---

## §1 — The card

One width in every state: **300 DIP**, set by the media row rather than the bare volume one. At
268 the media row left the title ~98 dip once the art and the transport had taken theirs, which
is narrow enough that the marquee would run on almost every track — and a marquee that always
runs is just a moving ellipsis. Compact is 224.

States, all drawn in the mockup: normal, past 85 %, muted, with media, compact.

| # | Decision | Why |
|---|---|---|
| 1 | Speaker icon has three shapes — full, low below 33 %, crossed when muted | The icon alone answers "is my sound off?" without reading a number |
| 2 | Card label runs through the existing `ShortenFriendlyName` | The settings dropdown already does; the card does not. No new logic. |
| 3 | A bus line — `Voicemeeter · A1`, `Windows · Output`, `Muted` | On a machine with Voicemeeter plus Sonar plus a headset, a bare number is ambiguous |
| 4 | Level as a number, tabular figures, `%` set smaller | The digits must not shuffle as the value changes |
| 5 | Track keeps its **6 DIP**, gains a glow on the fill | It is already 6 — see the correction above. The glow is what lifts it off the card |
| 6 | Card width set by the fullest row | Same rule as the notch's fixed frame |
| 7 | Long titles scroll, they do not ellipse | See §3 |
| 8 | A hairline tick at 85 %, drawn only near it | Gives the colour change a cause instead of looking like a glitch |

## §2 — Colour

| Token | Value | Means |
|---|---|---|
| Signal | `#4AD695` | level under 85 % — the resting state |
| Caution | `#F5C242` | 85–99 % — loud, still fine |
| Alarm | `#E0674F` (dark) / `#DC2626` (light) | 100 %, or a Voicemeeter bus clipping |
| Bezel | `#06070A` | the notch's resting shape. Never a card ground. Constant across themes. |
| Notch ink | `#F2F5F8` / `#9AA6B2` | text drawn **on** the bezel — the widget pages and the HUD |
| Surface | `#151A21` | card ground, over Mica |
| Muted ink | `#8D9AA8` | device, artist, bus. Never the level. |

**The bezel forces its own ink.** Anything drawn on the notch needs light text in both themes,
because the surface under it is near-black in both. The theme's own `OsdTextPrimary` is near-black
on the light theme, which on a near-black bezel is not a contrast problem so much as an invisible
panel. That is the price of the bezel being a cutout rather than a surface, and it is why
`NotchInk`, `NotchInkMuted` and `NotchTrack` exist alongside it. High contrast is the exception
and keeps the system's own pair: the point of that mode is that the user's colours are the ones on
screen.

**The accent setting stays separate.** A purple accent gives purple card chrome, not a purple
level, because the level's colour is information. The single exception: when
`UseColorThresholds` is off the fill takes the accent and the thresholds stop applying — which
is the honest reading of what turning that setting off means.

**Second correction, 2026-09-10.** The plan said the alarm colour moves to `#E0674F` "in all
three OSD palettes". It moved in the dark one only. `#E0674F` was picked against a near-black
card; on the light palette it loses contrast, and the light theme's existing `#DC2626` is the
right value there. A colour chosen for one ground is not a token, it is that ground's value —
and applying it to the other one would have been following the spec instead of the design.

Two more things this section was wrong about. `AccentBrush` in `Theme.xaml` was a fourth
definition of the same green and was **referenced by nothing**; it is deleted rather than
reconciled. And `#E54B4B` in `SettingsTheme.xaml` is the close button's hover state, not the
alarm colour — they share a literal by coincidence, and tying them together would make a theme
tweak to one silently change the meaning of the other. Left alone, with a comment saying so.

**Correction, 2026-09-10.** An earlier draft of this section claimed the threshold colours were
literals in two files that had drifted apart. They are not: `OsdGainGreen`, `OsdGainAmber` and
`OsdGainRed` are already named brushes in all three OSD palettes. What is actually duplicated is
`#4AD695` itself, defined separately as `Accent`, `AccentBrush` and `OsdGainGreen` — three roles
sharing one value by coincidence rather than by reference, which will drift the first time one of
them is tuned. Worth fixing, but it is a different and smaller job than the one claimed.

## §3 — Long titles scroll

Identical rule to the notch, and deliberately one implementation:

- **Conditional** — measured overflow starts it, anything that fits sits still.
- **Constant speed, `28 dip/s`**, not a fixed duration. A fixed duration makes a long title fly
  and a short one crawl; that is the tell of a marquee nobody measured.
- **Ease-in-out with alternate**, so it dwells at both ends instead of snapping back under the
  reader's eye.
- **Re-measured on `SizeChanged` and on track change**, never cached. A title that changes while
  the card is up is exactly the case a one-shot measurement gets wrong — see §7.
- **Stops on dismiss and under reduced motion.**

## §4 — Icons

Nine `PathGeometry` resources drawn for this product: 1.6 stroke on a 24 grid, round caps and
joins, fills only where a shape must stay readable at 13 px — the transport marks, which become
mud as outlines at that size.

Replacing the font is not only aesthetic. A `FontIcon` bound to `Segoe MDL2 Assets` depends on a
font whose contents differ between Windows builds; a `PathGeometry` cannot go missing. The
accessibility lint gains a rule that fails the build on any remaining `Segoe MDL2 Assets` under
`Views/`.

## §5 — Motion

| Moment | What moves | Timing |
|---|---|---|
| Appear | opacity 0 → target, plus an 8 DIP rise from the edge it sits on | 180 ms cubic out |
| Level change while shown | the fill width only; the card does not re-enter | 120 ms cubic out |
| Threshold crossed | fill and number colour, together | 140 ms linear |
| Long title | the text, only when it does not fit | 28 dip/s, 1.2 s delay |
| Media card arrives | card height, row fading in behind it | 200 ms cubic out |
| Dismiss | opacity only — a card sliding away reads as dragged | 160 ms cubic in |

Classic appears and disappears; it does not morph. That is the whole difference from the notch,
and these timings are what keep it that way.

## §6 — Settings

### Structure

Two groups in a left rail: **Overlay** (Appearance, Audio, Media) and **App** (Theme, Shortcuts,
Behaviour). The old page was one scroll in config-file order.

### Dependency is shown, not toggled

Notch-only settings sit **indented under Style with a rail beside them**, rather than appearing
and vanishing as the style changes. Controls that disappear teach people the page is
unpredictable; controls that are visibly subordinate teach them the rule.

### The fallback is stated on the page

A banner under Style: the notch steps aside when a window covers the screen — a full-screen
game, a borderless browser, a full-screen video — and comes back afterwards. The preview shows
it rather than describing it, and the Style setting stays on Notch throughout, which is the
distinction the banner exists to make.

### Copy is named by what happens

*On screen for*, not `ShowDurationMs`. *Colour the level*, not `UseColorThresholds`. Each
subtitle says what changes, and where a setting has a threshold worth knowing — 85 %, 50 % — it
says the number instead of hiding it.

### The preview is the point

Every setting on this page changes something invisible from the words alone, and the old dialog
made a person close it, press a volume key and reopen it to find out. Style and resting height
drive the preview live.

### No save button

Changes apply as they are made, which the app already does. A save button on a page that
autosaves is a lie; one that does not autosave is a trap.

## §7 — What the automated gates cannot see

Everything above renders in a layered window that no capture path can photograph over RDP, and
the xunit suite is not STA so it cannot construct a `UserControl`, load a template, or receive a
mouse message. Slice 2 shipped green on build, tests and the lint, then crashed on first hover.

Open until a physical console session confirms them: the track's contrast over a bright
wallpaper, the icons at 100 % and 150 % DPI, the name shortening against the real G733 string,
the threshold tick appearing when it should, the marquee starting only when it must and stopping
on dismiss, and the Settings preview tracking the live setting.
