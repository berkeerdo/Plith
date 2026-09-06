# Phase 6 — Manual Verification

Phase 6 (notch shell) is landing task by task on `feature/phase-6-notch-shell`. This file
tracks the checks that need a person, the same way `docs/PHASE5-VERIFICATION.md` does, and
for the same reason: no test in this repository can observe a rendered pixel or a running
animation, so an unrun check must never be recorded as a pass.

This entry covers **Task 6, Task 7 and Task 8** — `AmbientNotchPresentation`, the `OsdHost`
mode switch, hover-to-descend via `NotchHoverPoller`, and the fullscreen-cover retraction
signal from `FullscreenVideoWatcher`. Later tasks in this phase (the position editor's notch
behaviour, etc.) will add their own sections here as they land.

---

## 0. What is automated as of Task 8

- `dotnet build src/Plith/Plith.csproj -c Debug` — 0 warnings, 0 errors.
- `dotnet test tests/Plith.Tests/Plith.Tests.csproj` — 162/162 passing, `FullscreenVideoDetectorTests`
  unchanged (Task 8 does not touch `ShouldSuppress`'s signature or behaviour).
- `pwsh -File scripts/check-a11y.ps1` — exit 0.

None of the above exercises `AmbientNotchPresentation` itself: both it and `ClassicPresentation`
hold a `BandWindow`, which is a `ContentControl` behind a native `HwndSource` that the headless,
non-STA test suite cannot construct. `PresentationPolicy` — the pure predicates the two adapters
delegate to — is unit-tested, but the adapters' WPF plumbing (the animations, `BeginAnimation`
calls, `SetStrip`, and the `OsdHost.ApplyPresentationMode` wiring) is not.

Task 7's `NotchHoverPoller` holds a `DispatcherTimer` and P/Invokes `GetCursorPos`, so it is in
the same boat: the headless suite cannot exercise it end to end either. The pure geometry it
calls — `NotchGeometry.StripRect`, `NotchGeometry.PhysicalToDip`, `NotchGeometry.IsInsideStrip`
— is unit-tested; the timer, the P/Invoke, and the `OnStripHoverChanged` wiring in `OsdHost` are
not.

---

## 1. The descent — Task 6's headline behaviour

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing Task 6 — manual GUI verification in this project is a human
> step, and an agent attempting it has previously caused real harm. The check below is recorded
> exactly as open, not as passed on the strength of the code reading correct.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under `[Osd]`,
restart Plith, then press a volume key.

| # | Check | Pass | Fail |
|---|---|---|---|
| 1.1 | At rest, before any key press | A thin strip (`NotchStripHeightDip`, default 5 DIP) sits flush with the top edge of the target monitor, horizontally centered, with no margin above it | No strip, a strip with a visible top margin, or a strip that is the full card height |
| 1.2 | Press a volume key | The full card slides down out of the strip over ~220 ms with an ease-out curve, then holds at `OsdOpacityPercent` opacity for the show duration | The card snaps into place with no animation, appears at the wrong opacity, or appears somewhere other than under the strip |
| 1.3 | After the hide timer elapses | The card retracts back up into the strip over ~260 ms with an ease-in curve, ending exactly where 1.1 started | The card disappears instead of retracting, overshoots, or leaves a gap/seam at the strip boundary |
| 1.4 | Repeat 1.2–1.3 while a media session is playing (larger card) | The strip still reads the top edge of the taller card; the resting (parked) offset adjusts so the strip shows the card's top edge, not a slice of its middle | The parked strip shows mid-card content, or the card is cut off asymmetrically |
| 1.5 | Toggle `Presentation` back to `ClassicOsd` while the notch is parked, then back to `AmbientNotch` | Each switch settles cleanly into that mode's rest state with no leftover offset, no stray opacity, and no double strip | Classic shows a phantom offset; the notch reappears with the wrong opacity or a hidden/missing strip |

**Why 1.5 matters:** `OsdHost.ApplyPresentationMode` is the piece of Task 6 with no equivalent in
Phase 5 — it tears down the previous presentation's state (clears the content-offset animation,
resets `ContentOffset` to 0, hides the strip) before building the new one, then, for the notch
branch specifically, re-measures content and calls `Park()` to settle without an animated flash.
That teardown/rebuild path only runs when a live settings change actually flips
`SettingsModel.Presentation`, which is exactly the kind of runtime transition the automated
suite cannot reach (`SettingsService.Changed` firing while a `BandWindow` is live).

