# Phase 6 slice 1 — Notch shell (Classic + Ambient presentation modes)

**Date:** 2026-09-06
**Status:** Approved design, ready for writing-plans
**Roadmap reference:** `docs/ROADMAP.md` §3 (preset modes), §6 "Phase 6 — Notch mode + System Controls card"

## Problem statement

Phase 6 is the product pivot: Plith stops being an audio OSD and becomes the Windows
equivalent of the macOS "boring notch" category. The roadmap lists five deliverables
under it (notch geometry, two preset modes, preset picker + migration, System Controls
card, Battery card) plus a retention metric. That is far too much for one design.

This spec covers **slice 1 only**: the shell that makes a notch possible, and the two
presentation modes that can ship on top of today's cards.

`OsdHost` today has exactly one way to appear: it is invisible at rest, fades its
`Opacity` in at a settings-chosen anchor point, holds for a timer, and fades out. The
Ambient Notch is a different shape of the same idea — it *parks* a thin strip at the top
edge and *slides down* into a panel on an event, then retracts. Nothing about which cards
show, how long they hold, or when the OSD is suppressed changes.

## Goals

- A presentation-mode seam inside `OsdHost` that owns resting state, transition and
  hit-testing, and nothing else.
- `ClassicOsd` reproduces today's behaviour exactly — same fades, same anchors, same
  timing, same defect fixes.
- `AmbientNotch` ships: a 4–6 px strip pinned top-center, expanding on event or hover.
- The strip retracts entirely while a window covers the monitor, and returns after.
- Decision logic stays testable without a UI thread.

## Non-goals

- **Full Notch is deferred.** Its always-on cards do not exist: mic status belongs to the
  System Controls card (slice 3) and a Clock card is not on the roadmap's card list at
  all. Shipping Full Notch now would mean shipping an empty strip.
- Preset picker UI and the "meet the new Plith" migration nudge — slice 4.
- Battery and System Controls cards — slices 2 and 3.
- Retention telemetry — a separate privacy decision, not bundled here.
- Any change to `CardHost`, `ICard`, card ordering, show policy or the suppression gate.

## §1 — The presentation-mode seam

### Why extend `OsdHost` rather than add a `NotchHost`

`OsdHost` already owns band-window creation, the UIAccess z-band, the accent-mirror
resource dictionary, positioning, the hide timer, hover keep-alive, suppression wiring
and position edit mode. Ambient needs every one of those identically. Only three
behaviours differ: resting state, transition, hit-testing.

A separate host would duplicate all of the above, including each defect fixed during
Phase 5 verification — the fade-in generation counter, the deliberately absent
`BeginAnimation(OpacityProperty, null)`, the suppressor guard in `OnMouseEnter`. Those
are subtle and were found by observation, not by review. Duplicating the surface means
re-earning them.

### Interface

New `src/Plith/Views/Presentation/IOsdPresentation.cs`:

```csharp
internal interface IOsdPresentation
{
    /// Distance from the working-area edge used by Reposition(). Classic 96, Notch 0.
    double EdgeMarginDip { get; }

    /// True when the host shows nothing the user would read as "the OSD is up".
    /// Classic: Opacity below target. Notch: the strip is parked.
    bool IsAtRest { get; }

    /// True when the window should hit-test the mouse in its current state.
    bool WantsHitTesting { get; }

    void PrepareShow();

    /// Animate to fully visible. Must hand off from the current animated value rather
    /// than snapping — this is what makes interrupting a hide look continuous.
    void AnimateToVisible(double targetOpacity, Action onCompleted);

    /// Repeat show while already visible: no animation.
    void SnapToVisible(double targetOpacity);

    void AnimateToRest(Action onCompleted);
}
```

