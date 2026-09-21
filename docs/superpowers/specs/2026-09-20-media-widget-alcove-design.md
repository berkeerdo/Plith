# Notch Media Page: Alcove Redesign

**Date:** 2026-09-20
**Branch it builds on:** `feature/shelf-drop-catcher`
**Spec this refines:** `docs/superpowers/specs/2026-09-07-notch-widget-carousel.md` (§ the media page)

## Why this change has the shape it has

Two things are wrong with the media page, and they are different kinds of wrong.

The first is that it reads as a list row. A 46 DIP thumbnail, two lines of text and three
small buttons on a rail is the shape of one entry in a playlist, and this page is not an
entry in anything: it is the one thing playing. The page's own comment argues this and then
builds the list row anyway, compensating with a blurred copy of the artwork behind a scrim.

The second is that the compensation costs more than it returns. The blurred backdrop forces
the page to declare its own ink (`NotchInk` and `NotchInkMuted` are overridden in
`MediaWidget.xaml`), because its ground is dark in both themes while every other page's
ground follows the theme. So the page is an exception to the surface it sits on, it is
invisible to `check-contrast.ps1` for that reason, and the blur is drawn from a bitmap
decoded at `DecodePixelWidth = 96` and then stretched across 356 DIP.

### What Alcove actually does, measured

Alcove's expanded now-playing panel, measured from the application's own press screenshot on
2026-09-20, is three rows on a **plain black ground with no artwork tint at all**:

1. A ~64 px rounded album tile, left. Bold title, grey artist beside it. At the far right of
   the row, four accent-coloured bars: a live waveform, added in Alcove 1.7 (June 2026).
2. Elapsed time left, progress bar centre, **remaining** time right.
3. Five centred controls: favourite, previous, a larger play/pause, next, output device.

The panel's aspect ratio is roughly 2.16:1. Plith's widget frame is 356x116, which is
3.07:1. Alcove's media shape is substantially squarer than the frame this page has to live in,
so the layout cannot be copied row for row. It has to be re-laid-out for a wider frame.

### The two constraints this design is built around

**The frame does not change.** `NotchGeometry.OpenFrameDip` is 356x116 and its comment
records that letting the page drive the size was tried twice and rejected on the mockup:
varying both dimensions made the notch appear to jump around while swiping, and varying only
the height still moved the bottom edge on every page turn. A taller media page would re-open
a decision already taken. The content is therefore re-laid-out into the frame, not the frame
into the content.

**There is no position data today.** `MediaSnapshot` carries five fields (title, artist,
thumbnail bytes, playing, has-session) and none of them is a timeline. A progress bar is not
a view change: it needs SMTC's `GetTimelineProperties()` plumbed through the client, the
snapshot and the view model.

## Goal

A person opens the notch while something is playing and lands on a page that looks like the
one thing playing: a large crisp album tile, the title and artist, where in the track they
are and how much is left, and the controls to do something about it. The page uses the same
themed surface as every other page, so it is no longer an exception to the theme or to the
contrast lint.

And they land on it without swiping: when something is playing, the media page is the page
the notch opens on.

## Decisions taken, and why

1. **The frame stays 356x116.** See above. Alcove's vertical stack is opened out
   horizontally instead, using the width Alcove does not have.

2. **The artwork tint goes, and the page moves onto the themed surface.** This is the
   opposite of compensating for a small tile: the tile gets bigger and the ground gets
   honest. It deletes the `NotchInk`/`NotchInkMuted` overrides, ends the page's light-theme
   exception, and puts its colour pairs in front of `check-contrast.ps1` for the first time.
   It is also what Alcove does.

3. **The album tile is 56 DIP, not 64.** 64 was the first number, taken from Alcove. It does
   not fit: the content band is 73 DIP tall and a 64 tile plus the progress row overflows it.
   56 is the largest tile that leaves the progress row its space, and it is still 10 DIP
   larger than today's.