**Open question, not resolved in this fix round: the parked strip is drawn twice.**
`Park()` leaves the card parked at `NotchGeometry.RestingOffset`, which by construction
sits the card's own bottom edge exactly at `y = stripHeight` — the same band the dedicated
`NotchStrip` border (`src/Plith/Views/OsdContent.xaml`) already covers. Both elements paint
with `{DynamicResource OsdSurfaceBrush}`, so at rest the strip is, as far as the code goes,
two overlapping surfaces in the same band: the `NotchStrip` Border, and a one-`stripHeight`-
tall sliver of the parked card's own bottom edge (with its own corner radius and drop
shadow, since it is the same `Border` the full card uses). Whether the intended parked look
*is* that doubled edge, or whether the card should instead sit fully behind `NotchStrip` at
`HiddenOffset` while parked (reserving `HiddenOffset` for the fullscreen-retraction case
only), is a visual composition call this fix round deliberately did not make — it is a
question about what looks right on a running build, not something a diff can settle. Check
this during the section 1 manual pass: at rest (1.1), does the top edge read as one strip or
as two overlapping edges/shadows? Neither outcome should be assumed; record what is actually
seen.

**Second open question, also not resolved: the card's drop shadow may still bleed past
`HiddenOffset`.** The card carries a `DropShadowEffect` (`BlurRadius` 28) as part of the same
`Border` whose edge `HiddenOffset` pushes off screen. `ClipToBounds` on the outer container
does not clip effects — only content — so it is unverified whether a faint halo from that
blur still shows in the reserved margin band even when the card itself is fully retracted.
This was not investigated further or worked around; it is recorded here, unresolved, for the
same manual pass to look for: during 1.3's retraction (and while retracted generally, once
Task 8 wires a caller to `Retract()`), check the strip's margin band for any faint glow or
halo beyond the strip itself. Record what is actually seen, not what should theoretically
happen.

**Known environment limitation, carried over from Phase 5:** over an RDP session the band
window's layered surface cannot be captured by any means (`BitBlt`, `BitBlt` with
`CAPTUREBLT`, and `PrintWindow` with `PW_RENDERFULLCONTENT` were all tried and failed for the
Classic case in Phase 5). If this check is attempted over RDP, expect the same — run it from
the physical console instead.

---

## 2. Hit-testing while parked

> **Status: NOT VERIFIED**, for the same reason as section 1.

`PresentationPolicy.WantsHitTesting` says the notch should not accept mouse messages while
parked (`_isParked == true`), so that the strip sitting at the very top of the screen does not
swallow clicks meant for window-maximize, browser tabs, or Snap Layouts.

| # | Check | Pass | Fail |
|---|---|---|---|
| 2.1 | With the notch parked, try to maximize a window by dragging its title bar to the top edge under the strip | Snap/maximize works as if the strip were not there | The strip intercepts the drag or click |
| 2.2 | Click through the strip's screen region while nothing is expanded | Click reaches whatever is beneath it | The click is swallowed |

---

## 3. Hover-to-descend and the click-through toggle (Task 7)

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
hovered*, from `OnStripHoverChanged`, `ShowOsd`, and `FadeOutAndHide`. There are two distinct
first-execution risks to watch for, not one:

- The guard could still skip the toggle in this specific calling context (for example if
  `IsLoaded` reads `false` at the moment a poller-driven event fires, which has never been
  observed one way or the other). The failure mode is asymmetric and easy to misread: the strip
  would either keep swallowing clicks at the top edge permanently, or refuse to become
  click-through once descended, and nothing in the build/test/lint output would show it. If
  checks 3.3, 3.4, or 3.5 below fail, this guard is the first thing to check — the fix would be
  to route the toggle through the same `ApplyWindowStyles` path `Activatable` and `TopMost`
  already use, neither of which has an `IsLoaded` guard.
- The toggle could instead *succeed*, and that is its own first-execution risk: `ToggleClickThrough`
  calls `SetLayeredWindowAttributes(hWnd, 0, 255, LWA_ALPHA)` on a `WS_EX_LAYERED` window that has
  never had those attributes applied before — window creation sets the `WS_EX_LAYERED` style but
  never calls `SetLayeredWindowAttributes` itself. If that call changes how the window composites
  (a flicker, a flash to a different alpha, a redraw hitch), the symptom is a rendering change on
  the very first hover, not a swallowed click — watch for it specifically during check 3.1.

| # | Check | Pass | Fail |
|---|---|---|---|
| 3.1 | Move the cursor onto the parked strip | The notch descends smoothly, as a slide rather than a jump, into the full card | Nothing happens, the descent needs an unreasonably long dwell, or it jumps/snaps instead of sliding |
| 3.2 | Move the cursor away from the descended card | The notch retracts back to the strip after the hide timer elapses | The notch stays down indefinitely, or retracts instantly with no hold time |
| 3.3 | While descended, click a media transport button | It responds | The click is swallowed (see the `IsClickThrough`/`IsLoaded` note above) |
| 3.4 | While parked (strip only, not descended), drag a window to the top edge of the screen | It maximises — the strip did not eat the drag | The drag is intercepted by the strip |
| 3.5 | While parked, click a browser tab (or any UI) directly under the strip | It activates normally | The click is swallowed |

