# Brightness: design

**Goal:** changing the screen's brightness shows Plith's OSD, the way changing the volume
does. On a laptop that means the OEM's own brightness key. On a desktop with an external
monitor there is no such key, so Plith supplies the input itself.

**Status:** design approved, not yet planned. Branch `feature/brightness`, based on `main`
(0.1.8), which already carries `CardHost` and the notch.

---

## 1. What was measured

Everything below was measured on this machine before the design was written. The hardware is
a desktop (chassis type 3, no battery) with a single external monitor, an ASUS PG27AQDM.

| Question | Answer | How |
|---|---|---|
| Does the internal-panel WMI path exist here? | **No.** `root/wmi` `WmiMonitorBrightness` returns "Not supported" | Queried directly |
| Does DDC/CI answer a brightness read? | **Yes.** min=0, current=30, max=100 | `GetMonitorBrightness` through `dxva2.dll` |
| Does DDC/CI accept a write? | **Yes.** 30 to 45, read back 45, restored to 30 | `SetMonitorBrightness`, then a read to confirm |
| What does a write cost? | **56 ms**, and 61 ms for the restore | Stopwatch around the call |
| Does `GetMonitorCapabilities` report brightness? | **It fails outright.** Returns false, caps=0x0 | Same probe, same monitor, immediately before a read that worked |
| Is there a `VK_BRIGHTNESS` to hook? | **No such constant.** Zero hits, and no brightness `APPCOMMAND` either | `grep` over `WinUser.h`, Windows SDK 10.0.26100.0 |

### 1.1 And then it stopped answering, which is the third thing that decides the architecture

Every measurement above was taken from a **console session**. Later the same day, with the
same binary and the same monitor, `GetMonitorBrightness` began returning `ERROR_NOT_SUPPORTED`
and the low-level `GetVCPFeatureAndVCPFeatureReply(0x10)` failed alongside it, three times in
a row. The cause was not the code:

```
SESSIONNAME = RDP-Tcp#0        SM_REMOTESESSION = 1
console   ...  2  Conn
```

The session had been moved from the console to Remote Desktop. `EnumDisplayMonitors` then
returns the RDP virtual display rather than the physical panel, `WmiMonitorConnectionParams`
lists both (`AUS27FD` and a `Default_Monitor`), and the handle that comes back describes a
"Generic PnP Monitor" that answers no DDC/CI at all.

Two consequences, and the second is a design change rather than a note:

- **Brightness cannot be developed or verified over RDP**, the same way this repo already
  knows the OSD cannot be captured over RDP. The hardware checks in the plan need a console
  session.
- **Capability is not a startup-time fact.** A person who connects to their desktop remotely
  and later returns to it is an ordinary case, not an edge one, and discovery that runs once
  would leave the feature permanently off for them. Discovery reruns on display and session
  change; see §3.2.

Two of the earlier measurements decide the rest of the architecture.

**`GetMonitorCapabilities` is not a usable gate.** On the one monitor available to this
project it reports nothing while brightness reads and writes both work. Any implementation
that asks it first concludes the feature is unsupported on hardware where it is supported.
Capability is decided by attempting a read, and by nothing else.

**A write costs roughly a frame and a half.** Volume writes are effectively free; these are
not. Fifteen writes per second is the ceiling, and holding a key down produces far more
events than that.

## 2. The mechanism, which is not a keyboard hook

The roadmap's interception table says brightness arrives as `WH_KEYBOARD_LL` with
`VK_BRIGHTNESS_*`. That is wrong twice: the constant does not exist in the Windows SDK, and
no hook is needed.

Windows raises a WMI event, `WmiMonitorBrightnessEvent`, whenever the brightness of a
monitor changes. It carries the new `Brightness` as a percentage and the `InstanceName`, so
nothing has to be queried afterwards. It has existed since Vista. ModernFlyouts, already a
reference project for this repo, shows its brightness flyout from that event and hooks
nothing.

Listening to the change rather than to the key is strictly better:

- It does not matter which OEM key was pressed, or whether that key reaches a hook at all.
  Laptop brightness keys are handled in the driver stack and there is no guarantee they
  surface as keyboard input.
- It fires for every cause: the key, the Windows Settings slider, the Quick Settings panel,
  another application.

The event covers **internal panels only**. An external monitor never raises it, because
DDC/CI is request and response: the monitor answers when asked and never announces anything.
That is not a limitation to work around, it is the shape of the protocol.

The consequence is two independent halves.

## 3. Architecture