4. **Thumbnail decode goes from 96 px to 192 px.** A 56 DIP tile is 112 px at 200% DPI, so 96
   is already short of the tile this design draws, never mind the 300% case. 192 covers both.
   The cap exists to bound memory per track change and stays a cap.

5. **The progress row spans the full content width, below the tile row.** Putting the bar in
   the middle column beside the tile was the first shape and it loses twice: the bar becomes
   ~116 DIP long, and it shares a 116 DIP column with two lines of text and two time labels.
   Full width is also how Alcove reads.

6. **Elapsed and remaining, not elapsed and total.** Alcove's choice, and the more useful of
   the two: "how much is left" is the question a person asks at a glance.

7. **When the duration is unknown, the bar and both labels are not drawn.** Live streams
   report no end time. A bar of unknown length is a lie, and a zero-length one is worse.

8. **The bar is a templated `ProgressBar`, not two `Border`s.** Not cosmetic: `ProgressBar`
   has an automation peer with a value, so the position reaches a screen reader.
   `NotchHud`'s level bar uses `Border`s and stays as it is, because it is on screen for two
   seconds and this page is held open.

9. **A fourth control, output device, joins the rail and opens the Windows sound page.**
   Changing the default output device has no documented API; it is done through the
   undocumented `IPolicyConfig` COM interface. An in-notch device list is therefore its own
   slice. The button does the honest cheap thing instead: `ms-settings:sound`.

10. **When something is playing, the notch opens on the media page.** Not when a session
    merely exists: a Spotify left open and paused for days would lock the notch onto the
    media page and the clock page would never come first again. Playing is the condition
    because the opening page should be what is happening now.

11. **The opening page is computed at open time, never stored.** `NotchPager.Reset()` exists
    because the carousel spec deferred remembering the last page, and its comment records
    why the deferral was kept: a value written in a completion handler is the exact hazard
    that produced five defects on this branch. The rule keeps that property. It becomes
    `ResetTo(index)` with the index computed on the way in.

12. **The rule lives in a pure class, `NotchOpeningPolicy`, not in `OsdHost`.** `OsdHost` is
    a `BandWindow` the test project cannot construct, which is how two previous versions of
    the event rule reached a running build with no test between them. `NotchEventPolicy` was
    extracted for exactly this reason; this rule follows it.

13. **A file landing on the shelf still wins.** `ShowShelfLanding` names the page it wants.
    An event that names a page outranks a rule that guesses one.

## The design

### Layout

