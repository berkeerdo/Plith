# Phase 6 — Manual Verification

Phase 6 (notch shell) is landing task by task on `feature/phase-6-notch-shell`. This file
tracks the checks that need a person, the same way `docs/PHASE5-VERIFICATION.md` does, and
for the same reason: no test in this repository can observe a rendered pixel or a running
animation, so an unrun check must never be recorded as a pass.

This entry covers **Task 6, Task 7, Task 8 and Task 9** — `AmbientNotchPresentation`, the
`OsdHost` mode switch, hover-to-descend via `NotchHoverPoller`, the covers-monitor signal from
`FullscreenVideoWatcher` and the Classic fallback it drives, and the Settings UI's presentation
picker, strip-height slider, and position-edit guard. **Task 10** adds sections 6, 7 and 8 below, closing out the
three checks listed in the design spec's §8 that no earlier task recorded: Snap
Layouts/auto-hide-taskbar interference, an idle resource measurement with the window never
hidden, and whether the descent reads as motion on both a 60 Hz and a high-refresh display.
Task 10 is documentation only — it did not run any of these checks either, for the same reason
every other section here is open: no agent may launch, focus, or drive the running app.

---

## 0. What is automated as of Task 8

> **Note:** the figures below were accurate when Task 8 landed. Task 9 (Settings UI) and the
> entire notch-home-view slice landed afterward and added far more coverage; the current count
> is recorded here so this section stays true of the tree rather than of a moment in its
> history.

- `dotnet build src/Plith/Plith.csproj -c Debug` — 0 warnings, 0 errors.
- `dotnet test tests/Plith.Tests/Plith.Tests.csproj` — 218/218 passing as of the current tree
  (161/161 as of Task 8 itself; `FullscreenVideoDetectorTests` was unchanged by Task 8, since it
  does not touch `ShouldSuppress`'s signature or behaviour). The count fell from 162 to 161
  when a tautological `NotchGeometry` test — two identical calls asserted equal — was deleted
  rather than left standing as coverage of a property it never exercised; it climbed to 218
  afterward as Task 9 and the home-view slice each added their own tests.
- `pwsh -File scripts/check-a11y.ps1` — exit 0.

None of the above exercises `AmbientNotchPresentation` itself: both it and `ClassicPresentation`
hold a `BandWindow`, which is a `ContentControl` behind a native `HwndSource` that the headless,
non-STA test suite cannot construct. `PresentationPolicy` — the pure predicates the two adapters
delegate to — is unit-tested, but the adapters' WPF plumbing (the animations, `BeginAnimation`
calls, `SetNotchLook`, `SetNotchMetrics`, and the `OsdHost.ApplyPresentationMode` wiring) is not.

Task 7's `NotchHoverPoller` holds a `DispatcherTimer` and P/Invokes `GetCursorPos`, so it is in
the same boat: the headless suite cannot exercise it end to end either. The pure geometry it
calls — `NotchGeometry.HoverRect`, `NotchGeometry.PhysicalToDip`, `NotchGeometry.IsInsideNotch`
— is unit-tested; the timer, the P/Invoke, and the `OnNotchHoverChanged` wiring in `OsdHost` are
not.

---

## 1. The expansion — the notch's headline behaviour

> **Status: NOT VERIFIED. Rewritten after the second live session.**
> The checks below replace an earlier set written for a design that has since been removed.
> That design parked a full-width strip over the card and translated the card down from
> behind it; the user's verdict on a running build was that it did not read as a notch, and
> the spec's §2 now records why. The current model is one surface that changes size. None of
> it has been run: no build from this branch has been launched, focused, or interacted with
> by an agent — manual GUI verification in this project is a human step, and an agent
> attempting it has previously caused real harm.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under
`[Osd]`, restart Plith, then press a volume key.

| # | Check | Pass | Fail |
|---|---|---|---|
| 1.1 | At rest, before any key press | A narrow shape (190 DIP wide, `NotchStripHeightDip` tall) sits flush with the top edge of the target monitor, horizontally centred, with no margin above it and no shadow below it | Nothing visible; a shape spanning the full panel width; a visible top margin; a grey blur under a 2 DIP shape |
| 1.2 | Press a volume key | That same shape **grows** into the full panel over ~340 ms, decelerating into its final size; the card content is absent for roughly the first half and fades in as the shape settles | The panel appears at full size instantly; a card slides down from behind a strip; the content is legible while the shape is still moving; two outlines or two shadows are visible at once |
| 1.3 | After the hide timer elapses | The panel shrinks back to exactly the 1.1 shape over ~260 ms, accelerating, with the content fading out first | The panel vanishes instead of shrinking; it overshoots; a seam or a leftover card edge is visible at the end |
| 1.4 | Repeat 1.2–1.3 with a media session playing (wider, taller panel) | The resting shape in 1.1 is **pixel-identical** — same width, same height, same horizontal centre — and the panel simply grows to a different size | The resting shape widens, or its centre moves, when the media card appears |
| 1.5 | Toggle `Presentation` to `ClassicOsd` while the notch is at rest, then back | Each switch settles cleanly into that mode's rest state: Classic is a rounded floating card at the user's own anchor with its shadow, the notch is the 1.1 shape again, and neither shows the other's outline | Classic appears at opacity 0 or with no shadow; the notch reappears at the wrong size, or with `CardSurface`'s border drawn on top of `NotchSurface` |

**Why 1.4 is the sharpest check here.** It is the one the previous design failed on a running
build, and it failed silently: the resting strip was stretched across the window, the window's
width follows the card, so a media session appearing changed the resting shape's width and the
anchor with it. Nothing in a build, a test run or the lint could see that. The fix is that the
collapsed width is now a constant rather than derived from the window
(`NotchGeometry.CollapsedWidthDip`), and `SurfaceSize_TheRestingPillDoesNotFollowThePanelWidth`
covers the geometry — but only 1.4 covers the thing on screen.