```
                 sense                                    act
     WmiMonitorBrightnessEvent                   BrightnessWriter (coalescing)
     (internal panel, any cause)                          |
                 |                              +---------+---------+
                 |                              |                   |
                 v                         DdcDevice           WmiDevice
          BrightnessMonitor              (dxva2, external)   (internal panel)
                 |                                    |
                 +----------------> BrightnessCard <--+
                                          |
                                     CardHost (existing show authority)
```

**Sense** and **act** never call each other. On a laptop the sense half alone produces the
whole feature: the person presses their own key, Windows changes the brightness, the event
arrives, the card shows. On a desktop the sense half is silent forever and the act half
drives the card directly, because Plith made the change and is the only one who knows.

### 3.1 `BrightnessMonitor` (sense)

Subscribes to `WmiMonitorBrightnessEvent` with a `ManagementEventWatcher` and raises
`Changed(int percent)`.

**Threading is load-bearing.** WMI raises on a worker thread. `CardHost`'s own class
documentation names this exact case as a future bug: a card raising `VisibilityChanged` or
`ShowRequested` off the dispatcher throws inside the WPF binding engine, far from the file
that caused it. The monitor therefore marshals to the dispatcher before anything reaches the
card, the way `OsdOrchestrator.OnMediaChanged` already does for SMTC.

Where no internal panel exists the watcher starts and never fires. That is the correct
behaviour and costs nothing, so it is not conditioned on anything.

### 3.2 `IBrightnessDevice` and its two implementations (act)

```csharp
interface IBrightnessDevice
{
    string Id { get; }            // stable across a session, used for logging and settings
    bool TryRead(out BrightnessReading reading);
    bool TryWrite(int value);
}

readonly record struct BrightnessReading(int Min, int Current, int Max);
```

The range is part of the reading rather than assumed to be 0 to 100.
`GetMonitorBrightness` returns all three, and the DDC/CI specification does not require the
minimum to be zero. The measured monitor happens to report 0 and 100, which is exactly the
kind of coincidence that hides a bug on someone else's hardware. Steps are expressed as a
percentage of the device's own span and clamped to it.

`DdcBrightnessDevice` wraps one `PHYSICAL_MONITOR` handle from
`GetPhysicalMonitorsFromHMONITOR`. `WmiBrightnessDevice` wraps the internal panel through
`WmiMonitorBrightnessMethods.WmiSetBrightness`.

**Discovery attempts a read and keeps whatever answers.** `GetMonitorCapabilities` is not
consulted, for the reason measured above. A device that fails a read is dropped rather than
retried.

**Discovery reruns on `WM_DISPLAYCHANGE` and on `WM_WTSSESSION_CHANGE`.** A monitor can be
plugged in, and a session can move between the console and Remote Desktop, which was measured
taking every DDC/CI-capable display away mid-session and would otherwise leave the feature
dead until a restart. The rerun is what turns "no devices" from a verdict into a state.

Handles are cached for the life of the device rather than opened per write. The measured 56
ms is the DDC/CI exchange itself; reopening the handle every time would add to it.

### 3.3 `BrightnessWriter` (coalescing)

One worker, latest value wins, one write in flight at a time. A request that arrives while a
write is running replaces the pending target rather than queueing behind it, so holding a key
down produces a smooth run of writes at whatever rate the hardware sustains, and never a
backlog that keeps changing the screen after the key is released.

This is not an invention. The same shape appears in every implementation that was examined:
Monitorian queues writes on a background worker so rapid changes do not block the UI thread,
and the laptop-key-to-DDC bridge that measured 65-71 ms per write describes collapsing rapid
key presses to the newest value.

The writer is the one piece of the act half that is pure logic, so it is also the one piece
that is properly unit-testable. See §6.

### 3.4 `BrightnessCard`

`Id` is `"brightness"`, `AccessibleName` is `"Brightness"`, `Order` is 30, which puts it
below Media (10) and Audio (20) and leaves the usual gaps.

`ShowReason` gains `BrightnessChange`. In notch mode the reason decides the shape, and
brightness takes the same short HUD a volume key gets, because it answers the same kind of
gesture.

**Visibility is transient, and this is the design's least certain decision.** The Audio card
is always visible because the OSD has no state in which it says nothing about audio. Applying
that rule here would put a brightness row on every volume press, for every user, including
laptop users who never asked for one. So the card is visible from a brightness change until
the OSD's show duration elapses, timed by the card against `SettingsModel.ShowDurationMs`.

That means the duration has two readers rather than one. The alternative is plumbing an
"OSD hidden" signal back from `OsdHost` through `CardHost`, which today is deliberately
fire-and-forget and holds no window reference. Reading the same setting from two places is
the smaller cost. If the timing looks wrong on hardware, this is the first thing to revisit.

### 3.5 Input