`OsdHost` keeps `_showGeneration`, `_fadeInGeneration`, `_isFadingIn`, `_isFadingOut`,
the hide timer, `ReassertTopmost` and the suppression guard. It calls into the mode only
for the animation itself. The Phase 5 anti-flicker rule ("do not restart a fade-in that
is already running toward this target") is mode-independent and stays in `OsdHost`, so
both modes inherit it.

### Implementations

`ClassicPresentation` — `Opacity` 0 at rest; `DoubleAnimation` on `OpacityProperty`
(140 ms in, 220 ms out, cubic ease); `WantsHitTesting` always true.

`AmbientNotchPresentation` — the window sits at its expanded size, flush with the top
edge, permanently visible. Content is offset by a `TranslateTransform`: `-h` at rest so
only `NotchStripHeight` shows, `0` when descended. `WantsHitTesting` is false at rest and
true while descended.

Because the notch window is permanently visible, `BandWindow.Show()` is called once when
the mode activates rather than on each `ShowOsd` — today's `Show()` sits inside the
`if (wasHidden)` branch, which for notch mode is only true on that first activation.

## §2 — Geometry and motion

### Move the content, not the window

Animating the window's `Top` means a `SetWindowPos` per frame, which janks. Instead the
window is fixed at its descended size and a `TranslateTransform` on the content animates.
WPF composites that; no per-frame Win32 call.

At rest the window is at full size but almost entirely transparent and click-through, so
it costs nothing visually or interactively.

### Edge margin becomes mode-owned

`Reposition()` currently hardcodes `EdgeMarginDip = 96` for every anchor, so `TopCenter`
sits 96 DIP below the top edge. The notch must be flush. The constant becomes
`_presentation.EdgeMarginDip`.

### Working area, not monitor bounds

`Reposition()` keeps using `Screen.WorkingArea`. With the taskbar on top, the notch sits
below it — which is correct, not a special case.

### Pure geometry

New `src/Plith/Views/Presentation/NotchGeometry.cs`, static, no WPF window dependency:

| Input | Output |
|---|---|
| working area, measured content size, strip height | resting offset, descended offset |
| working area, content size, strip height | strip hit rectangle |
| cursor point, strip rectangle | inside / outside |

This mirrors the `FullscreenVideoDetector` / `FullscreenVideoWatcher` split. That split is
what made §2's predicate unit-testable, and is why the Spotify AUMID false-positive could
be reasoned about rather than only observed.

## §3 — Hover and hit-testing

The strip is click-through at rest (`WS_EX_TRANSPARENT`). The top edge of the screen is
where users drag windows, reach browser tabs and hit Snap Layouts; an always-on strip that
swallows clicks there would be a defect, not a feature.

Hover is detected by **polling `GetCursorPos`**, not by a `WH_MOUSE_LL` hook. Mouse hooks
run on the input hot path — a slow callback lags the whole system's cursor. Plith already
carries one global hook (`WH_KEYBOARD_LL`); a second on a hotter path is the larger risk.
A ~60 ms poll is responsive enough and runs only in notch mode.

On entering the strip rectangle: `IsClickThrough = false`, then descend. On leaving:
retract and restore `IsClickThrough = true`.

The keep-alive *policy* in `OnMouseEnter` / `OnMouseLeave` is reused unchanged — the
`HoverKeepAlive` setting check, the edit-mode guard, the suppressor guard, and
`RestartHideTimer(_currentVisibleFor)` on leave. Its *visual* half is not: `OnMouseEnter`
today writes `Opacity` directly, which is a Classic-specific statement of "stay up". That
line becomes `_presentation.SnapToVisible(...)` so notch mode holds its descended state
instead.

### DPI — the sharpest edge in this design

`GetCursorPos` returns physical pixels; `Left` / `Top` and `Screen.WorkingArea` are
device-independent units. Mixing them silently breaks every comparison on a non-100 %
display — exactly the failure the comment above `ForegroundCoversMonitor` documents, and
the same class of bug that made Phase 5's capture scripts read the wrong screen region.

**Constraint:** the hover hit-test must convert into a single coordinate space explicitly,
and a test must cover a non-100 % scale factor.

## §4 — Retraction while a window covers the monitor

The strip fully retracts (and stops polling for hover) while the foreground window covers
its monitor, and returns when it stops.

### This is not suppression

`IShowSuppressor` means "do not show at all". This means "behave like Classic". Folding
one into the other would break the gate that §2 of the Phase 5 spec exists to provide.
It is a separate signal on a separate channel.

### Reuse the existing gather

`FullscreenVideoWatcher.ForegroundCoversMonitor()` already computes precisely this and is
currently `private static`. Lift it so the watcher can publish it as a second output. **No
second polling loop is added** — the existing 1 Hz timer plus `EVENT_SYSTEM_FOREGROUND`
hook feeds both, and the gather already runs regardless of whether
`HideDuringFullscreenVideo` is enabled (only `ShouldSuppress` gates on that flag).

### One rule, no game/video distinction

Covering the monitor is enough. A persistent strip over a fullscreen film is as unwelcome
as one over a game, and a rule with no classifier in it has no classifier to get wrong.

### Failure direction is inverted here — deliberately

Every failure path in `FullscreenVideoWatcher` fails toward *showing* the OSD, because a
suppression bug that hides the OSD is worse than one that shows it. For retraction the
opposite holds: a failure that leaves a strip sitting over a game is worse than one that
retracts when it need not. **Fail toward retracting.**

### Latency, stated honestly

Same characteristics as today's suppression: the WinEvent hook catches alt-tab instantly,
the 1 Hz poll catches F11 within a second. Entering fullscreen with F11 can leave the
strip visible for up to ~1 s.

## §5 — Settings

`SettingsModel` gains:

```csharp
public enum PresentationMode { ClassicOsd, AmbientNotch }
```

Default `ClassicOsd`, including for existing config files — the roadmap's "existing
installs do not feel bulldozed" requirement. `FullNotch` is deliberately absent from the
enum until slice 4; adding a value nothing implements invites a silent fallthrough.

Strip height is a setting (roadmap: "height configurable"), defaulting inside the 4–6 px
band §3 of the roadmap specifies.

In notch mode `Position` is pinned to top-center. `Position` and `CustomPosition*` are
left untouched on disk so switching back to Classic restores the user's anchor exactly.

## §6 — Four conflicts in existing code

Found by reading the current implementation against this design. Each needs a change;
none is discretionary.

### 6.1 `ResolveTargetScreen` only honours the saved monitor for `Custom`

```csharp
if (m.Position == OsdPosition.Custom && !string.IsNullOrEmpty(m.CustomPositionMonitorDeviceName))
```

The notch is not `Custom`, so as written it always lands on the primary screen on a
multi-monitor setup. The condition must widen to cover notch mode. This also answers the
roadmap §10 open question "notch pins to which monitor" — the saved device name, matched
the same way, falling back to primary when that display is unplugged.

### 6.2 Position edit mode is meaningless in notch mode

`EnterPositionEditMode()` lets the user drag and save, and `PersistCurrentPositionAsCustom`
writes `Position = Custom`. In notch mode the anchor is pinned, so a save would silently
corrupt it. Settings must disable the Edit Position control while notch mode is active.

### 6.3 The hidden test in `ShowOsd` assumes Classic

```csharp
bool wasHidden = Opacity < targetOpacity - 0.01;
```

Resting opacity in notch mode is not zero. This becomes `_presentation.IsAtRest`.

### 6.4 `HideOsd`'s early return has the same assumption

```csharp
if (Opacity < 0.01) return;
```

This is 6.3 in mirror image: it means "already hidden, nothing to do", but in notch mode
resting opacity is above the threshold, so the guard never fires and a hide is started on
an already-parked strip. It becomes `_presentation.IsAtRest` too.

Worth noting the case that makes this mostly theoretical rather than urgent: `HideOsd` is
called when the suppression gate closes, and `ShouldSuppress` requires
`foregroundCoversMonitor`, which is the same condition that has already retracted the
strip under §4. The two paths agree by construction. It is still fixed rather than relied
upon, because that agreement is a property of today's `ShouldSuppress` inputs and nothing
enforces it.

## §7 — Tests (`tests/Plith.Tests/`, headless)

`NotchGeometryTests`
- resting offset hides all but the strip, for several content heights
- descended offset is zero
- strip rectangle is flush with the working-area top, centered horizontally
- strip rectangle follows the working area when the taskbar is on top
- cursor just inside / just outside the strip
- **a non-100 % scale factor case**, per §3

`PresentationPolicyTests`

The `IOsdPresentation` implementations hold a `BandWindow`, which is a WPF
`ContentControl` behind an `HwndSource` — it cannot be constructed on this suite's
headless, non-STA threads. So the *decisions* live in a pure static `PresentationPolicy`
and the presentation classes are thin adapters over it, tested through §8 instead.

- Classic reports `EdgeMarginDip == 96` and always wants hit-testing
- Notch reports `0`, and wants hit-testing only while descended
- `IsAtRest` for both, at rest and while visible

Retraction gets **no unit test**, and that is a deliberate call rather than an omission.

Its decision is the covers-monitor boolean verbatim — the same value `ShouldSuppress` already
receives — plus a `catch` block that defaults it to true. There is no branching logic between
those two, so a test would either assert `true == true` or need to force an exception out of
`GetForegroundWindow` / `GetMonitorInfo`, which the headless suite cannot do. Extracting a
predicate to make it testable would be extracting the word `true`.

What the existing suite does cover is that `ShouldSuppress` is untouched: `FullscreenVideoDetectorTests`
must pass unchanged, which is what proves publishing a second output did not disturb the
suppression gate. The retraction behaviour itself is covered by §8.4 instead, on hardware.

`SettingsServiceTests`
- a config file with no `PresentationMode` key loads as `ClassicOsd`
- round-trip of `PresentationMode` and strip height

## §8 — Verification requiring a human

Recorded up front so they are not mistaken for covered:

1. The strip does not interfere with Snap Layouts hover, auto-hide taskbar reveal, or
   dragging a window to the top edge to maximise.
2. Idle resource measurement with the window never hidden. Phase 5 measured GDI handles
   flat at 22 across 160 volume changes, but that scenario hid the window between events.
3. The descent reads as motion, not as a jump, on a 60 Hz and a high-refresh display.
4. Retraction over a real game, on the installed Program Files build (UIAccess granted).
5. The runtime click-through toggle actually takes effect. `BandWindow.IsClickThrough`
   supports being set after creation, but that path has never executed in Plith:
   `OsdHost` assigns it once in its constructor, before `CreateWindow()`, when
   `HasSourceCreated` is false — so `ToggleClickThrough` returns early every time. Notch
   mode is its first caller. Its guard is `if (!IsLoaded || !HasSourceCreated) return;`,
   and a silently-skipped toggle leaves the strip either swallowing clicks at the top of
   the screen or refusing them once descended.

## §9 — Risks

| Risk | Mitigation |
|---|---|
| DPI space mixing in hover hit-test | §3 constraint + explicit non-100 % test |
| Always-visible layered window costs resources | §8.2 idle measurement before merge |
| Retraction fails, strip sits over a game | Fail toward retracting (§4) |
| Top edge belongs to Windows | Click-through at rest; §8.1 manual check |
| Notch shipped with nothing in the strip | Full Notch deferred; Ambient's strip is ambient by design |

## Deferred to later slices

- Full Notch, once System Controls provides mic status and a Clock card exists.
- Preset picker UI and migration nudge.
- Per-card configuration in the notch.
- Dynamic notch sizing (Phase 7, driven by the Shelf card).