**Why 1.5 matters:** `OsdHost.ApplyPresentationMode` is the piece with no equivalent in
Phase 5 — it tears down the previous presentation's state (clears the expansion animation,
resets `NotchExpand` to 0) before building the new one, then, for the notch branch
specifically, re-measures content and calls `Park()` to settle without an animated flash.
It also flips `CardSurface` between painting and not painting: in notch mode `NotchSurface`
is the only visible surface, and if that switch leaked, Classic would show a borderless
shadowless card or the notch would show two stacked outlines. That teardown/rebuild path only
runs when a live settings change actually flips `SettingsModel.Presentation`, which is exactly
the kind of runtime transition the automated suite cannot reach
(`SettingsService.Changed` firing while a `BandWindow` is live).

**First open question — CLOSED, and the mechanism that caused it no longer exists.** This
document previously asked whether the parked strip should be the dedicated `NotchStrip` border
alone, or that border plus a sliver of the parked card's own bottom edge (the old
`RestingOffset` put the card's bottom edge at `y = stripHeight`, so both drew in the same
band). The user reported the doubled edge directly on the first live run: it read as the
corner of a card poking out from under the top of the screen, not as a notch. That whole
translate-a-card-behind-a-strip design is gone. The current code
(`src/Plith/Views/OsdContent.xaml.cs`, `src/Plith/Views/Presentation/AmbientNotchPresentation.cs`)
has no `NotchStrip`, no `RestingOffset`, and no `HiddenOffset` — grep confirms none of the
three exist anywhere in `src/Plith`. There is one surface, `NotchSurface`, whose `Width` and
`Height` are written directly from a single `NotchExpand` progress value
(`OsdContent.ApplyNotchExpand`), and the card content (`SlidingRoot`) is hidden by fading its
`Opacity` to 0, not by moving it off screen. A translated element's edge poking out from behind
an offset is not a failure mode this design can produce, because nothing is translated. Nothing
below re-opens this.

**Second open question.** The original question was whether the card's drop shadow could bleed
past its parked position, and whether it could be clipped at the bottom of the descended card.
The card-parking half of that question no longer applies — there is no parked card to bleed
from — but the underlying shadow-versus-reserved-space concern still has a live analogue in the
current single-surface design, so it is restated in those terms rather than dropped:

- *Bleed at rest — CLOSED by construction, not yet observed.* `NotchSurface`'s own shadow
  (`NotchShadow` in `src/Plith/Views/OsdContent.xaml`) has its `Opacity` driven by
  `NotchGeometry.Lerp(0, NotchShadowOpacity, t)` in `ApplyNotchExpand`, and at rest `t` is
  exactly `0`, making that `Opacity` exactly `0` too — an effect at zero opacity renders
  nothing, which is a stronger guarantee than "moved off screen and probably not visible." This
  is an argument from the code, not something watched on a running build, so it is worth a fast
  glance during 1.1 and after 1.3's retraction (does the band around the resting pill show any
  glow at all?), but it is no longer treated as an open risk the way the original question was.
- *Clipping while descended — still open, numbers corrected.* `NotchShadow` is `ShadowDepth="6"`,
  `Direction="270"` (straight down), `BlurRadius="20"` — not 28; 28 belongs to `CardShadow`,
  which is Classic's shadow and is inert (`Opacity = 0`) whenever `SetNotchLook(true)` has run.
  `NotchSurface`'s expanded height is deliberately `ContentInsetDip` (14 DIP) shorter than the
  card content's own measured height (`OsdContent.SetNotchMetrics`), which is what reserves the
  vertical gap for this shadow inside the outer `Grid`'s `ClipToBounds="True"` clip. Whether a
  6 DIP shadow depth plus a 20 DIP blur radius actually fits inside that 14 DIP gap, or gets cut
  off in a straight line at the clip edge instead of fading out, is a real question a 20/6/14
  arrangement does not answer on paper — it needs eyes on a running build. During 1.2, look at
  the bottom edge of the open panel against a light background: does the shadow fade, or does
  it stop abruptly?

  Deliberately not fixed by this round even if it turns out to clip: the fix is not a one-liner
  (the blur radius, shadow depth, and `ContentInsetDip` all have to move together, and getting
  the new numbers right is a visual judgement), and no build from this branch has been run
  since the shadow values above were last touched.

Record what is actually seen, not what should theoretically happen.

**Known environment limitation, carried over from Phase 5:** over an RDP session the band
window's layered surface cannot be captured by any means (`BitBlt`, `BitBlt` with
`CAPTUREBLT`, and `PrintWindow` with `PW_RENDERFULLCONTENT` were all tried and failed for the
Classic case in Phase 5). If this check is attempted over RDP, expect the same — run it from
the physical console instead.

---

## 2. Hit-testing while parked

> **Status: PARTIALLY VERIFIED — 2026-09-06, on the signed Program Files install.**
>
> The window style was measured directly from Win32 and is correct. The user-facing halves
> (2.1, 2.2) are still unobserved: a set style bit is strong evidence that clicks pass
> through, but it is not the same as watching a click land.

### What was measured

With `Presentation = AmbientNotch`, a fresh launch logged:

```
[OsdHost] UIAccess granted — the OSD can be created in the topmost band and can cover exclusive-fullscreen games
[OsdHost] Presentation applied: AmbientNotch, clickThrough=True, coversMonitor=False, parked=True, stripHeight=5
```

and the OSD's own HWND, enumerated by process and read with `GetWindowLongPtr(GWL_EXSTYLE)`:

```
class=PlithBandWindow_...  rect=1060,0-1500,178  TRANSPARENT=True LAYERED=True TOPMOST=True
```

Three separate things follow, none of them inferred from behaviour:

1. **`WS_EX_TRANSPARENT` (0x20) is genuinely set on the window.** This is the specific
   evidence the final review's Critical needed. `OsdHost` assigns `IsClickThrough` in its
   constructor, before `CreateWindow()`, when `BandWindow`'s setter still no-ops on
   `!IsLoaded` — so before the `Loaded` re-assertion was added, this bit was **not** set and
   the resting notch swallowed clicks from every launch. Note the log line alone could not
   have shown this: `clickThrough=True` reports the dependency property, which reads back
   true whether or not the native style was ever applied. Only the ex-style settles it.
2. **`Top = 0`** — the notch is flush with the top edge. The saved config had
   `Position = Custom`, so this also confirms the notch-mode anchor override is in force and
   that `EdgeMarginDip` is 0 for this mode.
3. **`Left = 1060`, width 440 on a 2560-wide display** — exactly centred, on the monitor named
   by `CustomPositionMonitorDeviceName = \\.\DISPLAY1`. That is the widened
   `ResolveTargetScreen` condition working.

### Still to observe

| # | Check | Pass | Fail |
|---|---|---|---|
| 2.1 | With the notch parked, try to maximize a window by dragging its title bar to the top edge under the notch | Snap/maximize works as if the notch were not there | The notch intercepts the drag or click |
| 2.2 | Click through the notch's screen region while nothing is expanded | Click reaches whatever is beneath it | The click is swallowed |

Original status note, still true of 2.1 and 2.2: **NOT VERIFIED**, for the same reason as
section 1.

`PresentationPolicy.WantsHitTesting` says the notch should not accept mouse messages while
parked (`_isParked == true`), so that the notch sitting at the very top of the screen does not
swallow clicks meant for window-maximize, browser tabs, or Snap Layouts.

| # | Check | Pass | Fail |
|---|---|---|---|
| 2.1 | With the notch parked, try to maximize a window by dragging its title bar to the top edge under the notch | Snap/maximize works as if the notch were not there | The notch intercepts the drag or click |
| 2.2 | Click through the notch's screen region while nothing is expanded | Click reaches whatever is beneath it | The click is swallowed |

---

## 3. Hover-to-open and the click-through toggle (Task 7)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing Task 7 — manual GUI verification in this project is a human
> step, and an agent attempting it has previously caused real harm. The checks below are
> recorded exactly as open, not as passed on the strength of the code reading correct.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under `[Osd]`,
restart Plith, and make sure `HoverKeepAlive=true` (its default).

**Why this section matters more than it looks:** `BandWindow.IsClickThrough` has always
supported assignment after window creation (`ToggleClickThrough`, gated on
`IsLoaded && HasSourceCreated`). It is not true that this has never run before — Task 6 already
added a second call site, `ApplyPresentationMode`'s `IsClickThrough = !_presentation.WantsHitTesting`,
which runs *after* `CreateWindow()`, and at a live settings-driven mode switch `IsLoaded` is
already `true`, so `ToggleClickThrough` executes there. The accurate claim is narrower: nobody
has yet run the branch this task adds — the toggle firing *while the window is actively being
hovered*, from `OnNotchHoverChanged`, `ShowOsd`, and `FadeOutAndHide`. There are two distinct
first-execution risks to watch for, not one:

- The guard could still skip the toggle in this specific calling context (for example if
  `IsLoaded` reads `false` at the moment a poller-driven event fires, which has never been
  observed one way or the other). The failure mode is asymmetric and easy to misread: the notch
  would either keep swallowing clicks at the top edge permanently, or refuse to become
  click-through once open, and nothing in the build/test/lint output would show it. If
  checks 3.3, 3.4, or 3.5 below fail, this guard is the first thing to check — the fix would be
  to route the toggle through the same `ApplyWindowStyles` path `Activatable` and `TopMost`
  already use, neither of which has an `IsLoaded` guard.
- The toggle could instead *succeed*, and that is its own first-execution risk: `ToggleClickThrough`
  calls `SetLayeredWindowAttributes(hWnd, 0, 255, LWA_ALPHA)` on a `WS_EX_LAYERED` window that has
  never had those attributes applied before — window creation sets the `WS_EX_LAYERED` style but
  never calls `SetLayeredWindowAttributes` itself. If that call changes how the window composites
  (a flicker, a flash to a different alpha, a redraw hitch), the symptom is a rendering change on
  the very first hover, not a swallowed click — watch for it specifically during check 3.1.

  > **This happened. 2026-09-06, first launch that reached the call.** It was not a flicker: the
  > `HwndSource` is created with `UsesPerPixelTransparency = true`, and
  > `SetLayeredWindowAttributes` switches a layered window to *constant*-alpha layering, which
  > discards the per-pixel channel outright. Every transparent pixel became opaque black, and
  > because notch mode never hides the window, the result was a permanent black rectangle at the
  > top of the screen rather than a transient artefact. Fixed by deleting the call: `WS_EX_LAYERED`
  > is already applied at creation and click-through is entirely the `WS_EX_TRANSPARENT` bit.
  >
  > Worth recording *how* it was found. Both automated gates stayed green throughout, and so did
  > every reviewer across ten tasks — because no test in this repository can construct this window.
  > It was found within seconds of a person looking at the screen. That is the standing argument
  > for why the checks in this file are the real gate and the green suite is not.

| # | Check | Pass | Fail |
|---|---|---|---|
| 3.1 | Move the cursor onto the resting notch | The notch grows smoothly into the full panel | Nothing happens, the expansion needs an unreasonably long dwell, or it jumps/snaps instead of growing |
| 3.2 | Move the cursor away from the open panel | The notch collapses back to the notch after the hide timer elapses | The notch stays down indefinitely, or collapses instantly with no hold time |
| 3.3 | While open, click a media transport button | It responds | The click is swallowed (see the `IsClickThrough`/`IsLoaded` note above) |
| 3.4 | While parked (notch only, not open), drag a window to the top edge of the screen | It maximises — the notch did not eat the drag | The drag is intercepted by the notch |
| 3.5 | While parked, click a browser tab (or any UI) directly under the notch | It activates normally | The click is swallowed |