Vertical budget, from today's page: 14 DIP top padding, a **73 DIP content band**, 29 DIP
below (20 of which is the page rail's fixed lane, 9 the gap above it).

```
 356 x 116
+--------------------------------------------------------+   14  top padding
|  +--------+  The Garden                 |< >|| >|  ()) |   +
|  |  ART   |  Faithless                                 |   |  56  row 1
|  |   56   |                                            |   +
|  +--------+                                            |    3  gap
|  2:27  [============------------------------]   -2:00  |   14  row 2
+--------------------------------------------------------+
|                      -  ---  -                         |   20  page rail lane
+--------------------------------------------------------+
```

Row 1 columns: `18 + 56 (art) + 12 + 116 (text) + 136 (rail) + 18 = 356`.

The rail is four 28 DIP round buttons: previous, play/pause, next with 6 DIP gaps between
them (96 DIP), then a 12 DIP separator, then output (28 DIP). Play/pause keeps its brighter
fill; the separator is what stops output reading as a transport control.

Row 2 spans the full 320 DIP content width: `34 (elapsed) + 8 + 232 (bar) + 8 + 38
(remaining)`. The bar is 4 DIP tall, radius 2, track `NotchTrack`, fill `OsdAccent`.

The text column loses 32 DIP (148 today, 116 here) to the fourth control. The title already
scrolls rather than ellipses, so nothing is hidden, but it will scroll more often. Recorded
as a measured cost of decision 9, not as a detail.

The art and the text stay one click target that opens the source application
(`OpenSourceArea`), and the transport stays outside it, unchanged.

### Timeline data

`MediaSnapshot` gains three fields: `Position`, `Duration`, `LastUpdated`, taken from
`GlobalSystemMediaTransportControlsSession.GetTimelineProperties()`.

**The trap this design is built around:** `TimelinePropertiesChanged` fires about once a
second on some sources. `MediaSessionClient.ScheduleEmit()` re-reads the media properties
**and re-downloads the thumbnail**, so subscribing the timeline to that path would download
album art once per second. The timeline therefore emits on its own event,
`TimelineChanged`, carrying only the timeline and refreshing no properties. `Changed` keeps
carrying a full snapshot, timeline included, so a subscriber that only wants one value is
never forced to hold two sources of truth.

Position updates are sparse and arrive stamped, so the bar's movement is interpolated rather
than polled:

```
MediaProgress.Elapsed(position, lastUpdated, duration, isPlaying, now)
```

A pure static, because the suite is not STA and pure code is the only part of this feature a
test can reach. Cases it must get right: duration zero or absent, position past duration,
paused (frozen at `position`, no drift), `lastUpdated` in the future (clock skew: clamp to
`position`), negative interpolated values.

The widget ticks a 1 Hz `DispatcherTimer` and asks that function, **only while the page is
visible**. 1 Hz is enough for both halves: the seconds text changes at 1 Hz, and a 232 DIP
bar over a four-minute track advances about one DIP per second. The timer follows the
existing `IsVisibleChanged` pattern in this file, which is there because a page that has been
away misses every notification while it is off the tree.

### Ground, art and the page's own colours

Removed from `MediaWidget`: the `Backdrop` image, its `BlurEffect`, the scrim `Rectangle`,
`BackdropOpacity`, `RenderBackdrop()`, the `NotchInk` and `NotchInkMuted` overrides, and the
`BottomRoundedClip` applied on `SizeChanged`. The clip exists so a page that paints to the
frame's edges takes the notch's outline; this page stops painting a ground, so it stops
needing one. `NotchGeometry.BottomRoundedClip` stays, because `WeatherWidget` still paints a
sky and still needs it.

Kept: the album tile's 1 px inner hairline (photographic art with dark edges dissolves into
the panel without it), the track-change animation, and the no-session state (placeholder
tile, "Nothing playing", transport disabled, and now also no progress row).

### Output device button

Reuses the existing `IconSpeakerBody` plus `IconSpeakerWaveInner`/`IconSpeakerWaveOuter`
geometries. No new icon is drawn, and no `Segoe MDL2` code point is introduced: the
accessibility lint fails the build on those, in code-behind as well as XAML.

Accessible name: "Change output device". It launches `ms-settings:sound`. The compromise is
stated in the UI's own terms: the button opens the system's sound settings, not the quick
settings output picker, which has no documented way in.

### The opening page

```
NotchOpeningPolicy.OpeningPage(bool isPlaying, int mediaPageIndex)
    => isPlaying && mediaPageIndex >= 0 ? mediaPageIndex : 0;
```

`ApplyWidgetPages` already tracks `_shelfPageIndex` as it builds the list; it gains
`_mediaPageIndex` beside it, from the same list, so the two cannot disagree about an order
they both read from one place.

`OnNotchClicked` replaces `_pager.Reset(); _widgets.SyncToPager(0);` with a reset to the
computed index. `NotchPager` gains `ResetTo(int index)`, which clamps into range and forgets
the last gesture's timing exactly as `Reset()` does; `Reset()` becomes `ResetTo(0)`.

This is the only call site that changes. Measured on 2026-09-20: the widget frame is opened
in exactly two places, `OnNotchClicked` and `ShowShelfLanding`, and the second names its own
page.

## What gets deleted

