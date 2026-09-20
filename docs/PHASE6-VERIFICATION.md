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
shadow was visible during 4.1 — not section 1's shadow question (that one is about the
resting pill's own shadow, and does not apply here), but the switch-leaking concern section 1
raises separately about `OsdHost.ApplyPresentationMode`: if `OsdContent.SetNotchLook(false)`
failed to run when Classic was built, `NotchSurface` would stay `Visible` (or its shadow would
stay non-zero) underneath the Classic card drawn on top of it, rather than being collapsed
outright.

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
actually runs cold. **Also confirm `HoverKeepAlive=true` under `[Osd]` (or "Hover keep-alive"
checked in Settings).** "Show ambient info on hover" opens the row through the same hover path
`HoverKeepAlive` gates — `OsdHost.OnNotchHoverChanged` early-returns before ever opening the
row when it is off — so with it off, hovering the notch does nothing at all no matter what
"Show ambient info on hover" is set to. Settings now disables that toggle and says so in its
hint whenever Hover keep-alive is off (see fix wave item 3); a verifier hitting a dead hover
here should check that setting before treating it as a regression.

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
| 11.10 | Narrator on the open panel | The row announces as "Ambient status"; its value reads as "Time ‹time›, ‹date›", then ", Battery ‹percent›" (plus ", charging" when charging) if the battery column is present, then ", Weather ‹temperature›°  ‹condition›" if the weather column is present (`AmbientCardViewModel.AccessibleSummary`) — Time, Battery and Weather all carry a spoken label now (fix wave item 7; the weather segment used to be bare, which read as a trailing fragment rather than a third labelled item) | It announces a type name, reads bare numbers instead of the composed sentence above, or any of the three segments is missing its label or in the wrong order |
| 11.11 | Narrator on the open panel, listening past the card-level summary | After the composed "Ambient status" sentence above, Narrator also re-announces "‹time›", "‹date›", the weather glyph character and "‹temperature›°  ‹condition›" and "‹percent›" individually, once each, as it walks the clock/weather/battery TextBlocks | Nothing after the composed sentence, or a type name instead of the raw text — either would mean WPF started honouring an empty `AutomationProperties.Name` differently than it does today |

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