**What to also note while running 3.1–3.2:** the poller compares cursor position against
`NotchGeometry.HoverRect` every 60 ms via `GetCursorPos` (physical pixels) converted through
`NotchGeometry.PhysicalToDip`. On a non-100% display scale this is the one place a mismatch
would show up as a notch that is hoverable in the wrong screen location — worth specifically
trying on a scaled monitor if one is available, not just the primary display.

**Known limitation, not something to chase during this pass:** `HoverRect` and `DpiScale` are
only published from `Reposition()`, which does not run on `WM_DPICHANGED`. If the display scale
changes live (moving the OSD's monitor between screens with different scaling, or a live DPI
change on the same monitor) while the notch is parked, the poller keeps comparing against the
stale rectangle/scale until the next show-from-rest calls `Reposition()` again — so the notch
can go unhoverable until that next show. Recorded here as a known limitation deferred out of
this round, not as something observed on a run.

**Second known limitation — mixed-DPI multi-monitor.** This is a different case from the one
above, not the same one restated: it needs no DPI *change* at all to bite. `_hoverPoller.DpiScale`
is a single scalar, so `NotchGeometry.PhysicalToDip` converts the whole virtual desktop at one
scale factor. On a desktop where two displays run different scale factors, every physical cursor
reading taken on the other display is converted with the OSD monitor's scale, so the notch is
hoverable in the wrong screen region — and the further the cursor is from the virtual-desktop
origin, the larger the error. Correct handling needs a per-monitor scale looked up from the point
being converted rather than one cached scalar. Recorded as a known limitation, not as an
observation: no mixed-DPI desktop has been run against this build.

---

## 4. The covered-monitor fallback to Classic (Task 8)

> **Behaviour changed since this section was written.** Task 8 originally *retracted the
> strip* while a window covered the monitor. That was not enough — the OSD stayed pinned to
> top-centre, so a volume key in a game still put the card in the middle of the field of view
> (see §9.3). The covered state now rebuilds the presentation as `ClassicPresentation`
> outright: the anchor, the edge margin, the monitor, the card's shape and the transition all
> come from Classic while a window covers the monitor, and the notch is rebuilt when it stops.
> `Retract()` and the retracted rest state no longer exist. The checks below are worded for the
> shipped behaviour.

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing Task 8, and no synthetic mouse or keyboard input was sent
> to any window or game — manual GUI verification in this project is a human step, and an
> agent attempting it has previously caused real harm. The checks below are recorded exactly
> as open, not as passed on the strength of the code reading correct.

