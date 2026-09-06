# Phase 6 — Manual Verification

Phase 6 (notch shell) is landing task by task on `feature/phase-6-notch-shell`. This file
tracks the checks that need a person, the same way `docs/PHASE5-VERIFICATION.md` does, and
for the same reason: no test in this repository can observe a rendered pixel or a running
animation, so an unrun check must never be recorded as a pass.

This entry covers **Task 6 only** — `AmbientNotchPresentation` and the `OsdHost` mode switch.
Later tasks in this phase (hover-to-expand, the fullscreen retraction signal, the position
editor's notch behaviour, etc.) will add their own sections here as they land.

---

## 0. What is automated as of Task 6

- `dotnet build src/Plith/Plith.csproj -c Debug` — 0 warnings, 0 errors.
- `dotnet test tests/Plith.Tests/Plith.Tests.csproj` — 158/158 passing.
- `pwsh -File scripts/check-a11y.ps1` — exit 0.

None of the above exercises `AmbientNotchPresentation` itself: both it and `ClassicPresentation`
hold a `BandWindow`, which is a `ContentControl` behind a native `HwndSource` that the headless,
non-STA test suite cannot construct. `PresentationPolicy` — the pure predicates the two adapters
delegate to — is unit-tested, but the adapters' WPF plumbing (the animations, `BeginAnimation`
calls, `SetStrip`, and the `OsdHost.ApplyPresentationMode` wiring) is not.

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

## Reporting back

Note the check number and what you saw, the way `docs/PHASE5-VERIFICATION.md` does. A failure
in section 1 is worth stopping for — it is the mode's entire reason to exist.
