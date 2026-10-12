# The notch in Loft

Date: 2026-10-12 · Status: approved by the owner 2026-10-13

## 1. Why

The notch today paints itself with the user's accent colour. `ThemeService.BuildAccentOverride`
derives a two stop gradient from the picked colour and assigns it to `OsdSurfaceBrush`, and
`NotchSurface` paints with that. A lime accent gives an olive panel, a pink accent gives a pink
one. Everything else on the surface is then computed backwards from whatever colour that turned
out to be: `NotchInk`, `NotchInkMuted`, `NotchTrack`, `OnAccent` and `AccentTrack` all exist to
keep text and bars readable on a surface whose colour is not known until runtime.

praxvon-ui closed this exact question in the other direction. ADR 0014 makes the accent one seed
that derives into four roles, and the rule is that a surface is never filled with it: lines,
borders and markers take `accent-mark` (3:1 on every surface), accent coloured text takes
`accent-text` (4.5:1 on every surface), and the fill is reserved for small filled parts that
carry `accent-foreground` on them. The surface is the floor, and the floor does not move.

Plith is the only Praxvon surface that still floods. Moving it onto Loft makes the two agree and
deletes most of the machinery above, because ink computed against a fixed floor is a constant.

**The earlier decision this reverses, stated plainly.** `OsdPalette.Dark.xaml` records that the
untinted notch was reported as wrong: "the card tints with the picked colour and the notch did
not, so the two surfaces of the same app disagreed about what theme they were in", and tinting
the notch was asked for directly. That complaint is still answered here. Under Loft the card and
the notch sit on the same night floor, so they still agree; they agree on a neutral instead of on
a tint. What goes away is the tint, not the agreement it was introduced to produce.

## 2. What ships

1. **A generated token dictionary.** `Resources/LoftTokens.Dark.xaml` and `.Light.xaml`,
   generated from praxvon-ui's `tokens/base.json` and `tokens/themes/praxvon.json` by a script in
   `scripts/`, the sibling of praxvon-ui's `build-tokens`. Generated rather than hand copied: a
   hand copy drifts the first time either side is tuned, and the drift is silent. Checked in, so
   a build does not need the praxvon-ui checkout.
2. **The accent becomes a mark.** `OsdSurfaceBrush` stops being derived from the accent and
   becomes the Loft floor: `#0B0B0A` to `#151412` on dark, `#E7E6E0` to `#F3F2ED` on light. The
   accent derives into `accent`, `accent-foreground`, `accent-mark` and `accent-text` by the rule
   in `packages/cli/src/accent.ts`: tone walking in OKLCH with hue and chroma held, until the
   mark clears 3:1 and the text clears 4.5:1 against both floors.
3. **The ornament layer.** `+` corner marks on the notch panel, ruler ticks under values that sit
   on a scale (volume, brightness, seek), uppercase mono labels, and `[brackets]` in
   `accent-text` for the selected item in the output picker. Ornaments are tokens, so the light
   theme and the high contrast theme can turn them off without touching a view.
4. **Typography.** Public Sans for body, Martian Mono for labels and values, both OFL, both
   shipped with the app. Every number that changes in place takes tabular figures: the clock, the
   temperature, the elapsed and remaining times, the volume and brightness readouts.
5. **Values settle on marks.** Loft's rule is that no line ends loose and a value settles without
   overshoot. The volume bar, the brightness bar and the seek thumb animate with
   `spring.smooth` (bounce 0) and snap to their step; reduced motion jumps.

## 3. The shape exception

Loft specifies square corners. The notch's bottom two corners are rounded, radius 8, set in
`OsdContent.xaml.cs:324` as `CornerRadius(0, 0, radius, radius)`; the top two are already square
because the panel hangs flush from the top edge of the screen.