**Known limitation: the ambient row's Narrator announcement is doubled (fix wave item 4).**
`AmbientCardView.xaml` binds `AutomationProperties.Name` to the composed
`AmbientCardViewModel.AccessibleSummary` on the `UserControl` root, which is the correct and
only place WPF gives it an automation peer (`Grid`/`StackPanel` own none). Every child
`TextBlock` inside — the clock, the weather glyph and text, the battery glyph and text — also
owns its own automation peer, though, and `TextBlockAutomationPeer.GetNameCore` falls through
to the element's own `Text` whenever `AutomationProperties.Name` is unset, including set to the
empty string. An `AutomationProperties.Name=""` on each of those TextBlocks was tried, on the
theory it would suppress that fallback; it does not — WPF's `AutomationProperties` was checked
(reflecting over `System.Windows.Automation.AutomationProperties` in `PresentationCore.dll`
lists `Name`, `HelpText`, `LabeledBy`, `LiveSetting`, `ItemStatus`, `IsOffscreenBehavior` and so
on, but nothing resembling WinUI/UWP's `AccessibilityView="Raw"`) and offers no supported way
to pull a `TextBlock` out of the automation tree short of overriding
`UIElement.OnCreateAutomationPeer` to return `null` on a custom control. So the inert
`Name=""` attributes were removed, and Narrator is expected to speak the card-level summary and
then re-announce "‹time›", "‹date›", the weather glyph character, "‹temperature›°  ‹condition›"
and "‹percent›" individually as it walks the row — see 11.11 above. Accepted as a known
limitation, not fixed. `MediaCardView.xaml` carries the same inert idiom in one place; it
predates this branch and was left untouched.

---

## 12. The crash the first real hover produced (2026-09-07)

The slice-2 build was installed and launched, and Plith **terminated** the first time the
user hovered the notch:

```
System.ArgumentException: 'Segoe MDL2 Assets' is not a valid value for property 'FontFamily'
  at Plith.Views.OsdHost.Reposition()                      <- UpdateLayout, realising the card
  at Plith.Views.OsdHost.ShowOsd(TimeSpan, Boolean fromHover)
  at Plith.Views.OsdHost.OnNotchHoverChanged(Boolean inside)
  at Plith.Views.Presentation.NotchHoverPoller ... DispatcherTimer.FireTick()
```

**Cause.** `AmbientCardView.xaml` bound `FontFamily="{x:Static services:WeatherCodeMap.GlyphFontFamilyName}"`,
and that constant is a `string`. A literal `FontFamily="Segoe MDL2 Assets"` attribute works
because the XAML parser runs a type converter on it; `{x:Static}` hands the property a
`System.String` **object**, and `DependencyObject.SetValue` performs no conversion.

It was an unhandled exception on the dispatcher, so it took the process down rather than
degrading — and the trigger was the notch's headline gesture, so the feature could not be
used at all.

**Why every automated gate missed it, one line each:**

| Gate | Why it passed |
|---|---|
| `dotnet build` (Debug + Release, 0 warnings) | `{x:Static}` compiles fine; the mismatch is a run-time `SetValue`. |
| The 219-test suite | Not STA — it cannot construct a `UserControl`, so no template was ever realised. |
| `scripts/check-a11y.ps1` | Parses the XAML as text; it never loads it. |
| Per-task review, whole-branch review, fix-wave re-review | All read the diff. The binding looks correct, and its intent (one source of truth for the font) was sound. |
| Live verification | No agent may launch or drive the app, so the only path to this defect was a human hovering. |

**The binding was introduced by a review fix, not by the original code.** An earlier round
required the view to bind the font name from the constant so the view and the font-existence
test could not drift onto different fonts. The requirement was right; the mechanism was wrong.

**Fixed** by exposing `AmbientCardViewModel.GlyphFont`, a real `FontFamily` built from
`WeatherCodeMap.GlyphFontFamilyName`, and binding **both** glyph `TextBlock`s to it — the
battery one previously repeated the string literal, which was exactly the drift the constant
was meant to prevent.

### What is now covered, and what still is not

`tests/Plith.Tests/AmbientGlyphFontTests.cs` asserts `GlyphFont` is a `FontFamily` and that it
names the family the glyph-existence test checks against. Those guard the **member**.

They do **not** guard the **binding**: re-pointing the XAML at the string constant brings the
crash straight back and nothing fails. And no test anywhere covers the general class — any
`{x:Static}` in any view handing a property the wrong type.

An STA harness that loads each view and forces a layout pass was attempted and abandoned: it
could not resolve the app's merged resource dictionaries from a PowerShell runspace, and a
gate that reports resource-resolution noise instead of defects is worse than none. The
remaining coverage for this class is a human launching the build and hovering.

**Standing consequence for this project:** a green build, a green suite and a green lint say
nothing about whether the OSD's markup loads. Until that changes, **install and hover before
believing any change to the card views works** — that check is cheap and it is the only one
that would have caught this.

---

## 13. Two more defects the first working hover exposed (2026-09-07)

Once section 12's crash was fixed, the notch could actually be hovered for the first time.
Two further defects surfaced immediately, and both are recorded here because both are
invisible to the build, the suite and the lint.

### 13.1 The panel closed with the cursor sitting on it — FIXED

The hide timer ran to completion while the user was looking at the open panel. Instrumenting
`OsdHost` from inside the process gave the answer in one line:

```
poller inside=True clickThrough=False wantsHit=True isMouseOver=False ... REAL_transparent=False hwndSize=440x233
hide timer fired (isMouseOver=False, clickThrough=False)
```

`WS_EX_TRANSPARENT` was genuinely cleared on the HWND, the window was correctly sized, and
**WPF still never raised `MouseEnter`** — not once, for the panel's whole life.

**Cause.** The OSD is a `WS_EX_LAYERED` window with per-pixel alpha, and Windows hit-tests
layered windows against that alpha: where a pixel is fully transparent the mouse passes
straight through and no message reaches WPF. At rest the notch is `NotchStripHeightDip` of
opaque pixels — 2 DIP on the reporting user's configuration — in an otherwise empty window,
so the cursor that triggers a hover is over transparent space. The panel then opens *beneath
a stationary cursor*, and a cursor that does not move generates no further `WM_MOUSEMOVE`, so
Windows never re-evaluates. `IsMouseOver` was false the entire time.

The keep-alive had been built on `IsMouseOver`, which on this window can never become true by
the very gesture that opens it.

**Fixed** by deciding keep-alive in the hide timer itself, against `NotchHoverPoller`'s polled
`GetCursorPos` reading, which depends on neither alpha, message delivery, nor movement.

A first attempt hung it off a poller *enter/leave event* and failed for a second reason worth
recording: moving up toward the notch crosses the panel's rectangle **before** the resting
shape's, so the enter transition fires while the notch is still parked, and a cursor that then
stays put never produces a second transition. Whatever that early transition decides is final.

### 13.2 Nothing in the open panel accepts a click — OPEN

Reported on the same build, with a media session playing, so the media card's transport
buttons were present and were the thing being clicked.

This is almost certainly the same root cause as 13.1 — WPF receives no mouse messages at all
on this window — with the difference that a click cannot be worked around by polling the way
the hide timer was. Something has to actually deliver `WM_LBUTTONDOWN` to the WPF content.

**Measured so far:**

| Fact | Value |
|---|---|
| Outer band window ex-style, panel open | `0x08080088` — LAYERED, TOOLWINDOW, TOPMOST, NOACTIVATE. `WS_EX_TRANSPARENT` **clear**. |
| `WS_EX_NOREDIRECTIONBITMAP` | Not set — dropped on Win11 by `BandWindow.CreateWindow` |
| Child `HwndSource` window | `HwndWrapper[Plith;;…]`, `ex=0x00080000` (LAYERED only), `WS_EX_TRANSPARENT` clear, sized to content |
| WPF `MouseEnter` | Never raised |

**Leading hypothesis, not yet tested.** The outer band window carries `WS_EX_LAYERED` but
nothing ever sets its layered attributes: `SetLayeredWindowAttributes` was deliberately removed
from `ToggleClickThrough` in `3211d37` because it destroys the per-pixel alpha channel and left
a permanent black rectangle on screen. A layered window with no layered attributes and no
`UpdateLayeredWindow` may hit-test as fully transparent, passing all input through regardless
of `WS_EX_TRANSPARENT`. If that is the mechanism, the fix cannot simply restore that call —
that is what section 12's sibling defect was — and needs a different structure.

**This blocks more than itself.** Any interactive notch content — media transport buttons, a
widget carousel's arrows, a file shelf's drop target — depends on the panel receiving mouse
input. It should be the first thing solved in the next slice, not assumed.

**Note for whoever picks this up:** three external sampling harnesses were written during this
investigation and all three produced misleading or empty results, while a four-line diagnostic
inside `OsdHost` answered the question immediately. Instrument the process; do not sample it
from outside.

---

## 14. The click-through problem — SOLVED, after eight attempts

The notch's window swallows clicks in the region the panel would occupy, even while closed
and invisible. Six approaches were tried in one session and none resolved it. Everything
below was measured on running builds, so the next attempt does not have to re-derive it.

### The architecture that causes it

`BandWindow` creates the top-level window through `CreateWindowInBand` for the UIAccess
z-band. WPF's content is then hosted in an `HwndSource` created with
`WindowStyle = WS_VISIBLE | WS_CHILD` and `ParentWindow` set to it — so **the window WPF draws
into is a child, and the top-level window is one WPF never paints**.

That matters because of one documented rule: **`UsesPerPixelOpacity` applies only to top-level
windows.** Alpha-based hit-testing — where a fully transparent pixel passes the mouse through
for free, which is exactly what an overlay wants — is therefore unavailable here. The
top-level window has no alpha for the raw input thread to test.

### What was measured, per configuration

| Configuration | Measured result |
|---|---|
| `WS_EX_LAYERED` set, layered attributes never supplied (the original state) | The raw input thread finds no content and skips the window. A `WndProc` counter recorded **0** `WM_MOUSEMOVE`, **0** `WM_LBUTTONDOWN`, **0** `WM_NCHITTEST` while the user hovered and clicked an open panel. |
| `WS_EX_LAYERED` cleared | Input arrives — **1820** moves, **7** clicks, **1842** hit-tests in one session — but the window is now an ordinary opaque one for hit-testing and captures its whole rectangle, including while closed. |
| `SetLayeredWindowAttributes(hWnd, 0, 255, LWA_ALPHA)` | Destroys the per-pixel alpha WPF draws with; leaves a permanent black rectangle. This is the defect `3211d37` removed. |
| `WS_EX_TRANSPARENT` on the container | Set correctly (`ex=0x080000A8`, verified from inside the process). Does not make the window click-through on its own: that requires `WS_EX_LAYERED` as well, which is unavailable per the first row. |
| `WS_EX_TRANSPARENT` on the child too | Applied and verified (`ex=0x00080020`). Did not resolve it. |
| `SetWindowRgn` clipping the window to the drawn pill | Did not resolve it. Removed again rather than left alongside the next attempt. |
| Answering `WM_NCHITTEST` with `HTTRANSPARENT` outside the drawn shape | The current state of the branch. **Did not resolve it.** |

Also measured, and worth not re-deriving: `WindowFromPoint` **ignores** `WS_EX_TRANSPARENT`,
so using it to test whether clicks pass through gives a misleading answer. It returned the
band window for every probe while the window was correctly click-through.

### The leading hypothesis for the next attempt

The problem is structural, not a matter of style bits: **the window WPF paints must be the
top-level window** for per-pixel alpha hit-testing to work at all. Two routes worth
investigating, in order:

1. Let `HwndSource` create its own top-level window with `UsesPerPixelTransparency`, then move
   that window into the UIAccess z-band afterwards. `CreateWindowInBand` has a counterpart for
   existing windows in the same undocumented family; if it can be reached, the band and the
   per-pixel alpha stop being mutually exclusive.
2. If they cannot be reconciled, the container has to supply its own alpha —
   `UpdateLayeredWindow` with a bitmap WPF renders into — which is a substantial change to how
   the OSD is composited and should not be attempted without measuring the cost.

### Process note

Three external sampling harnesses were written during this investigation and all three gave
misleading or empty results; a four-line diagnostic inside `OsdHost` and a message counter in
the `WndProc` answered each question immediately. Instrument the process, do not sample it.

The session's other lesson: deleting a locked `obj/` artifact to get a build moving produced a
binary that compiled with **0 errors** and crashed at startup with
`Cannot locate resource 'views/osdcontent.xaml'` — the XAML resource had silently not been
embedded. The real cause of the lock is that `Plith.Installer` references `Plith`, so a
parallel solution build has the WPF temp project and the main project writing the same output.
Build with `-m:1`; never delete intermediates to break a lock.

### Resolution

The window WPF paints is now the top-level window, and it is moved into the z-order band
afterwards rather than created there.

`HwndSource` creates it with `UsesPerPixelTransparency = true` and no parent, so it is a real
top-level layered window — which is the one configuration where per-pixel opacity applies.
`SetWindowBand`, exported from user32 alongside `CreateWindowInBand` and probed for at runtime,
then moves it into the UIAccess band. Verified present on this machine before the change:
`CreateWindowInBand`, `CreateWindowInBandEx`, `SetWindowBand`, `GetWindowBand` all exported.

The alpha WPF renders is now the hit-test mask: transparent pixels pass the mouse through and
drawn pixels receive it, with no help. Three mechanisms written during the investigation became
dead on the spot and were deleted rather than left as alternatives — the `WM_NCHITTEST` filter,
the polled click-through re-sync, and the `SetWindowRgn` clipping.

One regression came with it and is worth recording, because it followed directly from the
change: `RepositionHwndSource()` pinned the HwndSource to `(0,0)`, which was correct while it
was a `WS_CHILD` inside a container and moved the whole OSD to the screen's top-left corner
once it was the top-level window itself. Both call sites removed.

**What made this take eight attempts.** Every earlier attempt tried to make the container and
the child agree — style bits, a per-point hit-test filter on each, a window region, a polled
re-sync. All of them were patching a gap that only existed because the window WPF paints was
not the window the system hit-tests. The documented sentence that settles it —
*"UsesPerPixelOpacity applies only to top-level windows"* — was found by research on the
seventh attempt, and the two-directional measurement that proved no setting could satisfy both
requirements came on the eighth.
---

## 15. Widget carousel (slice 3) — everything still open

Slice 3 is code-complete on `feature/phase-6-notch-shell`: build green in Debug and Release with
0 warnings, 326 tests passing, `scripts/check-a11y.ps1` exit 0. **None of that touches anything
below.** The suite is not STA, so it cannot construct a `UserControl`, load a template, or
receive a mouse message; the OSD renders in a layered window no capture path can photograph over
RDP. Slice 2 shipped equally green and then crashed on the first hover, refused every click, and
drew half its content over the desktop.

### 15.1 Paging

- [ ] **Two-finger swipe on the touchpad pages exactly one widget.** The single highest-risk item
  in this slice. `NotchPager` accumulates deltas and commits one page per threshold, then waits
  for the accumulator to fall back — but the threshold (`120`), the rearm floor (`40`) and the
  idle gap (`150 ms`) are **provisional constants chosen without hardware**. Driver delta
  magnitudes differ, and whether inertia arrives after the fingers lift, and how large those
  tail deltas are, cannot be reasoned about from here.
- [ ] **Inertia after a commit does not page again.** The specific failure this is guarding
  against: a swipe that pages once, then pages again half a second later as the tail arrives.
- [ ] **A mouse tilt wheel pages once per detent.** A tilt sends exactly one `WHEEL_DELTA` and
  never anything smaller, so the rearm floor can never fire for it — only the idle gap can. This
  path was found by reasoning, not by testing, and has never run.
- [ ] **`Shift` + wheel pages, and in the direction a person expects.** The sign is negated
  because forward scrolls left by the Windows convention. That convention was applied from
  memory.
- [ ] **A plain vertical wheel still does nothing.** It must reach WPF unhandled.
- [ ] Clicking each page dot goes to that page; the hit target is reachable without care.
- [ ] Paging quickly in both directions leaves exactly one page in the tree — no stacked ghosts
  from an outgoing slide whose `Completed` never fired.

**Read the log for this.** Every commit writes `Widget page committed: delta=…, index=…`. The
delta values in that line are the only characterisation of this machine's touchpad that exists;
the three constants should be re-tuned from them rather than guessed at again.

### 15.2 The widgets

- [ ] Clock: ticks while shown, **stops when the page is away**. Check by leaving another page
  open and confirming the clock is correct — not stale — on returning.
- [ ] Audio: dragging the track changes the actual volume, on Voicemeeter and on a Windows
  endpoint. **First write path this app has ever had.**
- [ ] Audio: the thumb does not stutter or jump backwards under the finger while dragging. The
  guard is `_userIsDriving`, which suppresses incoming reports mid-gesture; if it is wrong, this
  is where it shows.
- [ ] Audio: keyboard arrows and Home/End move it, and a screen reader announces `62%` rather
  than `0.62`.
- [ ] Audio: unplug the headset mid-drag — the control stays where it was put rather than
  snapping.
- [ ] Media: transport buttons work; the play/pause icon matches the actual state.
- [ ] Media: a long title scrolls, a short one does not, and the scroll stops when the page
  leaves. **Both halves matter** — a marquee that always runs is the defect, not the feature.
- [ ] Weather: the sky matches the actual condition, and turns to the night gradient after 20:00.
- [ ] Weather: the reveal plays **once per day**, not once per open and not once per launch.
  Verify across an app restart on the same day. The date lives in `WeatherRevealDate` in the ini.
- [ ] Weather: with no location or no network the page says "Weather unavailable" rather than
  showing a blank sky.

### 15.3 The event HUD

- [ ] A volume key stretches the notch into the short bar, **not** the widget frame.
- [ ] A track change gives the wider HUD.
- [ ] The HUD's level bar is painted the first time it appears — it is sized from a width that
  only exists after arrange, which is exactly the class of defect this branch keeps producing.
- [ ] The speaker icon shows the mute cross when muted and drops its outer wave below 33 %.
- [ ] A HUD title ellipses rather than scrolling.

### 15.4 Cost, which has never been measured

- [ ] **Idle CPU with the notch closed, after a weather page has been open.** The ledger's oldest
  unmeasured item, and this slice is the first thing that could plausibly make it matter: the
  sky runs up to 30 storyboards. `WeatherWidget` logs `Sky started: … storyboards=N` and
  `Sky stopped: storyboards=N`; **the two counts must balance**, and after a close the last line
  must be a stop.
- [ ] Memory after paging through every widget fifty times.
- [ ] Motion smoothness at 60 Hz and at the monitor's real refresh rate.

### 15.5 Theme

- [ ] The bezel stays near-black on the **light** theme — the defect Task 8 fixed, and the only
  way to confirm the fix is to look at it.
- [ ] Widget text is readable on the bezel in both themes.
- [ ] High contrast: the notch uses the system's colours rather than the hard-coded ones.

### 15.6 Nothing underneath is captured

- [ ] After all of the above, the desktop under a closed notch still takes clicks. This is the
  regression that cost eight attempts in §14, and every change to the panel's contents changes
  the alpha the system hit-tests against.

### 15.7 Known incomplete

- **The lint rule banning `Segoe MDL2 Assets` under `Views/` was not added.** `MediaCardView.xaml`
  still uses it, and that replacement belongs to `2026-09-10-osd-and-settings-redesign.md`.
  Adding the rule now would have meant either pulling that work into this slice or narrowing the
  rule until it passed — and a rule narrowed to fit the code is a gate that has stopped meaning
  anything. Owed, not skipped.
- **`AmbientCard` is now unreachable in notch mode.** Events produce a HUD and a click produces
  the widget frame, so the card stack — and the ambient row inside it — is only ever shown by
  Classic, which does not open it on hover. The card, `NotchHomeState`, and the
  `ShowAmbientOnHover` setting are all still wired. Nothing is broken by it; it is dead weight
  that should be removed once slice 3 has been driven on real hardware and the widget pages are
  confirmed to cover what the row did.
- **Three constants are provisional**: `NotchPager.CommitThreshold`, `NotchPager.RearmFloor`,
  `NotchPager.IdleRearmMs`. See §15.1.

---

## 16. Classic OSD and Settings redesign — open

Green on build (Debug and Release, 0 warnings), 344 tests, and an accessibility lint that now
also fails on system icon fonts in both XAML and code-behind. As in §15, none of that reaches
anything below: the OSD renders in a layered window nothing can capture over RDP, and the
settings window is STA-only so the suite cannot construct it.

### 16.1 The card

- [ ] **The card at its new width.** 300 DIP, 224 in compact. Check that the level row does not
  look cramped at 224 and that the media row's title has room at 300.
- [ ] The shortened device name against the **real** G733 string. `AudioLabel.Shorten` is unit
  tested, but the string this machine actually reports has never been through it — the tests use
  what the old doc comment claimed, which turned out to be wrong twice.
- [ ] The bus line says the right rail on Voicemeeter and on a Windows endpoint, and says `Muted`
  when muted.
- [ ] The speaker icon's three states: crossed when muted, one wave below 33 %, two above.
- [ ] **The 85 % tick appears as the level approaches and sits where the colour changes.** It is
  positioned by a spacer sized from `CautionThresholdPosition`; if that binding is wrong the tick
  draws at the far left, which is the failure to look for.
- [ ] The fill's glow follows the threshold colour rather than staying green over an amber bar.
- [ ] A long track title scrolls on the classic card, a short one does not, and the scroll stops
  when the OSD dismisses.
- [ ] Every icon at 100 % and at 150 % DPI. These are geometry now, so they scale — but the
  stroke weights were chosen at one size.

### 16.2 Colour

- [ ] The new alarm colour reads as more urgent than the amber beside it, which the old one did
  not. This is the only reason it changed, and it cannot be checked any other way.
- [ ] The light theme still uses `#DC2626` and has not picked up the dark value.
- [ ] `UseColorThresholds` off makes the fill take the accent, and on restores the thresholds.
- [ ] High contrast: the card, the notch and the settings caption buttons all use the system's
  colours.

### 16.3 Settings

- [ ] Each rail item shows its section and hides the rest, and **Appearance is showing when the
  window opens** — the initial selection is applied from code because the XAML `IsChecked` fires
  before the handler exists.
- [ ] Arrow keys move between rail items; a screen reader announces them as one group.
- [ ] The preview shows the notch when Notch is picked and the card when Classic is.
- [ ] **Dragging the resting-height slider visibly changes the preview.** The height is
  exaggerated on purpose; if it still looks static, the exaggeration is too small rather than the
  wiring being wrong.
- [ ] The fallback banner appears only in notch mode.
- [ ] Nothing in the collapsed sections misbehaves when it is brought back — the update check,
  the hotkey capture and the accent swatches all run code while their section is hidden.

### 16.4 Known incomplete

- **Task 8 of the plan was not run.** It removes `AmbientCard`, `NotchHomeState`,
  `AmbientCardView` and the `ShowAmbientOnHover` setting, which slice 3 made unreachable. The
  plan gates it on slice 3 having been driven on real hardware, and §15 is still open in its
  entirety — so it stops here, deliberately, rather than deleting something whose replacement is
  unverified.
- The accent green is now stated once per palette file, but `Palette.Dark` and `OsdPalette.Dark`
  still each declare their own. They are different dictionaries loaded into different scopes and
  merging them is a larger change than the duplication costs; recorded rather than done.

---

## 17. What a self-review found before merge

Read back over the branch before proposing a merge, on the argument that with nothing verified
on hardware this was the last cheap moment to find something. Five defects, none of which would
have failed a build, a test or the lint, and none reachable from anything the suite can
construct.

| Found | Why no gate caught it |
|---|---|
| The audio widget's "user is driving" flag could stick forever after a keyboard arrow, silently freezing the display | Needs a focused Slider and a real key press |
| The weather page's bleed transform had a hard-coded centre taken from a size that is not that element's | Renders correctly enough to look deliberate |
| "Show weather" off did not remove the weather page, though Settings said it did | The setting works; only the sentence was wrong |
| Three comments named `BandWindow.HitTestFilter`, deleted two commits earlier | Comments are not compiled |
| A doc comment was orphaned in front of the wrong method, leaving two summary blocks on one | Same |

**Two of the five are the same failure as the branch's own recurring one**, in a different dress:
a value fixed at a moment (`_userIsDriving`, the transform's centre) rather than derived where it
is used. The other three are documentation describing code that no longer exists — which on this
branch is not cosmetic, because the comments are where the reasoning lives and a wrong one sends
the next reader at the wrong mechanism.

**None of this replaces the hardware session.** It found what could be found by reading. Every
item in §15 and §16 is still open.

---

## 18. System Controls — built, measured, and removed

Built as a four-tile page, driven on real hardware, and then deleted at the user's request. The
measurements are kept in `docs/ROADMAP.md` rather than here, because they are facts about this
machine that outlive the code: DDC/CI answers where WMI cannot, `IPolicyConfig` switches the
default output, and endpoint names need a tile-sized form.

What is worth keeping is why it failed, because it was not a bug.

The page worked. Brightness read and wrote, the mic toggled, the output switched, the tiles
carried live readings. It was rejected on how it looked and felt, over four rounds of rework, and
the last round found the actual cause of the last complaint: **the tiles were built from
hard-coded blue-grey hex while the panel takes an accent tint from the theme.** On a lime accent
the panel is green and the tiles sat on it as cold blue rectangles. Absolute colour in a themed
surface is the defect; relative colour — a translucent lift over whatever the panel is — is the
fix. That correction was never applied; the page was removed instead.

**The render harness could not have caught it**, and that is the durable lesson. It renders
widgets on a flat `#06070A` ground, while the running app renders them on an accent-tinted
gradient. Every colour judgement made from those renders was made against the wrong background.
If the harness is ever used to judge colour again, it must paint the real surface first —
`AccentTheme.DeriveOsdSurfaces` with the user's accent, the same call `ThemeService` makes.

---

## 19. The notch-to-HUD transition, measured

Reported three times across the branch as juddering, stepping, "heavy". Fixed three times by
reasoning, and it survived all three, because the reasoning was about the wrong failure.

### The probe

"The animation is heavy" has two causes that look identical: frames being dropped, and the
transition simply taking too long. They need opposite fixes. Nothing can capture this window —
it is layered with per-pixel alpha, and over RDP no capture works at all — so the shape was
counted instead: `CompositionTarget.Rendering` ticks across one morph, with the elapsed time and
the two sizes. One line per transition, temporarily.

### What it said

```
Morph: 56 frames in 235ms = 238 fps   356x116 -> 328x102
Morph: 38 frames in 235ms = 162 fps   328x102 -> 300x88
Morph: 26 frames in 250ms = 104 fps   300x88  -> 300x74
Morph: 27 frames in 250ms = 108 fps   300x74  -> 300x60
Morph: 22 frames in 234ms =  94 fps   300x60  -> 300x46
```

Not one frame was dropped — 94 to 238 fps throughout. **One transition was playing as five
chained morphs**, 978 ms end to end. Every earlier fix had been aimed at per-frame cost, and
per-frame cost was never the problem. The shadow detach in `31c5430` was a real improvement
aimed at a real cost, and it could not have fixed this.

### Why five

`NotchSurface` and `SlidingRoot` are siblings in the root `Grid`, so the Grid measures to
whichever is larger. `ApplyNotchExpand` sets `NotchSurface`'s size on every frame of a morph —
so the control's measurement follows the animation. The target was being derived from that
measurement, which made the morph its own input: the surface was still large, so the target only
shrank part of the way; the morph then shrank the surface; the smaller measurement started
another morph. It converged geometrically, in five rounds.

The XAML comment above `NotchSurface` asserted the opposite — "animating its size re-measures
nothing but itself" — and had been true of an earlier tree. It was the load-bearing false belief.

### The fix

Two decouplings, both of the same kind: stop deriving a target from something downstream of it.

- The target now comes from `SlidingRoot.DesiredSize` — the panel content alone, unaffected by
  the surface beside it, settling in one pass — rather than from the whole control's measurement.
- The pin that keeps the window from snapping moved off `SlidingRoot` onto the control itself. On
  `SlidingRoot` it inflated the very desired size now used as the target; on the control it holds
  the window and touches nothing the morph reads.

Measured after: `Morph: 52 frames in 234ms = 222 fps, 356x116 -> 300x46`. One animation.
Confirmed on the running build by the user.

### What is left behind

The probe stays, but silent. It warns only when frames fall below 45 fps (a genuine stall) or a
morph begins within 200 ms of the last ending (a chain — this defect returning). Both thresholds
come from the measurements above rather than from taste. A line per volume key would be noise,
and noise is how a log stops being read.

**This is the branch's own recurring defect in its fourth dress** — a value derived once, from
the wrong place. The three previous ones were fixed by recomputing where used. This one could not
be, because the value was not stale; it was circular. The difference was only visible from a
measurement.

---

## 20. The media page's Alcove redesign, driven on hardware (2026-09-20)

Spec: `docs/superpowers/specs/2026-09-20-media-widget-alcove-design.md`.
Plan: `docs/superpowers/plans/2026-09-20-media-widget-alcove.md`.

Build green, 526 tests green, `check-a11y.ps1` and `check-contrast.ps1` green, renders in both
themes. **None of that presses anything**, which is section 15's own lesson, so
`scripts/drive-media-page.ps1` was written to click the real notch and read the real UI
Automation tree.

### What the run said

Run at 22:40 on 2026-09-20, Debug build, console session Active, presentation AmbientNotch.
Spotify held a session (`SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`, "Three" by Mahmut Orhan)
and it was **paused**, with a timeline of `00:00:00 of 00:02:59`.

```
[PASS] a click while NOT playing opens the clock page, not the media page
       names in the tree: 22:40, 20 Eylül, playing Three - Mahmut Orhan | 20 Eylül |
                          Partly cloudy | 21° | Three - Mahmut Orhan
[PASS] the notch pages to the media widget
       names after paging: Now playing | Three | Mahmut Orhan | Previous track | Play |
                           Next track | Change output device | 0:00 | Playback position | -2:59
[PASS] the progress bar reports a value to UI Automation   value=0 of 100
[PASS] the elapsed and remaining clocks are drawn          clock-shaped names: 0:00, -2:59
```

The second line is the whole page read back from the live tree: every accessible name the
redesign declares is really there, including the new output control, and the bar is a real
`ProgressBar` whose position reaches a screen reader as a value rather than as a length of
pixels. The notch window measured `384x130` for the `356x116` frame, matching the shelf's own
numbers for the same shell.

### Still open: the playing direction

**The half of the rule that needs playback has not been measured.** Nothing was playing at the
time, so what passed is "not playing opens the clock", and the run says so in its own output
rather than reporting a pass for the other direction. Also unmeasured: whether the bar actually
advances, which only a playing source can show.

### Instrument defects, both this script's own

**1. A PowerShell scriptblock cannot collect the snapshot.** `MediaSessionClient` raises
`Changed` on a threadpool thread, and a scriptblock converted to an `Action<T>` has no runspace
there: it never runs, and nothing throws. The script reported "SMTC never delivered a snapshot"
while the client had read the session perfectly, which a separate probe proved by printing the
AUMID beside `changedFired=0`. The handler is now compiled with `Add-Type`.

That message was deliberately written to distinguish three answers rather than two (playing,
nothing playing, and unreadable), and that is the only reason the failure was diagnosed rather
than reported as "nothing is playing". An instrument that blames the machine for its own failure
would have cost a whole round here.

**2. Refusing to run while paused left half the rule unmeasurable.** The first version threw
unless something was playing, on the argument that a silent machine produces a vacuous pass. The
paused case is not vacuous: it is the other direction of the same rule. The script now measures
whichever direction the machine is in and names it in its output.

### Findings outside the change

**`ClockWidget` says "playing" for a paused session.** Visible in the first verdict's own
evidence: the clock page announced `playing Three - Mahmut Orhan` while SMTC reported the session
paused. `ClockWidget.Refresh` appends `", playing {NowTitle.Text}"` whenever its now-playing line
is visible, without consulting `IsPlaying`. Filed, not fixed: it is a different widget and
outside this change.

**`AutomationProperties.SetName(OpenSourceArea, ...)` still reaches nothing.**
`check-a11y.ps1` reports it as a known gap on every run: `OpenSourceArea` is a `Border`, which
WPF gives no automation peer, so "Open the app that is playing" is in no tree. It predates this
branch and the redesign kept the element as it was. The right fix is a `Button` with a
transparent template, which changes focus and keyboard behaviour and so is its own task.

### What the renders found that the lints could not

Recorded here because three of the four defects in this change were found by looking at a PNG.

**The progress fill went through four versions.** The contrast lint measured this page for the
first time, because removing the page's private ink is what made its pairs visible to it:

| Fill on groove | Worst measured | Where |
|---|---|---|
| `OsdAccent` on `NotchTrack` | **1,0:1** | lime accent, light theme |
| `NotchInk` on `NotchBezelBrush` | 1,1:1 | several accents |
| `NotchInk` on `OsdHighlight` | 1,1:1 | several accents |
| `NotchInk` on `NotchTrack` | 2,6:1 | near-white accent, dark theme |

No static pair clears the bar on every accent, because `ContrastInk.TrackOn` walks from the
surface only until it clears 3:1 **against the surface** and stops, leaving the ink an
unpredictable distance further along the same ramp.

An ink derived from the groove with `ContrastInk.PairOn` then cleared every ratio **and drew the
bar inverted**: on a dark-theme surface `TrackOn` returns a light grey, so the derived ink came
back near-black and the played part read as a hole punched in the groove. A contrast ratio has no
notion of which side should be stronger, so no lint could have caught it. Found in
`widget-media.png`. Both colours now come from `NotchInk`, the groove being the same ink at 24
percent, which is the one arrangement that cannot come out backwards.

**The disabled transport looked exactly like the live transport.** A replaced `ControlTemplate`
loses WPF's default dimming, so `Render`'s own comment about controls that do nothing when
pressed was only half true. Found in `widget-media-empty.png`.

**A long title clips hard against the rail.** `MarqueeText` clips its viewport with no edge
fade, anywhere in the product, so a still frame shows a title cut mid-glyph. It scrolls at
runtime, so this is a still-frame artefact rather than lost information, but the text column is
now 116 DIP rather than 148 and it will scroll more often. Filed, not fixed: an edge fade belongs
to `MarqueeText` and would change every surface that uses it. See `widget-media-long-title.png`.

### Seek, added afterwards, on a measurement that overturned the spec

The spec deferred seek with two reasons. One was real (the notch closes in about 2.6 s, so a
drag can be cut off) and one was **wrong**: "SMTC position writes are not supported by every
source" was applied to Spotify from memory rather than from a reading. Asked directly on
2026-09-20:

```
source: SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify
  IsPlaybackPosition  : True
  IsFastForwardEnabled: True
  min/max seek        : 00:00:00 / 00:03:02.903
```

The user's own main source accepts it. The other reason answered itself: `HoverKeepAlive`
already stops the hide timer while the pointer is on the open panel, which it is throughout a
drag.

The bar is a templated `Slider` now rather than a `ProgressBar`. That keeps the automation value
and adds keyboard arrows, Home and End for nothing, and it is 20 DIP tall to grab while drawing
4: `AudioWidget`'s own track records that rule at 6 DIP, and this bar is thinner.
`MediaSnapshot` carries `CanSeek` from `GetPlaybackInfo().Controls.IsPlaybackPositionEnabled`,
and a source that refuses gets a bar that still reports its position with no thumb and no hand
cursor.

The write happens on RELEASE rather than per mouse sample, because a write per sample makes the
source scrub. The 1 Hz tick is kept off the thumb by `AudioWidget`'s driving window, carried
over with the overflow defect its own comment records: a flag beside the timestamp, because a
sentinel timestamp of `long.MinValue` overflows and leaves the widget believing the user is
driving from the moment it is built.

**Not measured on hardware.** The driver gained a drag and a before/after position read through
the same SMTC probe, and it has not run: on the attempt at 00:25 on 2026-09-21 it refused,
correctly, because a game (`WardogsClient-Win64-Shipping`) held the pointer and produced 2726 px
of drift in one second. That is the precondition the shelf branch measured and this script
inherited, doing exactly its job. Two measurements are therefore still owed, and one run takes
both: the playing direction of the opening rule, and whether a drag moves the source.