| Piece | Why it goes |
|---|---|
| `Backdrop` image + `BlurEffect` + scrim `Rectangle` | The page no longer paints its own ground |
| `BackdropOpacity`, `RenderBackdrop()` | Nothing left to fade |
| `NotchInk`/`NotchInkMuted` overrides | The ground follows the theme, so the ink can too |
| `Clip = BottomRoundedClip(...)` in `MediaWidget` | Only pages that paint to the edges need it |
| `_pager.Reset()` at the click site | Replaced by `ResetTo(openingPage)` |

## Testing

Unit tests, all on pure code:

- `MediaProgress.Elapsed`: the six cases listed above, plus a normal mid-track read.
- `NotchOpeningPolicy.OpeningPage`: playing with a media page, playing with no media page
  (`-1`), not playing, and no session.
- `NotchPager.ResetTo`: in range, past the end, negative, and that it forgets gesture timing.

`MediaSessionClient`'s own read is deliberately **not** in that list. It needs a live SMTC
session, so there is no seam a unit test can reach without inventing one for its own sake.
It is covered by the hardware run below, and by the fact that a wrong read shows up as a bar
that does not move.

Checks and harnesses, which is where this branch's defects have actually been found:

- **`render-widgets.ps1` gains the media page in three states** (playing with a long title,
  paused, no session) **in both themes.** This is the only way to see the layout without a
  person, and every layout defect on this branch was found by looking at a render.
- **`check-contrast.ps1` measures this page for the first time.** Removing the ink overrides
  is what makes it visible to the script. Any pair that fails is a real finding, not a
  regression introduced here.
- **`check-a11y.ps1`** covers the new output button's name and the progress bar. The bar is a
  `ProgressBar` partly so this check has a peer-bearing element to land on.
- **`drive-shelf-pair.ps1` gains the opening-page check:** with something playing, click the
  notch and assert from the UIA tree that the first page shown is the media page. This is
  runnable by script, not by a person only: Debug Plith runs at Medium integrity (measured
  2026-09-19), so UI Automation reads its tree and `SendInput` drives it.

## Risks and open items

1. **The title column at 116 DIP is the layout's weakest number.** It is a consequence of the
   fourth control. If the render shows it reading badly, the cheapest correction is the 12 DIP
   separator, then the tile at 52. Decide it on a render, not here.

2. **`TimelinePropertiesChanged` behaviour varies by source.** Some apps never fire it, some
   fire it constantly, some report a position that only moves on seek. The interpolating
   design tolerates all three, but the bar may sit still on a source that reports nothing.
   The 1 Hz tick and the sparse-update interpolation are the mitigation; a source that reports
   no timeline at all falls into the "no duration, no bar" case, which is honest.

3. **`ms-settings:sound` opens a full Settings window.** It is a heavier answer than Alcove's,
   and if it reads as too heavy the fallback is to drop the button rather than to reach for
   `IPolicyConfig`.

## Out of scope, with reasons

- **The live waveform.** Alcove's four accent bars decorate a physical camera notch. Windows
  has no such notch, so the thing the decoration is in dialogue with does not exist here. It
  is also the most expensive item: there is no level metering anywhere in this codebase, so it
  would need `IAudioMeterInformation` (or the Voicemeeter level API) plus a 30 Hz timer, and
  deriving four bars from one peak value would be half invented.
- **Seek.** The notch closes after about 2.6 seconds, so a drag can be cut off mid-gesture,
  and SMTC position writes are not supported by every source. `MinSeekTime`/`MaxSeekTime`
  arrive in the same timeline call this slice already makes, so seek stays additive. No unused
  field is added for it today.
- **An in-notch output device list.** Undocumented `IPolicyConfig`, a popup surface the notch
  does not have, and separate Voicemeeter logic. Its own slice.
- **Favourite (Alcove's star).** SMTC exposes no such command.
- **A live-stream indicator.** Alcove marks streams and podcasts; here a stream simply shows
  no progress row. Worth revisiting when the no-duration case has been seen on hardware.