**The rounded bottom corners stay.** They are what makes the panel read as a notch rather than as
a rectangle hanging off the top edge, and that reading has been the product's identity since
0.1.6. This is the one place the notch departs from Loft, it is deliberate, and it is written
here so that a later reader does not "fix" it. The `+` corner marks sit inside the corner radius
rather than on it.

## 4. What it costs

Stated here rather than discovered later.

- **The picked accent no longer colours the panel.** A user who picks purple gets a neutral panel
  with purple lines and purple text, not a purple panel. This is a visible behaviour change for
  anyone who set an accent, and it is the point of the change rather than a side effect.
- **Two fonts ship with the app.** Public Sans and Martian Mono add to the installer and need
  their OFL attribution in `NOTICE.md`.
- **The light theme gets louder.** Loft's paper floor `#E7E6E0` is lighter than today's tinted
  light panel, so a notch hanging from the top edge of a light desktop is more visible than it is
  now. If that reads wrong on a screen it is a palette question, not a structural one.
- **Five palette keys lose their reason to exist.** `NotchInk`, `NotchInkMuted`, `NotchTrack`,
  `OnAccent` and `AccentTrack` are all derived from an unpredictable surface. Against a fixed
  floor they become constants, and `ContrastInk.PairOn` and `TrackOn` stop being called on the
  notch path. Removing them is part of this work, not a follow up, or the dead machinery stays
  and the next reader cannot tell which path is live.

## 5. Out of scope

- The Settings window. It has its own theme (`SettingsTheme.xaml`, 740 lines) and moving it is a
  separate piece of work with its own review.
- The shelf surface inside `Plith.DropCatcher`. It receives its palette over
  `ShelfPaletteWire`, so it follows automatically once the wire carries Loft values, but its
  own layout and ornaments are not redesigned here.
- New components. Loft's panel, readout, ruler and sparkline are praxvon-ui wave candidates and
  nothing here waits on them.
- React Native and web. praxvon-ui remains the owner of those.

## 6. How this is verified

Nothing here is claimed until it is rendered and measured. The repo's own lesson applies: a green
build cannot see a wrong shape.

| Check | Instrument | Passes when |
|---|---|---|
| Every surface in both themes and three accents | `scripts/render-widgets.ps1` | The panel is the floor colour at every accent, not a tint of it |
| Text 4.5:1, non-text 3:1 | `scripts/check-contrast.ps1` | Passes on both floors for every preset accent and for a custom one |
| No glyph font, every control named | `scripts/check-a11y.ps1` | Unchanged pass after the ornament layer is added |
| Shared XAML names no assembly | `scripts/check-shared-xaml.ps1` | Passes, including the generated token dictionary |
| The real notch still opens and pages | `scripts/drive-media-page.ps1` | Unchanged pass |
| The open still costs what it cost | `scripts/measure-notch-open.ps1` | No regression against the figure in `PERF-VERIFICATION.md` |
| Idle cost unchanged | `scripts/measure-idle-cost.ps1` | No new timer, no new wakeup |

The accent derivation gets a unit test of its own: for each of the six praxvon presets and a
sample of custom colours, the derived mark clears 3:1 and the derived text clears 4.5:1 against
both floors. That test is the reason the derivation can be trusted without a person checking
every accent by eye.

## 7. High contrast keeps its own palette

Decided 2026-10-13, owner's call on the recommendation.

**The high contrast theme does not take Loft.** It already opts out of the accent entirely:
`BuildAccentOverride` returns an empty dictionary when `IsHighContrast`, so nothing in section 2's
accent work reaches it. Its colours come from the user's own Windows high contrast scheme, and
Loft's floors and ornaments would be a second opinion about colours the operating system has
already decided, on behalf of the one group of users who cannot work around a wrong answer.

So `OsdPalette.HighContrast.xaml` is left alone, and the ornament tokens resolve to their off
values there. The practical consequence for the work: every check in section 6 runs on three
themes, and high contrast passes by being unchanged rather than by being ported.