Two hotkeys, up and down, through the existing free-form capture in Settings.

`HotkeyService` registers exactly one combo today: `HotkeyId` is a constant and the service
holds a single `(mods, vk)` pair. It gains an id parameter so a second and third instance can
coexist. Each instance owns its own message-only window, so the per-window hotkey ids do not
collide.

**`NoRepeat` must not be set on these two.** It is an existing option on the service and it
is right for the summon hotkey, where a held key should fire once. Brightness is the opposite:
holding the key should keep moving the value, and the coalescing writer is what makes that
safe.

A step is 10 percent of each device's own span by default, settable, applied to every device
that answered discovery.

### 3.6 Multi-monitor

All capable devices move together, written in sequence on the writer's worker.

A brightness key on a laptop moves "the screen", and a person with two monitors pressing
brightness down almost certainly means both. Per-monitor selection is a settings surface that
can be added later without changing any of the above, because the writer already addresses
devices individually.

This machine has one monitor, so this decision is reasoned rather than measured. It is
recorded here as a decision rather than a fact.

## 4. Degradation

Nothing in this feature is allowed to take the OSD down, and nothing is allowed to appear
when there is nothing behind it.

- No device answered discovery: the card never becomes visible, the hotkeys are not
  registered, and the Settings section says so rather than offering controls that do nothing.
  This is the normal state inside a Remote Desktop session and must read as "not here right
  now" rather than as "not supported".
- A write fails: the device is dropped for the session and the card does not show. A monitor
  that has gone away must not produce an OSD reporting a value nobody changed.
- The WMI watcher throws on construction (stripped SKU, service disabled): the sense half is
  silently absent, exactly as `MediaSessionClient` degrades when SMTC is unavailable.

## 5. Settings

A new Brightness group, hidden entirely when no device answered discovery:

- Enable toggle, off by default. New surfaces in this product ship off.
- Two hotkey captures, up and down, using the existing control.
- Step size.

The hint text says what actually happens, which is a rule this repo learned the expensive way
on the media card: the "Show on track change" hint described behaviour that contradicted its
own label for three phases.

## 6. Testing

What is genuinely testable without hardware, and will be tested:

- `BrightnessWriter`: coalescing, latest-wins, one in flight, ordering. Driven by a fake
  `IBrightnessDevice` that records calls and can be made slow.
- Step arithmetic and clamping to the device's reported min and max.
- `BrightnessCard`: the show rule, the transient visibility window, and that it never raises
  off the dispatcher.
- Discovery's decision logic, against a fake device set, including the case that decides the
  architecture: a device whose capability query fails but whose read succeeds must be kept.

What cannot be tested that way, and must be driven on hardware before this is called done:

- That the OSD actually appears, and at the right size, for a brightness change. This repo has
  shipped green builds with a broken OSD more than once, most recently a notch slice that
  crashed on the first hover.
- That a held key produces a smooth ramp rather than a stutter or a backlog.
- The whole sense half. There is no laptop here, so `WmiMonitorBrightnessEvent` is
  **unverified on real hardware** and stays that way until someone runs it on a laptop panel.
- **Every hardware check needs a console session.** Measured: inside Remote Desktop no
  physical display is reachable at all, so a run there proves nothing except that the
  no-devices path is quiet.

That last one is the honest limit of this slice and is recorded as such rather than assumed.

## 7. Out of scope, deliberately

- **A notch brightness widget page** with a draggable level, mirroring the audio widget.
  It is the obvious next step and it is not needed to make a brightness change show an OSD.
- **Contrast, volume, input source** over DDC/CI. The protocol offers them; the product has
  no reason to yet.
- **Reacting to brightness changed on the monitor's own buttons.** No event exists for an
  external monitor, and the only way to notice is to poll a 56 ms read forever. The cost is
  real and the benefit is small.
- **Per-monitor hotkeys or profiles.**

## 8. Documentation this corrects

`docs/ROADMAP.md`'s interception table names `WH_KEYBOARD_LL` with `VK_BRIGHTNESS_*` for
brightness. The constant does not exist and the mechanism is wrong. The row is replaced with
the measured one: `WmiMonitorBrightnessEvent` for internal panels, Plith's own hotkey plus
DDC/CI for external monitors, and the note that `GetMonitorCapabilities` cannot be trusted as
a gate.

`CLAUDE.md` describes Phases 5 and 6 as unmerged and the product as 0.1.5. `main` is at 0.1.8
and carries `CardHost`, `WidgetFrame` and `NotchGeometry`. Only the shelf is unmerged. The
status section is corrected as part of this work, because a stale record costs more than no
record: the next session does not rederive it, it continues the wrong thing.