**What to also note while running 3.1–3.2:** the poller compares cursor position against
`NotchGeometry.StripRect` every 60 ms via `GetCursorPos` (physical pixels) converted through
`NotchGeometry.PhysicalToDip`. On a non-100% display scale this is the one place a mismatch
would show up as a strip that is hoverable in the wrong screen location — worth specifically
trying on a scaled monitor if one is available, not just the primary display.

**Known limitation, not something to chase during this pass:** `StripRect` and `DpiScale` are
only published from `Reposition()`, which does not run on `WM_DPICHANGED`. If the display scale
changes live (moving the OSD's monitor between screens with different scaling, or a live DPI
change on the same monitor) while the notch is parked, the poller keeps comparing against the
stale rectangle/scale until the next show-from-rest calls `Reposition()` again — so the strip
can go unhoverable until that next show. Recorded here as a known limitation deferred out of
this round, not as something observed on a run.

---

## 4. Retraction while a window covers the monitor (Task 8)

> **Status: NOT VERIFIED.**
> This has not been run. No build from this branch has been launched, focused, or interacted
> with by an agent while producing Task 8, and no synthetic mouse or keyboard input was sent
> to any window or game — manual GUI verification in this project is a human step, and an
> agent attempting it has previously caused real harm. The checks below are recorded exactly
> as open, not as passed on the strength of the code reading correct.

**This must be run on the installed Program Files build, not a Debug build out of `bin\`.**
Debug builds have no UIAccess (see `Plith.Interop.UiAccess`), which silently changes what the
OSD is even allowed to draw over — a Debug build can already fail to cover a fullscreen game
for reasons that have nothing to do with retraction, and that failure would look identical to
a retraction bug in `plith.log`. Install the signed build before running this section.

**Setup:** in `%LOCALAPPDATA%\Plith\config.ini`, set `Presentation=AmbientNotch` under
`[Osd]`, restart the installed build, then open a game (or a fullscreen video player) and
alt-tab into it.

| # | Check | Pass | Fail |
|---|---|---|---|
| 4.1 | Alt-tab into the fullscreen game | `plith.log` shows `ForegroundCoversMonitor -> True`, and the parked strip is gone entirely — no strip, no sliver, no shadow bleed | The strip stays visible, or `ForegroundCoversMonitor -> True` never appears in the log |
| 4.2 | While still in the game, press a volume key, then let it auto-hide | The OSD still appears (this is the check that distinguishes retraction from suppression), **and** once it auto-hides the strip does not return — the notch is back to fully retracted, nothing drawn over the game | The OSD stays hidden on the key press (retraction conflated with `IShowSuppressor` somewhere, breaking Phase 5 §2's gate), **or** the strip reappears parked over the game after auto-hide (the volume key's own re-park undid the retraction — see the `_coversMonitor` check in `FadeOutAndHide`'s completion) |
| 4.3 | Alt-tab back out of the game | `plith.log` shows `ForegroundCoversMonitor -> False`, and the strip returns at the top edge, at the right offset for whatever is currently on the card (audio-only vs. audio+media size) | The strip does not return, returns at the wrong offset, or the log line is missing |

**Why 4.2 is the one to watch most closely:** Task 8's entire design premise is that
retraction and suppression are separate signals — `IShowSuppressor` means "do not show at
all," `ForegroundCoversMonitorChanged` means "retract the parked strip and behave like
Classic." Nothing in the automated suite can catch the two being accidentally merged, because
merging them would still build clean, still pass all 162 tests (none of which exercise a live
`OsdHost`/`AmbientNotchPresentation` pair), and still pass the a11y lint. A volume key still
producing the OSD while the strip stays retracted afterward is the only observation in this
whole section that actually distinguishes the two — and it is a two-part observation, not
one: the OSD appearing on the key press is necessary but not sufficient. `ShowOsd`'s at-rest
path is allowed to run over a game by design (that is what makes the OSD appear at all), and
its own hide timer then re-parks the card through `FadeOutAndHide` — a table that only checked
the appearance half would report a pass even if that re-park brought the strip back over the
game to stay, since 4.3's own alt-tab-out would mask exactly that failure by retracting it
again through the normal path. Do not skip the "let it auto-hide" half of 4.2.

**Record, if this section is run:** the exact `plith.log` lines for 4.1 and 4.3 (the
`ForegroundCoversMonitor -> …` transitions), and whether any faint strip/shadow was visible
during 4.1 — the same drop-shadow-bleed question section 1 already flagged as unresolved for
`HiddenOffset`, and Task 8 is the first task that actually reaches `Retract()` in a running
build.

---

## Reporting back

Note the check number and what you saw, the way `docs/PHASE5-VERIFICATION.md` does. A failure
in section 1 is worth stopping for — it is the mode's entire reason to exist.