**This must be run on the installed Program Files build, not a Debug build out of `bin\`.**
Debug builds have no UIAccess (see `Plith.Interop.UiAccess`), which silently changes what the
OSD is even allowed to draw over — a Debug build can already fail to cover a fullscreen game
for reasons that have nothing to do with collapse, and that failure would look identical to
a collapse bug in `plith.log`. Install the signed build before running this section.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under
`[Osd]`, restart the installed build, then open a game (or a fullscreen video player) and
alt-tab into it.

| # | Check | Pass | Fail |
|---|---|---|---|
| 4.1 | Alt-tab into the fullscreen game | `plith.log` shows `ForegroundCoversMonitor -> True` followed by `Presentation applied: AmbientNotch, …, coversMonitor=True` with no `parked=` suffix (that suffix only appears for a live notch, so its absence is how the log says "Classic was built"), and the notch is gone entirely — no shape, no sliver, no shadow bleed | The notch stays visible, `ForegroundCoversMonitor -> True` never appears, or the `Presentation applied` line still reports `parked=` |
| 4.2 | While still in the game, press a volume key, then let it auto-hide | The OSD still appears (this is the check that distinguishes the covered state from suppression), it appears **at the user's own Classic anchor** — not top-centre — and on the monitor Classic would use, **and** once it auto-hides nothing is left drawn over the game | The OSD stays hidden on the key press (the covered state conflated with `IShowSuppressor` somewhere, breaking Phase 5 §2's gate), **or** it appears top-centre anyway (the fallback is not reaching `Reposition`/`ResolveTargetScreen`), **or** a notch appears over the game after auto-hide (a notch presentation was built while covered) |
| 4.3 | Alt-tab back out of the game | `plith.log` shows `ForegroundCoversMonitor -> False (settled for …ms)` roughly 2–3 s after the alt-tab, then `Presentation applied: AmbientNotch, …, coversMonitor=False, parked=True`, and the notch returns at the top edge at the right offset for whatever is currently on the card (audio-only vs. audio+media size) | The notch does not return, returns at the wrong offset, the settle takes far longer than the logged elapsed time suggests, or the log lines are missing |

**Why 4.2 is the one to watch most closely:** Task 8's entire design premise is that the
covered state and suppression are separate signals — `IShowSuppressor` means "do not show at
all," `ForegroundCoversMonitorChanged` means "stop being a notch and behave like
Classic." Nothing in the automated suite can catch the two being accidentally merged, because
merging them would still build clean, still pass all 218 tests (none of which exercise a live
`OsdHost`/`AmbientNotchPresentation` pair — neither type can even be constructed by the
headless, non-STA suite), and still pass the a11y lint. A volume key still producing the OSD
while no notch appears over the game afterward is the only observation in this whole section
that actually distinguishes the two — and it is a two-part observation, not one: the OSD
appearing on the key press is necessary but not sufficient. `ShowOsd`'s at-rest path is allowed
to run over a game by design (that is what makes the OSD appear at all), and its own hide timer
then takes the card back down through `FadeOutAndHide` — a table that only checked the
appearance half would report a pass even if that left a notch over the game to stay, since
4.3's own alt-tab-out would mask exactly that failure by rebuilding the notch through the
normal path. Do not skip the "let it auto-hide" half of 4.2.

**Record, if this section is run:** the exact `plith.log` lines for 4.1 and 4.3 (the
`ForegroundCoversMonitor -> …` transitions, including 4.3's `settled for …ms` value), the
`Presentation applied: …` line that follows each of them, and whether any faint notch or
shadow was visible during 4.1 — the same drop-shadow-bleed question section 1 flags as
unresolved for `HiddenOffset`.

---

## 5. Settings UI — mode picker, resting height, and the position guard (Task 9)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing Task 9 — manual GUI verification in this project is a human
> step, and an agent attempting it has previously caused real harm. The checks below are
> recorded exactly as open, not as passed on the strength of the code reading correct.

**Setup:** launch Plith, open Settings, and find the new "Presentation" row at the top of the
"On-screen display" card (immediately above "Position").

| # | Check | Pass | Fail |
|---|---|---|---|
| 5.1 | Switch "Presentation" from Classic OSD to Ambient Notch | The notch appears live at the top of the screen with no restart — `OsdHost` rebuilds its presentation off `SettingsService.Changed` | Nothing changes on screen until Plith is restarted, or the OSD errors/crashes |
| 5.2 | With Ambient Notch selected, look at the "Set position" row | The button is greyed out (disabled), and hovering it shows the tooltip "The Ambient Notch is pinned to the top of the screen. Switch to Classic OSD to place the OSD yourself.", and the hint text below "Position" reads "The Ambient Notch is pinned to the top of the screen, so there is nothing to place. Switch to Classic OSD to choose a position." | The button stays clickable, the tooltip is missing, or the hint text still describes clicking "Set position" |
| 5.3 | With Ambient Notch selected, look for the "Notch resting height" row | It is visible, directly below "Presentation" (or below "Set position" once collapsed), with a slider running 2–24 | The row stays hidden, or shows the wrong range |
| 5.4 | Drag the "Notch resting height" slider | The resting notch on screen visibly grows/shrinks in real time (or on next park) to match the slider value | The notch does not change, or only changes after a restart |
| 5.5 | Switch back to Classic OSD | The notch disappears and the OSD behaves exactly as it did before switching (fade-in-place at the previous anchor), "Set position" re-enables with its tooltip cleared, and the hint text goes back to describing what the button does | The OSD keeps notch behaviour, the button stays disabled, or the position it restores to is not the anchor that was active before switching to notch mode |

**Why 5.5 matters:** `Position` and `CustomPositionXPercent`/`CustomPositionYPercent`/
`CustomPositionMonitorDeviceName` are deliberately left untouched on disk while notch mode is
active — `ApplyFromUi` never writes to them, and the position-edit path is disabled so a save
can't silently overwrite them with `OsdPosition.Custom` pinned at the notch's top-center anchor.
5.5 is the check that actually exercises that: it is the only way to observe whether Classic
truly comes back to the user's own anchor untouched, rather than something the notch left
behind.

**Nothing here is exercised by the automated suite.** `SettingsWindow` is a WPF `Window` the
headless, non-STA test suite cannot construct (same limitation the rest of this file already
notes for `BandWindow`), so `UpdatePresentationDependentControls`, the `PresentationCombo` /
`StripHeightSlider` bindings, and their interaction with the live notch are all unverified by
build, test, or lint passing.

---

## 6. Snap Layouts hover, auto-hide taskbar reveal, and dragging to the top edge (Task 10)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing this task — manual GUI verification in this project is a
> human step, and an agent attempting it has previously caused real harm. The checks below are
> recorded exactly as open, not as passed on the strength of the code reading correct.

Section 2 already covers the notch's own click-through/hit-test behaviour while parked. This
section is narrower and specifically about Windows' own top-edge gestures, which the notch
sits directly on top of and which nothing in this codebase implements or mediates — the design
spec's §8.1 lists them as a distinct risk from ordinary click-through because they are handled
by `explorer.exe` and `dwm.exe`, not by any window Plith owns.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under `[Osd]`,
restart Plith, so the notch is parked at the top-center of the monitor.

| # | Check | Pass | Fail |
|---|---|---|---|
| 6.1 | Hover the mouse over a window's maximize button so Windows shows the Snap Layouts flyout | The flyout appears normally, positioned and clickable as if the notch were not there | The flyout is visually obscured by the notch, or hovering it is delayed/blocked |
| 6.2 | If the taskbar is set to auto-hide, move the cursor to the taskbar's edge to reveal it | The taskbar reveals normally | The notch intercepts the pointer before the taskbar's edge-sensing does, and the taskbar does not reveal |
| 6.3 | Drag a window's title bar to the very top edge of the monitor, under the notch, to trigger maximise-by-drag | The window maximises as if the notch were not there | The drag is intercepted or visually caught on the notch |

**Why this is separate from section 2:** `PresentationPolicy.WantsHitTesting` and
`BandWindow.IsClickThrough` only govern whether *Plith's own window* accepts mouse input. Snap
Layouts and taskbar auto-hide reveal are driven by cursor position against screen edges at the
OS level and do not go through Plith's message loop at all when the notch is click-through —
but a layered, topmost, `WS_EX_LAYERED` window sitting exactly on the monitor's top edge is
exactly the kind of thing that has, in other overlay apps, been observed to visually cover
these OS surfaces even when it does not functionally block the underlying click. That visual
question is what 6.1 is checking; it is not covered by 2.1's drag-to-maximise check, which only
established that the *drag itself* is not swallowed.

---

## 7. Idle resource measurement with the window never hidden (Task 10)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing this task — manual GUI verification in this project is a
> human step, and an agent attempting it has previously caused real harm.

Phase 5 (`docs/PHASE5-VERIFICATION.md`) measured GDI handles flat at 22 across 160 volume
changes and found no leak. **That result does not carry over to Ambient Notch.** The Phase 5
run drove the OSD through show/hide cycles where the band window was hidden between events —
it never measured a window that stays visible (as the resting notch does) for an extended idle
period with no events at all. A leak that only manifests while a window remains mapped and
composited — a redundant `DispatcherTimer` tick, a repeating `GetCursorPos` poll from
`NotchHoverPoller`, a per-frame allocation in the parked-notch render path — would not have
shown up in that test and has not been looked for in this one either.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under
`[Osd]`, restart Plith, and leave the machine idle (no volume/media events, no deliberate
hover) with the resting notch on screen.

| # | Check | Pass | Fail |
|---|---|---|---|
| 7.1 | Record GDI object count, USER object count, handle count, thread count, and private bytes for the Plith process at startup (Task Manager "Details" tab, or Process Explorer) | — baseline only, not a pass/fail row — | — |
| 7.2 | Leave the process idle and parked for at least 30 minutes, taking the same readings every 5 minutes | All five figures stay flat (private bytes may oscillate with GC, as Phase 5 noted, but should not trend upward) | Any of GDI objects, USER objects, handles, or threads climbs monotonically across the window; private bytes trend upward without returning after GC |
| 7.3 | Repeat 7.1–7.2 once with the cursor resting motionless over the notch (so `NotchHoverPoller`'s comparison runs every tick but never fires a state change) | Same as 7.2 | Same as 7.2 |

**Why 7.3 is a separate row:** `NotchHoverPoller` runs on a `DispatcherTimer` regardless of
where the cursor is, but the branch it takes differs when the cursor sits inside `HoverRect`
without ever leaving (no `OnNotchHoverChanged` transition fires). If there is a leak specific
to that comparison path rather than to the general idle-parked state, 7.2 alone would miss it.

---

## 8. Expansion motion across refresh rates (Task 10)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing this task — manual GUI verification in this project is a
> human step, and an agent attempting it has previously caused real harm.

Section 1's checks 1.2 and 1.3 already ask whether the expansion/collapse animates rather than
snaps, but on a single unspecified display. This section asks the same question deliberately
twice, once per refresh class, because the animation is driven by WPF's `BeginAnimation` on a
~340 ms (expand) / ~260 ms (collapse) duration — short enough that frame pacing, not just the
easing curve, determines whether it reads as motion. At 60 Hz that is roughly 13–16 frames; on
a high-refresh panel it is proportionally more, and a composition or timer hiccup would be far
more visible as a single dropped frame at 60 Hz than lost in a denser frame sequence at
120 Hz+.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under
`[Osd]`, restart Plith. Repeat once with the Plith window on a 60 Hz display and once on a
display running at a refresh rate above 60 Hz (120 Hz, 144 Hz, or similar), if both are
available in the test environment.

| # | Check | Pass | Fail |
|---|---|---|---|
| 8.1 | On a 60 Hz display, press a volume key and watch the notch expand | The shape visibly grows over the ~340 ms window with a perceptible deceleration, and the content fades in only as it settles; it does not appear to jump directly from resting to open | The transition reads as an instantaneous jump, stutters, or visibly skips frames |
| 8.2 | On the same display, let the hide timer elapse and watch the collapse | The shape visibly shrinks back over the ~260 ms window with a perceptible acceleration | Same failure modes as 8.1 |
| 8.3 | Repeat 8.1 on a high-refresh (>60 Hz) display | Same pass criteria as 8.1 | Same failure modes as 8.1 |
| 8.4 | Repeat 8.2 on a high-refresh (>60 Hz) display | Same pass criteria as 8.2 | Same failure modes as 8.2 |

**If only one refresh class is available:** run what is available and record which one was
tested. A single-display result is still useful information; it is just not the full check —
note the gap rather than treating the untested class as passing by default.

---

## Reporting back

Note the check number and what you saw, the way `docs/PHASE5-VERIFICATION.md` does. A failure
in section 1 is worth stopping for — it is the mode's entire reason to exist.

---

## 9. Findings from the first real session (2026-09-06)

Recorded from a live run on the signed Program Files install, with UIAccess granted. These
are observations, not inferences.

### 9.1 Verified working

| What | Evidence |
|---|---|
| UIAccess is granted to the installed build | `[OsdHost] UIAccess granted — ...` |
| The presentation mode is applied from config | `Presentation applied: AmbientNotch, clickThrough=True, coversMonitor=False, parked=True, stripHeight=5` |
| A pre-upgrade `config.ini` with no `Presentation` key loads as Classic | first launch logged `Presentation applied: ClassicOsd` before the key was added |
| The notch is flush with the top edge, centred, on the saved monitor | `GetWindowLongPtr` + `GetWindowRect`: `rect=1060,0-1500,178` on a 2560-wide display, with `Position = Custom` in config and `CustomPositionMonitorDeviceName = \.\DISPLAY1` |
| `WS_EX_TRANSPARENT` is genuinely set while parked | ex-style read: `TRANSPARENT=True LAYERED=True TOPMOST=True`. Note the log's `clickThrough=True` alone would NOT have shown this — it reports the dependency property, which reads back true whether or not the native style was applied |
| Per-pixel transparency survives the click-through toggle (after the fix) | screen capture of the OSD's own rect shows the content behind it, where a black rectangle had been |
| The covers-monitor signal fires, and the strip goes away with it | `ForegroundCoversMonitor -> True` with nothing drawn in the OSD's rect. Note the *label* this row used to carry ("retraction fires") no longer names anything in the code: the covered state is now a wholesale rebuild into `ClassicPresentation`, not a retraction of the strip. The observation is unchanged; only the mechanism behind it was replaced |
| The covered state is not suppression | `Show: transition at 1060,0` logged *while* `coversMonitor` was true — the OSD still appeared on a volume key over a covering window |

### 9.2 The covers-monitor signal flaps during gameplay

**Status: confirmed on real hardware. A fix is implemented but has NOT been seen working on a
running build — no build carrying it has been launched. Treat the fix as unverified.**

`ForegroundCoversMonitor` does not settle while a game is running. From one session:

```
21:06:45.152  ForegroundCoversMonitor -> False
21:06:45.685  ForegroundCoversMonitor -> True
21:06:46.887  ForegroundCoversMonitor -> False
...
21:10:06.273  ForegroundCoversMonitor -> False
21:10:23.930  ForegroundCoversMonitor -> True
```

Under the design current at the time, every `False` re-parked the strip and every `True` took
it away, so the strip repeatedly appeared and disappeared across the top of the screen mid-game
(under the covered-state fallback the same flapping would rebuild the whole presentation). The user's report was direct: it gets in
the way badly.

The code is behaving exactly as designed — the defect is in the design. The spec argued for
"one rule, no game/video classifier: a rule with no classifier in it has no classifier to get
wrong". That reasoning holds for correctness and fails for stability: a raw foreground/geometry
predicate is simply noisy while a game is up, and the notch converts every sample into a visible
state change.

Three directions were considered — hysteresis, latching on the covering process, and
classifying games after all. **The first was chosen and implemented** (`PublishCoversMonitor`
in `FullscreenVideoWatcher`): the rising edge still publishes immediately, and a falling edge
must hold for `UncoverSettleMs` (2000 ms, three samples at the 1 Hz poll) of continuous
evidence before it is believed. `Environment.TickCount64` rather than `DateTime`, so a clock
adjustment cannot make a pending un-cover look settled. Latching on the process and
classifying games remain available if this proves insufficient; neither was implemented.

**What is still open:** whether 2000 ms is actually enough against the measured blips. Read
off the timestamps above, the flips are ~0.5 s and ~1.2 s apart, which this threshold covers;
the last pair is ~17.7 s apart, far outside it. That arithmetic is all this claim rests on —
no build carrying the hysteresis has been run, so its effect on the flapping has not been
observed at all.

**How to check it, when §4 is next run:** the `ForegroundCoversMonitor -> False` line now
prints the elapsed time actually observed rather than the constant, e.g.
`(settled for 2044ms)`. Two things to look for:

1. No `-> False` / `-> True` pair closer together than a few seconds while a game is up. Pairs
   that still flap mean the threshold is too short.
2. The strip returning within roughly 2–3 s of alt-tabbing out of the game, not later.
   A `settled for` value far above 2000 ms means the poll was being starved and the delay is
   not the threshold's fault.

### 9.3 Design problem: top-centre pinning is intrusive during gameplay

Also observed in the same session. In notch mode the OSD is pinned to top-centre, so it appeared
at `1060,0` — the middle of the field of view. The same user's Classic anchor is `1067,1140`,
near the bottom of the screen, chosen deliberately.

So switching to the notch silently relocates the OSD from a spot the user picked to the single
most intrusive spot on the screen.

**Partly addressed since.** The covered state no longer merely hides the strip — it rebuilds
the presentation as `ClassicPresentation` outright, so while a window covers the monitor the
OSD uses the user's own anchor, edge margin, card shape and transition again (and, since this
round, the monitor Classic would have chosen too). The `1060,0` placement described above is
what a volume key during gameplay would no longer produce.

What remains true is the *uncovered* case: in notch mode with nothing covering the monitor
the OSD is pinned to top-centre and there is still no way to move it, because §5's guard
disables position editing in notch mode. That guard is correct for the notch's own geometry
but leaves the user with no recourse.

**Neither statement above has been observed on a running build.** No build carrying the
covered-state fallback has been launched.

This is not a bug against the spec — the notch is *defined* as top-centre. It is evidence that
"Ambient Notch" and "an OSD you positioned yourself" are different products, and that a user who
has customised their position is being handed a downgrade in the uncovered case. Worth deciding
before the preset picker ships to anyone.


---

## 10. Second real session (2026-09-07) — what it found

### 10.1 The build under test was three hours stale

`C:\Program Files\Plith\Plith.exe` was timestamped 23:59:39, which is commit `3211d37`.
Five commits after it were on the branch and none of them were installed: the park-fully-off-
screen fix, the notch panel shape, the covered-monitor rebuild into Classic, and the
covers-signal hysteresis.

Every symptom reported during that session was therefore reported against code that had
already been changed. This is worth recording as a process finding, not just a fact: on this
project the running binary and the working tree diverge silently, and `manual-install.ps1`
needs elevation the agent cannot obtain unattended. **Check the exe timestamp against
`git log` before interpreting any live report.**

### 10.2 "It stutters" — diagnosed from the log, not yet re-measured

The user reported hitching. `plith.log` for that session shows the covers signal flipping
six times in 370 ms:

```
07:57:24.937 ForegroundCoversMonitor -> True
07:57:25.010 ForegroundCoversMonitor -> False
07:57:25.096 ForegroundCoversMonitor -> True
07:57:25.171 ForegroundCoversMonitor -> False
07:57:25.249 ForegroundCoversMonitor -> True
07:57:25.303 ForegroundCoversMonitor -> False
```

In `3211d37` every one of those edges started a retract or a re-show, so six animations
overlapped inside a third of a second. That is a sufficient explanation for the report and it
matches §9.2 exactly. The hysteresis that removes it (`UncoverSettleMs = 2000`) is on the
branch and was not in the binary.

**Open:** confirm on a build that carries it. The check is to run with a game up and count
`ForegroundCoversMonitor ->` lines in the log; the flapping pattern above must not recur.
Until that is done the diagnosis is well-supported but unconfirmed, and it is possible some
of the hitching had a second cause the log does not show.

### 10.3 The notch did not read as a notch

Reported directly: *"this doesn't look much like a notch, because all it does is come down
from above."* This is the finding that drove the §2 rewrite. It is not a defect against the
old spec — the old spec described a strip and a descent, and that is what was built. It is
the spec that was wrong, and only a running build could show it. See §1 above for the
replacement checks.

---

## 11. Notch home view (slice 2)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing this slice — manual GUI verification in this project is a
> human step, and an agent attempting it has previously caused real harm. Build, the 218-test
> suite, and `scripts/check-a11y.ps1` are green, but none of that exercises a rendered pixel,
> a live `Geolocator` prompt, or a real Open-Meteo response, so every row below is open.

This section covers what was added on top of the notch shell: hovering the resting notch opens
an `AmbientCard` row above the existing cards — a clock column, a weather column, and a battery
column, each collapsing independently when its data is unavailable. None of it opens on a
volume or media event; only a deliberate hover does. Weather resolves its location from a
typed city, then Windows Location, then IP geolocation, and refreshes every 15 minutes from
Open-Meteo. **Classic OSD is unaffected: `OsdHost` only starts `NotchHoverPoller` and only
opens `NotchHomeState` while `AmbientNotchPresentation` is active** (`OnNotchHoverChanged`
returns immediately otherwise), so there is no path by which Classic can show this row.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` and
`ShowAmbientOnHover=true` under `[Osd]` (or pick "Ambient Notch" and enable "Show ambient info
on hover" from Settings), restart Plith, and hover the resting notch. For 11.5–11.7, also clear
any cached `WeatherLatitude`/`WeatherLongitude`/`WeatherLocation` so the location resolution
actually runs cold.

| # | Check | Pass | Fail |
|---|---|---|---|
| 11.1 | Hover the resting notch | The panel opens with a row at the top: time and date on the left, battery on the right (laptops), weather in the middle if available | The row is missing, appears below the volume bar, or the panel opens without it and it pops in a frame later |
| 11.2 | Press a volume key without hovering | The panel opens with **no** ambient row — volume only, plus media if playing | The ambient row appears on an event |
| 11.3 | Hover, then let the panel collapse, then press a volume key | The row is gone on the key press | The row persists after the first hover |
| 11.4 | On a desktop with no battery | The battery column is absent entirely, not a dash or a zero | A placeholder renders |
| 11.5 | First run with no `WeatherLocation` set | Windows shows its location permission prompt once; granting it fills the weather column within ~10 s | No prompt appears, or the column stays empty after granting |
| 11.6 | Decline the location prompt, then restart | The column still fills (IP fallback), and `plith.log` shows `Windows Location access: Denied` **once**, not on every refresh | The prompt reappears, or the column stays empty |
| 11.7 | Type a city into "Weather location" | The column switches to that city within one refresh; `config.ini` gains non-zero `WeatherLatitude`/`WeatherLongitude` | The old location persists, or the cache is not written |
| 11.8 | Turn "Show weather" off | The column disappears and no further network requests are made | Requests continue |
| 11.9 | Switch to Classic OSD | No ambient row ever appears, and the three Settings rows are hidden | The row leaks into Classic |
| 11.10 | Narrator on the open panel | The row announces as "Ambient status"; its value reads as "Time ‹time›, ‹date›", then ", Battery ‹percent›" (plus ", charging" when charging) if the battery column is present, then ", ‹temperature›°  ‹condition›" if the weather column is present (`AmbientCardViewModel.AccessibleSummary`) — **by design, "Weather" is never spoken**: Time and Battery carry a spoken label, the weather segment does not | It announces a type name, reads bare numbers instead of the composed sentence above, or the Time/Battery segments are missing their labels or wrong order |

**Why 11.5 and 11.6 are the highest-risk checks in this slice.** Every other row here is a
WPF layout or a settings-wiring question, the same shape of thing the rest of this file has
already flagged as unverifiable without a running build. 11.5 and 11.6 are different in kind:
they exercise `Geolocator.RequestAccessAsync` from inside Plith's actual UIAccess process, and
that combination — a UIAccess-elevated, unpackaged desktop app calling the Windows Location
API — has never been observed running, only reasoned about from Microsoft's documentation (see
`WindowsLocationProvider`'s doc comment in `src/Plith/Services/LocationProviders.cs`). If
UIAccess changes how the consent toggle or the access-status result behaves, both checks fail
in a way no amount of code reading would have caught. 11.6 specifically is the one that decides
whether a user who said no gets left alone: `LocationResolver` and `WeatherService` were
deliberately built to retry Windows Location on every refresh regardless of the last outcome
(see `docs/superpowers/plans/2026-09-07-notch-home-view.md`, Task 5's note on
`ShouldRetryWindowsLocation`), on the reasoning that `RequestAccessAsync` does not re-prompt
after the first answer for an unpackaged app — but that reasoning is exactly the kind of thing
that is cheap to get wrong on paper and expensive to get wrong in front of a user, and it has
not been checked against a real `Denied` state on a real machine.

**Amendment to §7 (idle resource measurement).** The ambient row adds a 1 Hz `DispatcherTimer`
(the clock column's tick) and a 15-minute weather refresh timer to a window that §7 already
notes is never hidden while the notch is at rest. §7's idle run should now be read as covering
both: a leak specific to the home view's timers — rather than to `NotchHoverPoller` or the
general parked-notch render path §7 was originally written for — would show up in the same
30-minute idle measurement, and 7.3's motionless-hover variant now also exercises the home
view staying open (rather than just `NotchHoverPoller`'s comparison branch) for the duration.