# Brightness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** changing the screen's brightness shows Plith's OSD, the way changing the volume does.

**Architecture:** Two halves that never call each other. **Sense** subscribes to
`WmiMonitorBrightnessEvent`, which Windows raises for internal panels on every brightness
change whatever caused it, and is the whole feature on a laptop. **Act** writes through
`dxva2.dll` (external monitors) or WMI (internal panel), driven by two of Plith's own
hotkeys, and is the only possible trigger on a desktop because DDC/CI never announces
anything. Both halves feed one `BrightnessCard` through the existing `CardHost`.

**Tech Stack:** WPF / .NET 10, `System.Management` for the WMI event, `dxva2.dll` P/Invoke,
the existing `HotkeyService`, `CardHost` and `SettingsService`.

**Spec:** `docs/superpowers/specs/2026-09-18-brightness-design.md`, which records every
measurement this plan rests on.

## Global Constraints

- All code, comments and commit messages in English. Conventional Commits. No AI attribution
  anywhere.
- **No em dash in anything written**, code comments and commit messages included. Use a
  comma, a colon, or a second sentence.
- Target `net10.0-windows10.0.22000.0`, x64, `Nullable=enable`.
- Build with `-m:1`. Never delete `obj/`, it silently omits BAML.
- `tests/Plith.Installer.Tests` must not be modified (three known pre-existing CA1861
  warnings; a clean build reports exactly 3 warnings and 0 errors).
- `scripts/check-a11y.ps1`, `scripts/check-shared-xaml.ps1` and `scripts/check-contrast.ps1`
  must all exit 0 before this branch is called done.
- **No Segoe MDL2 or any icon font.** `check-a11y.ps1` fails the build on one, in
  code-behind as well as XAML. Icons are drawn geometry in `Resources/PlithIcons.xaml`.
- **No hardware check is meaningful over Remote Desktop.** Measured: inside an RDP session
  `EnumDisplayMonitors` returns the remote virtual display, no physical monitor is reachable,
  and every DDC/CI call fails. Check `$env:SESSIONNAME` before believing any hardware result.
- **`GetMonitorCapabilities` must never gate anything.** Measured: it returns false with
  caps=0x0 on the PG27AQDM attached to this machine while brightness reads and writes both
  work. Capability is decided by attempting a read.
- **Nothing may reach `CardHost` off the UI dispatcher.** Its class documentation names this
  exact feature as the expected cause: a card raising `VisibilityChanged` or `ShowRequested`
  from a WMI or COM callback throws deep inside the WPF binding engine.
- Plith must be closed before building. A running instance locks `Plith.exe` and `Plith.dll`,
  and `Plith.DropCatcher.exe` survives Plith's death and locks its own copies.

**Baseline on `main`, measured rather than quoted: 398 tests pass (381 in `Plith.Tests`, 17
in `Plith.Installer.Tests`).**

## File Structure

**New, `src/Plith/Services/Brightness/`**
- `BrightnessReading.cs`: the `(Min, Current, Max)` triple a device reports.
- `IBrightnessDevice.cs`: one controllable display.
- `BrightnessStep.cs`: pure arithmetic for where a step lands, clamped to the device's span.
- `DdcBrightnessDevice.cs`: one `PHYSICAL_MONITOR` handle behind the interface.
- `BrightnessDiscovery.cs`: enumerates monitors, keeps the ones that answer a read.
- `BrightnessWriter.cs`: coalescing writer, latest value wins, one write in flight.
- `BrightnessMonitor.cs`: the WMI event subscription (sense).

**New, elsewhere**
- `src/Plith/Cards/BrightnessCard.cs`
- `src/Plith/ViewModels/BrightnessCardViewModel.cs`
- `src/Plith/Views/BrightnessCardView.xaml` / `.xaml.cs`

**Modified**
- `src/Plith/Cards/ShowRequest.cs`: add `BrightnessChange`.
- `src/Plith/Services/HotkeyService.cs`: per-instance hotkey id, and `NoRepeat` optional.
- `src/Plith/Services/SettingsModel.cs`, `SettingsService.cs`: six new settings.
- `src/Plith/Resources/CardTemplates.xaml`: one `DataTemplate`.
- `src/Plith/Resources/PlithIcons.xaml`: one sun geometry.
- `src/Plith/Views/SettingsWindow.xaml` / `.xaml.cs`: the Brightness group.
- `src/Plith/App.xaml.cs`: wiring.

**Tests**
- `tests/Plith.Tests/BrightnessStepTests.cs`
- `tests/Plith.Tests/BrightnessDiscoveryTests.cs`
- `tests/Plith.Tests/BrightnessWriterTests.cs`
- `tests/Plith.Tests/BrightnessCardTests.cs`
- `tests/Plith.Tests/HotkeyServiceTests.cs` (extend)

---

### Task 1: The device contract and the step arithmetic

**Files:**
- Create: `src/Plith/Services/Brightness/BrightnessReading.cs`
- Create: `src/Plith/Services/Brightness/IBrightnessDevice.cs`
- Create: `src/Plith/Services/Brightness/BrightnessStep.cs`
- Test: `tests/Plith.Tests/BrightnessStepTests.cs`

**Interfaces:**
- Consumes: nothing.
- Produces: `BrightnessReading(int Min, int Current, int Max)`;
  `IBrightnessDevice { string Id { get; } bool TryRead(out BrightnessReading reading); bool TryWrite(int value); }`;
  `BrightnessStep.Next(BrightnessReading reading, int stepPercent, bool up) -> int`.

- [ ] **Step 1: Write the failing test**

`tests/Plith.Tests/BrightnessStepTests.cs`:

```csharp
using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessStepTests
{
    [Fact]
    public void AStepUpMovesByThePercentageOfTheSpan()
    {
        var r = new BrightnessReading(Min: 0, Current: 30, Max: 100);
        Assert.Equal(40, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }

    [Fact]
    public void AStepDownMovesTheOtherWay()
    {
        var r = new BrightnessReading(Min: 0, Current: 30, Max: 100);
        Assert.Equal(20, BrightnessStep.Next(r, stepPercent: 10, up: false));
    }

    [Fact]
    public void TheSpanIsTheDevicesOwn_NotAssumedToBeZeroToHundred()
    {
        // DDC/CI does not require a minimum of zero. The monitor measured for this feature
        // happens to report 0 and 100, which is exactly the coincidence that hides a bug on
        // somebody else's hardware.
        var r = new BrightnessReading(Min: 20, Current: 20, Max: 80);
        Assert.Equal(26, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }

    [Fact]
    public void AStepCannotLeaveTheDevicesRange()
    {
        var top = new BrightnessReading(Min: 0, Current: 97, Max: 100);
        Assert.Equal(100, BrightnessStep.Next(top, stepPercent: 10, up: true));

        var bottom = new BrightnessReading(Min: 10, Current: 12, Max: 100);
        Assert.Equal(10, BrightnessStep.Next(bottom, stepPercent: 10, up: false));
    }

    [Fact]
    public void AStepThatRoundsToNothingStillMoves()
    {
        // A one-unit span with a small percentage rounds to zero, and a key that changes
        // nothing reads as a broken key rather than as a limit.
        var r = new BrightnessReading(Min: 0, Current: 5, Max: 10);
        Assert.Equal(6, BrightnessStep.Next(r, stepPercent: 1, up: true));
    }

    [Fact]
    public void ADeviceWithNoSpanIsLeftAlone()
    {
        var r = new BrightnessReading(Min: 50, Current: 50, Max: 50);
        Assert.Equal(50, BrightnessStep.Next(r, stepPercent: 10, up: true));
    }
}
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessStepTests`
Expected: FAIL, `CS0246` / `BrightnessStep` does not exist.

- [ ] **Step 3: Write the three files**

`BrightnessReading.cs`:

```csharp
namespace Plith.Services.Brightness;

/// <summary>
/// What a display reports about its own brightness.
/// </summary>
/// <param name="Min">The device's own floor. DDC/CI does not require it to be zero, and
/// assuming zero silently mis-scales every step on a monitor that reports otherwise.</param>
/// <param name="Current">Where it is now, in the device's own units.</param>
/// <param name="Max">The device's own ceiling.</param>
public readonly record struct BrightnessReading(int Min, int Current, int Max);
```

`IBrightnessDevice.cs`:

```csharp
namespace Plith.Services.Brightness;

/// <summary>
/// One display whose brightness can be read and written.
///
/// Both methods return false rather than throwing. A monitor can be unplugged between two
/// calls, and a display that has gone away must not take the OSD down with it.
/// </summary>
public interface IBrightnessDevice
{
    /// <summary>Stable for the life of the device. Used in logs and in settings.</summary>
    string Id { get; }

    bool TryRead(out BrightnessReading reading);

    bool TryWrite(int value);
}
```

`BrightnessStep.cs`:

```csharp
namespace Plith.Services.Brightness;

/// <summary>
/// Where a brightness step lands. Pure arithmetic, so it is the part of the act half that is
/// properly testable without hardware.
/// </summary>
public static class BrightnessStep
{
    /// <summary>
    /// The value one step away from <paramref name="reading"/>, clamped to the device's own
    /// range.
    ///
    /// The step is a percentage of the device's span rather than a fixed number of units,
    /// because the span is whatever the device says it is.
    /// </summary>
    public static int Next(BrightnessReading reading, int stepPercent, bool up)
    {
        var span = reading.Max - reading.Min;
        if (span <= 0) return reading.Current;

        // At least one unit. A step that rounds to zero produces a key that appears broken
        // rather than a limit that has been reached.
        var delta = Math.Max(1, (int)Math.Round(span * (stepPercent / 100.0), MidpointRounding.AwayFromZero));
        var next = up ? reading.Current + delta : reading.Current - delta;

        return Math.Clamp(next, reading.Min, reading.Max);
    }
}
```

- [ ] **Step 4: Run the test and watch it pass**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessStepTests`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Services/Brightness/BrightnessReading.cs src/Plith/Services/Brightness/IBrightnessDevice.cs src/Plith/Services/Brightness/BrightnessStep.cs tests/Plith.Tests/BrightnessStepTests.cs
git commit -m "feat(brightness): a device contract and where a step lands"
```

---

### Task 2: Discovery, and the rule that a failed capability query proves nothing

**Files:**
- Create: `src/Plith/Services/Brightness/BrightnessDiscovery.cs`
- Create: `src/Plith/Services/Brightness/DdcBrightnessDevice.cs`
- Test: `tests/Plith.Tests/BrightnessDiscoveryTests.cs`

**Interfaces:**
- Consumes: `IBrightnessDevice`, `BrightnessReading` from Task 1.
- Produces: `BrightnessDiscovery.KeepAnswering(IEnumerable<IBrightnessDevice>) -> IReadOnlyList<IBrightnessDevice>`;
  `BrightnessDiscovery.Discover() -> IReadOnlyList<IBrightnessDevice>`;
  `DdcBrightnessDevice : IBrightnessDevice, IDisposable`.

- [ ] **Step 1: Write the failing test**

`tests/Plith.Tests/BrightnessDiscoveryTests.cs`:

```csharp
using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessDiscoveryTests
{
    /// <summary>A device whose behaviour the test controls. `CapabilitiesAnswer` exists only
    /// to model the monitor this feature was measured on, which reports no capabilities at
    /// all while reading and writing perfectly.</summary>
    private sealed class FakeDevice(string id, bool readSucceeds) : IBrightnessDevice
    {
        public string Id => id;
        public int Reads { get; private set; }

        public bool TryRead(out BrightnessReading reading)
        {
            Reads++;
            reading = readSucceeds ? new BrightnessReading(0, 30, 100) : default;
            return readSucceeds;
        }

        public bool TryWrite(int value) => readSucceeds;
    }

    [Fact]
    public void ADeviceThatAnswersAReadIsKept()
    {
        var kept = BrightnessDiscovery.KeepAnswering([new FakeDevice("a", readSucceeds: true)]);
        Assert.Single(kept);
        Assert.Equal("a", kept[0].Id);
    }

    [Fact]
    public void ADeviceThatCannotBeReadIsDropped()
    {
        var kept = BrightnessDiscovery.KeepAnswering([new FakeDevice("a", readSucceeds: false)]);
        Assert.Empty(kept);
    }

    [Fact]
    public void EachCandidateIsAskedExactlyOnce()
    {
        // A read costs a DDC/CI round trip, measured at 56 ms on the monitor this was built
        // against. Discovery asking twice would double a cost that is already visible.
        var device = new FakeDevice("a", readSucceeds: true);
        BrightnessDiscovery.KeepAnswering([device]);
        Assert.Equal(1, device.Reads);
    }

    [Fact]
    public void TheAnsweringOnesSurviveAlongsideTheSilentOnes()
    {
        var kept = BrightnessDiscovery.KeepAnswering(
        [
            new FakeDevice("silent", readSucceeds: false),
            new FakeDevice("answers", readSucceeds: true),
        ]);

        Assert.Single(kept);
        Assert.Equal("answers", kept[0].Id);
    }
}
```

- [ ] **Step 2: Run the test and watch it fail**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessDiscoveryTests`
Expected: FAIL, `BrightnessDiscovery` does not exist.

- [ ] **Step 3: Write `DdcBrightnessDevice`**

```csharp
using System.Runtime.InteropServices;

namespace Plith.Services.Brightness;

/// <summary>
/// One external display, reached over DDC/CI through dxva2.dll.
///
/// The handle is opened once and kept. A write was measured at 56 ms on the monitor this was
/// built against, and that is the DDC/CI exchange itself; reopening the handle per write
/// would add to a cost that already limits how fast brightness can move.
/// </summary>
public sealed class DdcBrightnessDevice : IBrightnessDevice, IDisposable
{
    private nint _handle;
    private bool _disposed;

    internal DdcBrightnessDevice(nint physicalMonitorHandle, string id)
    {
        _handle = physicalMonitorHandle;
        Id = id;
    }

    public string Id { get; }

    public bool TryRead(out BrightnessReading reading)
    {
        reading = default;
        if (_disposed || _handle == 0) return false;

        // Deliberately NOT preceded by GetMonitorCapabilities. Measured on a PG27AQDM: that
        // call returns false with caps=0x0 while this one answers 0/30/100. Gating on it
        // would report the feature unsupported on hardware where it works.
        if (!GetMonitorBrightness(_handle, out var min, out var current, out var max)) return false;

        reading = new BrightnessReading((int)min, (int)current, (int)max);
        return true;
    }

    public bool TryWrite(int value)
    {
        if (_disposed || _handle == 0) return false;
        return SetMonitorBrightness(_handle, (uint)value);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != 0)
        {
            _ = DestroyPhysicalMonitor(_handle);
            _handle = 0;
        }
    }

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetMonitorBrightness(nint handle, out uint min, out uint current, out uint max);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool SetMonitorBrightness(nint handle, uint value);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool DestroyPhysicalMonitor(nint handle);
}
```

- [ ] **Step 4: Write `BrightnessDiscovery`**

```csharp
using System.Runtime.InteropServices;

namespace Plith.Services.Brightness;

/// <summary>
/// Finds the displays whose brightness this machine can actually change.
/// </summary>
public static class BrightnessDiscovery
{
    /// <summary>
    /// The candidates that answered a read, in order.
    ///
    /// A read is the entire capability test. The alternative, GetMonitorCapabilities, was
    /// measured returning false with caps=0x0 on a monitor whose brightness reads and writes
    /// both work, so it can only produce false negatives here.
    /// </summary>
    public static IReadOnlyList<IBrightnessDevice> KeepAnswering(IEnumerable<IBrightnessDevice> candidates)
    {
        var kept = new List<IBrightnessDevice>();
        foreach (var candidate in candidates)
            if (candidate.TryRead(out _)) kept.Add(candidate);
        return kept;
    }

    /// <summary>
    /// Every physical monitor attached right now that answers a brightness read.
    ///
    /// Not unit-tested, and cannot be: it enumerates real display handles. The decision it
    /// makes is in <see cref="KeepAnswering"/>, which is.
    /// </summary>
    public static IReadOnlyList<IBrightnessDevice> Discover()
    {
        var candidates = new List<IBrightnessDevice>();

        try
        {
            var monitors = new List<nint>();
            var callback = new MonitorEnumProc((handle, _, _, _) => { monitors.Add(handle); return true; });
            _ = EnumDisplayMonitors(0, 0, callback, 0);
            GC.KeepAlive(callback);

            foreach (var monitor in monitors)
            {
                if (!GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, out var count) || count == 0) continue;

                var physical = new PHYSICAL_MONITOR[count];
                if (!GetPhysicalMonitorsFromHMONITOR(monitor, count, physical)) continue;

                for (var i = 0; i < physical.Length; i++)
                {
                    var description = physical[i].szPhysicalMonitorDescription;
                    candidates.Add(new DdcBrightnessDevice(
                        physical[i].hPhysicalMonitor,
                        // The description is not unique on its own: two identical monitors
                        // both report the same string. The index disambiguates them.
                        $"{description}#{i}"));
                }
            }
        }
        catch (DllNotFoundException)
        {
            // dxva2 is absent on stripped SKUs. No devices is a valid answer.
            return [];
        }

        var kept = KeepAnswering(candidates);

        // Anything that did not answer holds an open handle nobody will use.
        foreach (var candidate in candidates)
            if (!kept.Contains(candidate) && candidate is IDisposable disposable) disposable.Dispose();

        return kept;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct PHYSICAL_MONITOR
    {
        public nint hPhysicalMonitor;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string szPhysicalMonitorDescription;
    }

    private delegate bool MonitorEnumProc(nint monitor, nint hdc, nint rect, nint data);

    [DllImport("user32.dll")]
    private static extern bool EnumDisplayMonitors(nint hdc, nint clip, MonitorEnumProc proc, nint data);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetNumberOfPhysicalMonitorsFromHMONITOR(nint monitor, out uint count);

    [DllImport("dxva2.dll", SetLastError = true)]
    private static extern bool GetPhysicalMonitorsFromHMONITOR(nint monitor, uint count, [Out] PHYSICAL_MONITOR[] array);
}
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessDiscoveryTests`
Expected: PASS, 4 tests.

- [ ] **Step 6: Prove discovery finds the real monitor**

This is the one piece of Task 2 a unit test cannot reach, and the whole feature rests on it.
Add a temporary xunit fact in `BrightnessDiscoveryTests`, run it, read the failure message it
prints, then delete the fact before committing. `Assert.Fail` is the reporting channel here
because a passing test prints nothing:

```csharp
    [Fact]
    public void TemporaryHardwareProbe()
    {
        var found = BrightnessDiscovery.Discover();
        foreach (var d in found)
        {
            Assert.True(d.TryRead(out var r));
            Assert.Fail($"{d.Id}: min={r.Min} current={r.Current} max={r.Max}");
        }
        Assert.Fail($"device count: {found.Count}");
    }
```

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter TemporaryHardwareProbe`
Expected from a **console session**: one device, `min=0 current=<whatever it is now> max=100`.

**Check the session first, because zero devices is the normal answer over Remote Desktop and
says nothing about the code.** Run `echo $env:SESSIONNAME`: `Console` means the probe is
meaningful, `RDP-Tcp#N` means it is not. This was measured the hard way while writing this
plan: the same code that read 0/30/100 from the console returned `ERROR_NOT_SUPPORTED` an hour
later, because the session had moved to RDP and `EnumDisplayMonitors` was returning the remote
virtual display.

From a console session, zero devices means discovery is broken and nothing after this task can
work. Delete the fact once it has answered.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Services/Brightness/BrightnessDiscovery.cs src/Plith/Services/Brightness/DdcBrightnessDevice.cs tests/Plith.Tests/BrightnessDiscoveryTests.cs
git commit -m "feat(brightness): find the displays that answer, not the ones that claim to"
```

---

### Task 3: The coalescing writer

**Files:**
- Create: `src/Plith/Services/Brightness/BrightnessWriter.cs`
- Test: `tests/Plith.Tests/BrightnessWriterTests.cs`

**Interfaces:**
- Consumes: `IBrightnessDevice` from Task 1.
- Produces: `BrightnessWriter(IReadOnlyList<IBrightnessDevice> devices, Action<Action>? runner = null)`;
  `void Request(int value)`; `event Action<int>? Wrote`.

- [ ] **Step 1: Write the failing test**

`tests/Plith.Tests/BrightnessWriterTests.cs`:

```csharp
using Plith.Services.Brightness;

namespace Plith.Tests;

public class BrightnessWriterTests
{
    private sealed class RecordingDevice : IBrightnessDevice
    {
        public string Id => "recording";
        public List<int> Writes { get; } = new();

        /// <summary>Runs inside TryWrite, so a test can make something happen while a write
        /// is in flight. That is the only moment coalescing is observable.</summary>
        public Action? DuringWrite { get; set; }

        public bool TryRead(out BrightnessReading reading)
        {
            reading = new BrightnessReading(0, 30, 100);
            return true;
        }

        public bool TryWrite(int value)
        {
            Writes.Add(value);
            DuringWrite?.Invoke();
            return true;
        }
    }

    /// <summary>Holds the pump instead of running it, so the test decides when work happens.
    /// A real runner hands the work to the thread pool, which would make every assertion here
    /// a race.</summary>
    private sealed class ManualRunner
    {
        private readonly Queue<Action> _pending = new();
        public void Post(Action work) => _pending.Enqueue(work);
        public int PendingCount => _pending.Count;
        public void RunAll() { while (_pending.Count > 0) _pending.Dequeue()(); }
    }

    [Fact]
    public void ARequestWritesTheValue()
    {
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        writer.Request(40);
        runner.RunAll();

        Assert.Equal([40], device.Writes);
    }

    [Fact]
    public void RequestsThatArriveBeforeThePumpRunsCollapseToTheLast()
    {
        // Holding a brightness key produces far more requests than the hardware can take.
        // A write was measured at 56 ms, so a queue would keep changing the screen long
        // after the key came up.
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        writer.Request(40);
        writer.Request(50);
        writer.Request(60);
        runner.RunAll();

        Assert.Equal([60], device.Writes);
    }

    [Fact]
    public void OnlyOnePumpIsEverPosted()
    {
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([new RecordingDevice()], runner.Post);

        writer.Request(40);
        writer.Request(50);

        Assert.Equal(1, runner.PendingCount);
    }

    [Fact]
    public void ARequestArrivingDuringAWriteIsPickedUpByTheSamePump()
    {
        var device = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([device], runner.Post);

        var once = false;
        device.DuringWrite = () =>
        {
            if (once) return;
            once = true;
            writer.Request(90);
        };

        writer.Request(40);
        runner.RunAll();

        // The second value is written without a second pump being posted, which is what keeps
        // a held key producing a smooth run rather than a stall at the first value.
        Assert.Equal([40, 90], device.Writes);
    }

    [Fact]
    public void EveryDeviceGetsTheValue()
    {
        var a = new RecordingDevice();
        var b = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([a, b], runner.Post);

        writer.Request(55);
        runner.RunAll();

        Assert.Equal([55], a.Writes);
        Assert.Equal([55], b.Writes);
    }

    [Fact]
    public void WroteCarriesTheValueThatLanded()
    {
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([new RecordingDevice()], runner.Post);
        var seen = new List<int>();
        writer.Wrote += v => seen.Add(v);

        writer.Request(70);
        runner.RunAll();

        Assert.Equal([70], seen);
    }

    [Fact]
    public void ADeviceThatRefusesAWriteDoesNotStopTheOthers()
    {
        var refusing = new RefusingDevice();
        var working = new RecordingDevice();
        var runner = new ManualRunner();
        var writer = new BrightnessWriter([refusing, working], runner.Post);

        writer.Request(35);
        runner.RunAll();

        Assert.Equal([35], working.Writes);
    }

    private sealed class RefusingDevice : IBrightnessDevice
    {
        public string Id => "refusing";
        public bool TryRead(out BrightnessReading reading) { reading = default; return false; }
        public bool TryWrite(int value) => false;
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessWriterTests`
Expected: FAIL, `BrightnessWriter` does not exist.

- [ ] **Step 3: Write the implementation**

```csharp
namespace Plith.Services.Brightness;

/// <summary>
/// Writes brightness to every device, newest value wins, one write in flight.
///
/// A DDC/CI write was measured at 56 ms on the monitor this was built against, so a held key
/// produces far more requests than the hardware can take. A queue would keep changing the
/// screen after the key came up; this replaces the pending value instead, so the screen ends
/// where the person left it and gets there as fast as the bus allows.
///
/// Every implementation examined converges on this shape: Monitorian queues writes on a
/// background worker, and the laptop-key bridge that measured 65-71 ms per write collapses
/// rapid presses to the newest value.
/// </summary>
public sealed class BrightnessWriter
{
    private readonly IReadOnlyList<IBrightnessDevice> _devices;
    private readonly Action<Action> _runner;
    private readonly object _gate = new();

    private int? _pending;
    private bool _pumping;

    /// <param name="runner">How the pump gets off the calling thread. Defaults to the thread
    /// pool. A test passes something it controls, because a pump on the thread pool makes
    /// every assertion about coalescing a race.</param>
    public BrightnessWriter(IReadOnlyList<IBrightnessDevice> devices, Action<Action>? runner = null)
    {
        _devices = devices;
        _runner = runner ?? (work => Task.Run(work));
    }

    /// <summary>Raised on the pump's thread after a value has been written to every device.
    /// Subscribers that touch UI must marshal; see CardHost's note on the dispatcher.</summary>
    public event Action<int>? Wrote;

    public void Request(int value)
    {
        lock (_gate)
        {
            _pending = value;
            if (_pumping) return;
            _pumping = true;
        }

        _runner(Pump);
    }

    private void Pump()
    {
        while (true)
        {
            int value;
            lock (_gate)
            {
                if (_pending is null)
                {
                    _pumping = false;
                    return;
                }

                value = _pending.Value;
                _pending = null;
            }

            foreach (var device in _devices)
            {
                // The result is deliberately ignored. One monitor refusing a write is not a
                // reason to leave the others where they were, and a display that has gone
                // away must not take the gesture down with it.
                _ = device.TryWrite(value);
            }

            Wrote?.Invoke(value);
        }
    }
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessWriterTests`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Services/Brightness/BrightnessWriter.cs tests/Plith.Tests/BrightnessWriterTests.cs
git commit -m "feat(brightness): write the newest value, never a backlog"
```

---

### Task 4: The card

**Files:**
- Create: `src/Plith/Cards/BrightnessCard.cs`
- Create: `src/Plith/ViewModels/BrightnessCardViewModel.cs`
- Modify: `src/Plith/Cards/ShowRequest.cs`
- Test: `tests/Plith.Tests/BrightnessCardTests.cs`

**Interfaces:**
- Consumes: `ShowRequest`, `ShowReason`, `ICard`, `SettingsService`.
- Produces: `BrightnessCard(SettingsService settings, Action<TimeSpan, Action>? scheduleHide = null)`;
  `void Report(int percent)`; `BrightnessCardViewModel Vm { get; }`;
  `ShowReason.BrightnessChange`.

- [ ] **Step 1: Add the show reason**

In `src/Plith/Cards/ShowRequest.cs`, add `BrightnessChange,` to the `ShowReason` enum after
`AudioChange`.

- [ ] **Step 2: Write the failing test**

`tests/Plith.Tests/BrightnessCardTests.cs`:

```csharp
using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class BrightnessCardTests
{
    private static SettingsService NewSettings(int showDurationMs = 2000)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        var m = svc.Current.Clone();
        m.ShowDurationMs = showDurationMs;
        svc.Save(m);
        return svc;
    }

    /// <summary>Captures the hide callback instead of waiting for a timer, so the test decides
    /// when the window closes.</summary>
    private sealed class ManualHide
    {
        public TimeSpan? After { get; private set; }
        public Action? Callback { get; private set; }
        public int Scheduled { get; private set; }

        public void Schedule(TimeSpan after, Action callback)
        {
            After = after;
            Callback = callback;
            Scheduled++;
        }

        public void Fire() => Callback?.Invoke();
    }

    [Fact]
    public void ACardWithNothingReportedIsInvisible()
    {
        var card = new BrightnessCard(NewSettings());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void ReportingAChangeMakesItVisibleAndAsksForTheOsd()
    {
        var card = new BrightnessCard(NewSettings());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Report(40);

        Assert.True(card.IsVisible);
        Assert.Single(shows);
        Assert.Equal(ShowReason.BrightnessChange, shows[0].Reason);
        Assert.Equal("brightness", shows[0].OriginCardId);
    }

    [Fact]
    public void TheCardGoesAwayOnItsOwnAfterTheShowDuration()
    {
        // The Audio card is always visible, because the OSD has no state in which it says
        // nothing about audio. Brightness is not like that: a permanent row would appear on
        // every volume press for every user, including laptop users who never asked for one.
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(showDurationMs: 2000), hide.Schedule);
        var visibilityChanges = 0;
        card.VisibilityChanged += () => visibilityChanges++;

        card.Report(40);
        Assert.True(card.IsVisible);
        Assert.Equal(TimeSpan.FromMilliseconds(2000), hide.After);

        hide.Fire();

        Assert.False(card.IsVisible);
        Assert.True(visibilityChanges >= 2);
    }

    [Fact]
    public void ASecondChangeRestartsTheWindowRatherThanStackingTimers()
    {
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(), hide.Schedule);

        card.Report(40);
        card.Report(50);

        Assert.Equal(2, hide.Scheduled);
        Assert.True(card.IsVisible);
    }

    [Fact]
    public void TheReportedValueReachesTheViewModel()
    {
        var card = new BrightnessCard(NewSettings());
        card.Report(65);
        Assert.Equal(65, card.Vm.Percent);
    }

    [Fact]
    public void TheHideWindowFollowsTheCurrentSetting()
    {
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(showDurationMs: 3500), hide.Schedule);

        card.Report(40);

        Assert.Equal(TimeSpan.FromMilliseconds(3500), hide.After);
    }

    [Fact]
    public void AccessibleName_IsHumanReadable_AndDrivesToString()
    {
        // WPF's ItemAutomationPeer names the OSD's list container from ToString. See
        // ICard.AccessibleName for the measurement behind this.
        var card = new BrightnessCard(NewSettings());
        Assert.Equal("Brightness", card.AccessibleName);
        Assert.Equal(card.AccessibleName, card.ToString());
        Assert.DoesNotContain("Plith.Cards", card.ToString());
    }

    [Fact]
    public void TheCardSitsBelowAudioInTheStack()
    {
        Assert.Equal(30, new BrightnessCard(NewSettings()).Order);
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessCardTests`
Expected: FAIL, `BrightnessCard` does not exist.

- [ ] **Step 4: Write the view model**

```csharp
using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;

namespace Plith.ViewModels;

/// <summary>
/// What the Brightness card shows: one number and the bar that draws it.
/// </summary>
public sealed class BrightnessCardViewModel : INotifyPropertyChanged
{
    private int _percent;

    /// <summary>0 to 100, already normalised from the device's own span by the caller.</summary>
    public int Percent
    {
        get => _percent;
        set
        {
            if (!Set(ref _percent, value)) return;
            OnPropertyChanged(nameof(Normalized));
            OnPropertyChanged(nameof(DisplayText));
            OnPropertyChanged(nameof(AccessibleSummary));
        }
    }

    public double Normalized => Math.Clamp(_percent / 100.0, 0, 1);

    public string DisplayText => string.Create(CultureInfo.CurrentCulture, $"{_percent}%");

    /// <summary>Read by the card view's live region. See AudioCardView for why the
    /// AutomationProperties belong on the UserControl root.</summary>
    public string AccessibleSummary => string.Create(CultureInfo.CurrentCulture, $"Brightness {_percent} percent");

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value;
        OnPropertyChanged(name);
        return true;
    }
}
```

- [ ] **Step 5: Write the card**

```csharp
using System.Windows.Threading;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// Brightness card. Visible from a brightness change until the OSD's show duration elapses,
/// and invisible the rest of the time.
///
/// That is unlike the Audio card, which is always visible because the OSD has no state in
/// which it says nothing about audio. Applying the same rule here would put a brightness row
/// on every volume press, for every user, including the laptop users whose panel this card
/// may never be able to reach.
///
/// The cost is that the show duration has two readers: this card and OsdHost's hide timer.
/// The alternative is plumbing an "OSD hidden" signal back from OsdHost through CardHost,
/// which today is deliberately fire and forget and holds no window reference. Reading one
/// setting from two places is the smaller price.
/// </summary>
public sealed class BrightnessCard : ICard
{
    private readonly SettingsService _settings;
    private readonly Action<TimeSpan, Action> _scheduleHide;
    private readonly DispatcherTimer? _timer;
    private bool _visible;

    /// <param name="scheduleHide">Restarts the visibility window. Defaults to a
    /// DispatcherTimer. A test supplies its own so the window can be closed on demand rather
    /// than waited out.</param>
    public BrightnessCard(SettingsService settings, Action<TimeSpan, Action>? scheduleHide = null)
    {
        _settings = settings;
        Vm = new BrightnessCardViewModel();

        if (scheduleHide is not null)
        {
            _scheduleHide = scheduleHide;
        }
        else
        {
            _timer = new DispatcherTimer();
            _timer.Tick += (_, _) => { _timer.Stop(); Hide(); };
            _scheduleHide = (after, _) =>
            {
                _timer.Stop();
                _timer.Interval = after;
                _timer.Start();
            };
        }
    }

    public string Id => "brightness";
    public string AccessibleName => "Brightness";
    public int Order => 30;
    public bool IsVisible => _visible;
    public object ViewModel => Vm;
    public BrightnessCardViewModel Vm { get; }

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    // Load-bearing for accessibility, not a debugging aid: WPF's ItemAutomationPeer names the
    // OSD's list container from this. See ICard.AccessibleName.
    public override string ToString() => AccessibleName;

    public void Activate() { }

    public void Deactivate() => _timer?.Stop();

    /// <summary>
    /// The screen's brightness is now <paramref name="percent"/>.
    ///
    /// Must be called on the UI dispatcher. Both callers sit on a worker thread of their own
    /// (a WMI callback, and the writer's pump) and both marshal before reaching here, because
    /// CardHost reconciles straight into a bound ObservableCollection.
    /// </summary>
    public void Report(int percent)
    {
        Vm.Percent = percent;

        var wasVisible = _visible;
        _visible = true;
        if (!wasVisible) VisibilityChanged?.Invoke();

        _scheduleHide(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs), Hide);

        ShowRequested?.Invoke(new ShowRequest(ShowReason.BrightnessChange, Id));
    }

    private void Hide()
    {
        if (!_visible) return;
        _visible = false;
        VisibilityChanged?.Invoke();
    }
}
```

- [ ] **Step 6: Run the tests and watch them pass**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter BrightnessCardTests`
Expected: PASS, 8 tests.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Cards/BrightnessCard.cs src/Plith/ViewModels/BrightnessCardViewModel.cs src/Plith/Cards/ShowRequest.cs tests/Plith.Tests/BrightnessCardTests.cs
git commit -m "feat(brightness): a card that appears for a change and then leaves"
```

---

### Task 5: The card's view

**Files:**
- Create: `src/Plith/Views/BrightnessCardView.xaml`, `src/Plith/Views/BrightnessCardView.xaml.cs`
- Modify: `src/Plith/Resources/CardTemplates.xaml`
- Modify: `src/Plith/Resources/PlithIcons.xaml`

**Interfaces:**
- Consumes: `BrightnessCardViewModel` from Task 4.
- Produces: `Plith.Views.BrightnessCardView`; the `IconBrightness` geometry resource.

- [ ] **Step 1: Add the icon geometry**

In `src/Plith/Resources/PlithIcons.xaml`, next to the other icons, on the same 24 unit grid
every other icon is drawn on:

```xml
<!-- A sun: one disc and eight rays, drawn rather than set from a font. See the note at the
     top of this file for why no icon font is acceptable here. -->
<GeometryGroup x:Key="IconBrightness">
    <EllipseGeometry Center="12,12" RadiusX="4.5" RadiusY="4.5" />
    <LineGeometry StartPoint="12,1.5" EndPoint="12,4" />
    <LineGeometry StartPoint="12,20" EndPoint="12,22.5" />
    <LineGeometry StartPoint="1.5,12" EndPoint="4,12" />
    <LineGeometry StartPoint="20,12" EndPoint="22.5,12" />
    <LineGeometry StartPoint="4.6,4.6" EndPoint="6.4,6.4" />
    <LineGeometry StartPoint="17.6,17.6" EndPoint="19.4,19.4" />
    <LineGeometry StartPoint="19.4,4.6" EndPoint="17.6,6.4" />
    <LineGeometry StartPoint="6.4,17.6" EndPoint="4.6,19.4" />
</GeometryGroup>
```

- [ ] **Step 2: Write the view**

`src/Plith/Views/BrightnessCardView.xaml`. Read `src/Plith/Views/AudioCardView.xaml` first
and mirror its structure: same icon size, same margins, same bar. The two cards sit in one
stack and must not look like they came from different products.

```xml
<UserControl x:Class="Plith.Views.BrightnessCardView"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:vm="clr-namespace:Plith.ViewModels"
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
             d:DataContext="{d:DesignInstance Type=vm:BrightnessCardViewModel}"
             mc:Ignorable="d"
             AutomationProperties.Name="{Binding AccessibleSummary}"
             AutomationProperties.LiveSetting="Polite">
    <!-- The two AutomationProperties above belong on this UserControl root and nowhere else:
         WPF gives a Grid no automation peer, so a name set there never reaches UI Automation.
         See LiveRegionAnnouncer for the measurement behind that. -->
    <UserControl.Resources>
        <ResourceDictionary>
            <ResourceDictionary.MergedDictionaries>
                <ResourceDictionary Source="pack://application:,,,/Plith;component/Resources/PlithIcons.xaml" />
            </ResourceDictionary.MergedDictionaries>
        </ResourceDictionary>
    </UserControl.Resources>

    <DockPanel LastChildFill="True">
        <Viewbox DockPanel.Dock="Left" Width="21" Height="21" Margin="0,0,10,0"
                 VerticalAlignment="Center">
            <Canvas Width="24" Height="24">
                <Path Data="{StaticResource IconBrightness}"
                      Stroke="{DynamicResource OsdTextPrimary}"
                      StrokeThickness="1.6" StrokeLineJoin="Round"
                      StrokeStartLineCap="Round" StrokeEndLineCap="Round" />
            </Canvas>
        </Viewbox>

        <TextBlock DockPanel.Dock="Right"
                   Text="{Binding DisplayText}"
                   Foreground="{DynamicResource OsdTextPrimary}"
                   VerticalAlignment="Center"
                   Margin="10,0,0,0" />

        <ProgressBar Style="{DynamicResource ModernProgressBarStyle}"
                     Minimum="0" Maximum="1"
                     Value="{Binding Normalized, Mode=OneWay}"
                     VerticalAlignment="Center" />
    </DockPanel>
</UserControl>
```

`src/Plith/Views/BrightnessCardView.xaml.cs`:

```csharp
using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class BrightnessCardView : UserControl
{
    public BrightnessCardView()
    {
        InitializeComponent();
        LiveRegionAnnouncer.Attach(this, nameof(BrightnessCardViewModel.AccessibleSummary));
    }
}
```

- [ ] **Step 3: Register the template**

In `src/Plith/Resources/CardTemplates.xaml`, after the Ambient entry:

```xml
    <DataTemplate DataType="{x:Type vm:BrightnessCardViewModel}">
        <views:BrightnessCardView />
    </DataTemplate>
```

- [ ] **Step 4: Build and run the gates**

```bash
taskkill //IM Plith.exe //F; taskkill //IM Plith.DropCatcher.exe //F
dotnet build Plith.slnx -m:1
powershell -File scripts/check-a11y.ps1
powershell -File scripts/check-contrast.ps1
powershell -File scripts/check-shared-xaml.ps1
```

Expected: build succeeds with exactly 3 warnings (the known CA1861 ones) and 0 errors, and
all three scripts exit 0. `check-contrast.ps1` discovers pairs by scanning XAML for a
Foreground and a Background on the same element or style, so the new view is picked up with
no registration step. If it reports a new failing pair, change the brush rather than the
threshold.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/BrightnessCardView.xaml src/Plith/Views/BrightnessCardView.xaml.cs src/Plith/Resources/CardTemplates.xaml src/Plith/Resources/PlithIcons.xaml
git commit -m "feat(brightness): draw the card next to the volume one"
```

---

### Task 6: Two hotkeys, where the service only ever held one

**Files:**
- Modify: `src/Plith/Services/HotkeyService.cs`
- Modify: `src/Plith/Services/SettingsModel.cs`
- Modify: `src/Plith/Services/SettingsService.cs`
- Test: `tests/Plith.Tests/HotkeyServiceTests.cs`, `tests/Plith.Tests/SettingsServiceTests.cs`

**Interfaces:**
- Consumes: the existing `HotkeyService`.
- Produces: `HotkeyService(DiagnosticLog? log = null, int hotkeyId = 1, bool noRepeat = true)`;
  settings `BrightnessEnabled`, `BrightnessStepPercent`, `BrightnessUpHotkeyMods`,
  `BrightnessUpHotkeyKey`, `BrightnessDownHotkeyMods`, `BrightnessDownHotkeyKey`.

- [ ] **Step 1: Write the failing tests**

Add to `tests/Plith.Tests/HotkeyServiceTests.cs`:

```csharp
    [Fact]
    public void TwoServicesCanCoexistWithDifferentIds()
    {
        // The summon hotkey is one binding; brightness needs two more. Each instance owns its
        // own message-only window, so the per-window ids do not collide, but the id must stop
        // being a constant for a second instance to be meaningful.
        using var up = new HotkeyService(hotkeyId: 2);
        using var down = new HotkeyService(hotkeyId: 3);

        Assert.True(up.Apply((uint)HotkeyService.HotkeyMods.Control | (uint)HotkeyService.HotkeyMods.Alt,
                             KeyInterop.VirtualKeyFromKey(Key.F13)));
        Assert.True(down.Apply((uint)HotkeyService.HotkeyMods.Control | (uint)HotkeyService.HotkeyMods.Alt,
                               KeyInterop.VirtualKeyFromKey(Key.F14)));
        Assert.True(up.IsBound);
        Assert.True(down.IsBound);
    }

    [Fact]
    public void RepeatIsAllowedWhenTheServiceIsBuiltThatWay()
    {
        // NoRepeat is right for the summon hotkey, where a held key should fire once.
        // Brightness is the opposite: holding it must keep moving the value, and the
        // coalescing writer is what makes that safe.
        using var repeating = new HotkeyService(hotkeyId: 4, noRepeat: false);
        Assert.False(repeating.NoRepeat);

        using var single = new HotkeyService(hotkeyId: 5);
        Assert.True(single.NoRepeat);
    }
```

Add to `tests/Plith.Tests/SettingsServiceTests.cs`, following the round-trip pattern already
in that file:

```csharp
    [Fact]
    public void BrightnessSettingsRoundTrip()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);

        var m = svc.Current.Clone();
        m.BrightnessEnabled = true;
        m.BrightnessStepPercent = 15;
        m.BrightnessUpHotkeyMods = 3;
        m.BrightnessUpHotkeyKey = 0x26;
        m.BrightnessDownHotkeyMods = 3;
        m.BrightnessDownHotkeyKey = 0x28;
        svc.Save(m);

        var reloaded = new SettingsService(path).Current;

        Assert.True(reloaded.BrightnessEnabled);
        Assert.Equal(15, reloaded.BrightnessStepPercent);
        Assert.Equal(3u, reloaded.BrightnessUpHotkeyMods);
        Assert.Equal(0x26, reloaded.BrightnessUpHotkeyKey);
        Assert.Equal(3u, reloaded.BrightnessDownHotkeyMods);
        Assert.Equal(0x28, reloaded.BrightnessDownHotkeyKey);
    }

    [Fact]
    public void BrightnessIsOffAndUnboundOnAFreshConfig()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var m = new SettingsService(path).Current;

        Assert.False(m.BrightnessEnabled);
        Assert.Equal(0u, m.BrightnessUpHotkeyMods);
        Assert.Equal(0, m.BrightnessUpHotkeyKey);
        Assert.Equal(10, m.BrightnessStepPercent);
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter "HotkeyServiceTests|SettingsServiceTests"`
Expected: FAIL, no `hotkeyId` parameter and no `BrightnessEnabled`.

- [ ] **Step 3: Parameterise `HotkeyService`**

Replace the `HotkeyId` constant and the constructor:

```csharp
    private readonly int _hotkeyId;

    /// <param name="hotkeyId">Unique per window. Each instance owns its own message-only
    /// window, so two instances could both use 1, but naming the id keeps a future
    /// shared-window refactor from silently overwriting one binding with another.</param>
    /// <param name="noRepeat">True for a hotkey that should fire once while held, which is
    /// what the summon hotkey wants. False for brightness, where holding the key must keep
    /// moving the value.</param>
    public HotkeyService(DiagnosticLog? log = null, int hotkeyId = 1, bool noRepeat = true)
    {
        _log = log;
        _hotkeyId = hotkeyId;
        NoRepeat = noRepeat;
    }

    /// <summary>Whether a held key fires once or repeats.</summary>
    public bool NoRepeat { get; }
```

Replace every `HotkeyId` use with `_hotkeyId`, and in `Apply` replace

```csharp
        else if (RegisterHotKey(_source.Handle, HotkeyId, mods | (uint)HotkeyMods.NoRepeat, (uint)vk))
```

with

```csharp
        else if (RegisterHotKey(_source.Handle, _hotkeyId,
                                NoRepeat ? mods | (uint)HotkeyMods.NoRepeat : mods, (uint)vk))
```

- [ ] **Step 4: Add the settings**

In `SettingsModel.cs`, next to the summon hotkey fields:

```csharp
    /// <summary>Off by default. New surfaces in this product ship off.</summary>
    public bool BrightnessEnabled { get; set; }

    /// <summary>How far one key press moves brightness, as a percentage of each display's own
    /// span rather than of 0 to 100. DDC/CI does not require a minimum of zero.</summary>
    public int BrightnessStepPercent { get; set; } = 10;

    /// <summary>Modifier mask and virtual key for the brightness-up hotkey. Same layout as
    /// the summon hotkey: Alt=1, Ctrl=2, Shift=4, Win=8. Both zero means unbound.</summary>
    public uint BrightnessUpHotkeyMods { get; set; }
    public int BrightnessUpHotkeyKey { get; set; }

    public uint BrightnessDownHotkeyMods { get; set; }
    public int BrightnessDownHotkeyKey { get; set; }

    public bool HasBrightnessHotkeys =>
        BrightnessUpHotkeyMods != 0 && BrightnessUpHotkeyKey != 0 &&
        BrightnessDownHotkeyMods != 0 && BrightnessDownHotkeyKey != 0;
```

Add all six to `Clone()`.

In `SettingsService.cs`, add a `SectionBrightness` constant of `"Brightness"` alongside the
existing section constants, then in the load block:

```csharp
                BrightnessEnabled = ParseBool(data[SectionBrightness]["BrightnessEnabled"], false),
                BrightnessStepPercent = ParseInt(data[SectionBrightness]["BrightnessStepPercent"], 10, 1, 50),
                BrightnessUpHotkeyMods = ParseUInt(data[SectionBrightness]["BrightnessUpHotkeyMods"], 0),
                BrightnessUpHotkeyKey = ParseInt(data[SectionBrightness]["BrightnessUpHotkeyKey"], 0, 0, 255),
                BrightnessDownHotkeyMods = ParseUInt(data[SectionBrightness]["BrightnessDownHotkeyMods"], 0),
                BrightnessDownHotkeyKey = ParseInt(data[SectionBrightness]["BrightnessDownHotkeyKey"], 0, 0, 255),
```

and in the save block:

```csharp
        data[SectionBrightness]["BrightnessEnabled"] = m.BrightnessEnabled.ToString(inv);
        data[SectionBrightness]["BrightnessStepPercent"] = m.BrightnessStepPercent.ToString(inv);
        data[SectionBrightness]["BrightnessUpHotkeyMods"] = m.BrightnessUpHotkeyMods.ToString(inv);
        data[SectionBrightness]["BrightnessUpHotkeyKey"] = m.BrightnessUpHotkeyKey.ToString(inv);
        data[SectionBrightness]["BrightnessDownHotkeyMods"] = m.BrightnessDownHotkeyMods.ToString(inv);
        data[SectionBrightness]["BrightnessDownHotkeyKey"] = m.BrightnessDownHotkeyKey.ToString(inv);
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `dotnet test tests/Plith.Tests -m:1 --nologo`
Expected: PASS, and the count is now the Task 5 total plus 4.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Services/HotkeyService.cs src/Plith/Services/SettingsModel.cs src/Plith/Services/SettingsService.cs tests/Plith.Tests/HotkeyServiceTests.cs tests/Plith.Tests/SettingsServiceTests.cs
git commit -m "feat(brightness): let more than one hotkey exist, and let one repeat"
```

---

### Task 7: The sense half

**Files:**
- Create: `src/Plith/Services/Brightness/BrightnessMonitor.cs`
- Modify: `src/Plith/Plith.csproj` (add `System.Management` if it is not already referenced)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `BrightnessMonitor(Dispatcher dispatcher, DiagnosticLog? log = null)`;
  `void Start()`; `event Action<int>? Changed`; `IDisposable`.

**There is no unit test for this task and there cannot be one here.** It needs an internal
panel to raise the event, and this machine is a desktop. That is recorded in the spec as the
honest limit of this slice. What can be checked is that its absence is harmless, which
Step 3 does.

- [ ] **Step 1: Write the monitor**

```csharp
using System.Management;
using System.Windows.Threading;

namespace Plith.Services.Brightness;

/// <summary>
/// Notices that the screen's brightness changed, whoever changed it.
///
/// Windows raises WmiMonitorBrightnessEvent on every brightness change of an internal panel
/// and carries the new percentage in the event, so nothing has to be queried afterwards. That
/// is why this feature hooks no keys: laptop brightness keys are consumed in the driver stack
/// and there is no VK_BRIGHTNESS in the Windows SDK to hook even if they were not. Listening
/// to the change also covers the Settings slider, the Quick Settings panel, and any other
/// application.
///
/// It covers INTERNAL PANELS ONLY. An external monitor never raises it, because DDC/CI is
/// request and response and the monitor never announces anything. On a desktop this class
/// starts, never fires, and costs nothing, which is the correct behaviour rather than a
/// degraded one.
/// </summary>
public sealed class BrightnessMonitor : IDisposable
{
    private readonly Dispatcher _dispatcher;
    private readonly DiagnosticLog? _log;
    private ManagementEventWatcher? _watcher;
    private bool _disposed;

    public BrightnessMonitor(Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _dispatcher = dispatcher;
        _log = log;
    }

    /// <summary>Raised on the UI dispatcher with the new percentage.</summary>
    public event Action<int>? Changed;

    public void Start()
    {
        if (_watcher is not null || _disposed) return;

        try
        {
            var scope = new ManagementScope(@"\\.\root\wmi");
            var query = new WqlEventQuery("SELECT * FROM WmiMonitorBrightnessEvent");
            _watcher = new ManagementEventWatcher(scope, query);
            _watcher.EventArrived += OnEventArrived;
            _watcher.Start();
            _log?.Info("Brightness", "Watching WmiMonitorBrightnessEvent.");
        }
        catch (ManagementException ex)
        {
            // No internal panel, or WMI is unavailable. Degrade silently, the way
            // MediaSessionClient does when SMTC is missing.
            _log?.Info("Brightness", $"No brightness event source: {ex.Message}");
            _watcher = null;
        }
        catch (UnauthorizedAccessException)
        {
            _watcher = null;
        }
    }

    private void OnEventArrived(object sender, EventArrivedEventArgs e)
    {
        // WMI raises on a worker thread. CardHost reconciles straight into an
        // ObservableCollection bound to a live ItemsControl and its own documentation names
        // this exact case as the expected cause of a crash deep inside the WPF binding
        // engine. Nothing may reach a card from here without this hop.
        int percent;
        try
        {
            percent = Convert.ToInt32(e.NewEvent["Brightness"], System.Globalization.CultureInfo.InvariantCulture);
        }
        catch (Exception)
        {
            return;
        }

        if (_disposed) return;
        _dispatcher.BeginInvoke(new Action(() => { if (!_disposed) Changed?.Invoke(percent); }));
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (_watcher is null) return;
        try
        {
            _watcher.EventArrived -= OnEventArrived;
            _watcher.Stop();
            _watcher.Dispose();
        }
        catch (ManagementException) { }
        _watcher = null;
    }
}
```

- [ ] **Step 2: Add the package reference if it is missing**

Check first: `grep -n "System.Management" src/Plith/Plith.csproj`. If absent, add
`<PackageReference Include="System.Management" Version="9.0.0" />` to the existing
`ItemGroup` of package references, then `dotnet restore`.

- [ ] **Step 3: Prove its absence is harmless on this machine**

Add a temporary fact, run it, then delete it:

```csharp
    [Fact]
    public void TemporaryMonitorStartProbe()
    {
        using var monitor = new BrightnessMonitor(System.Windows.Threading.Dispatcher.CurrentDispatcher);
        monitor.Start();   // must not throw on a desktop with no internal panel
        monitor.Dispose();
    }
```

Run: `dotnet test tests/Plith.Tests -m:1 --nologo --filter TemporaryMonitorStartProbe`
Expected: PASS. A throw here means the catch list is wrong and must be widened before this
ships, because `App` calls `Start()` during startup and an unhandled throw there takes the
whole product down on every desktop.

- [ ] **Step 4: Commit**

```bash
git add src/Plith/Services/Brightness/BrightnessMonitor.cs src/Plith/Plith.csproj
git commit -m "feat(brightness): notice a change rather than a key press"
```

---

### Task 8: Wiring, Settings, and the hardware run

**Files:**
- Modify: `src/Plith/App.xaml.cs`
- Modify: `src/Plith/Views/SettingsWindow.xaml`, `src/Plith/Views/SettingsWindow.xaml.cs`

**Interfaces:**
- Consumes: everything from Tasks 1 to 7.
- Produces: a running feature.

- [ ] **Step 1: Wire it in `App.xaml.cs`**

Add fields beside `_audioCard` and friends:

```csharp
    private BrightnessCard? _brightnessCard;
    private BrightnessMonitor? _brightnessMonitor;
    private BrightnessWriter? _brightnessWriter;
    private IReadOnlyList<IBrightnessDevice> _brightnessDevices = [];
    private HotkeyService? _brightnessUpHotkey;
    private HotkeyService? _brightnessDownHotkey;
```

After the existing card construction and before `_cardHost.Register(_audioCard)`:

```csharp
        _brightnessCard = new BrightnessCard(_settings);
```

and register it after the audio card:

```csharp
        _cardHost.Register(_brightnessCard);  // Order 30 - below audio, and only while it has something to say
```

After `CardHost` is built:

```csharp
        // Discovery costs one DDC/CI read per attached monitor, measured at 56 ms each, so it
        // happens once at startup rather than per key press.
        _brightnessDevices = BrightnessDiscovery.Discover();
        _brightnessWriter = new BrightnessWriter(_brightnessDevices);

        // The pump runs on the thread pool; the card must be touched on the dispatcher.
        _brightnessWriter.Wrote += value => _osd.Dispatcher.BeginInvoke(
            new Action(() => _brightnessCard?.Report(ToPercent(value))));

        _brightnessMonitor = new BrightnessMonitor(_osd.Dispatcher, _diagnosticLog);
        _brightnessMonitor.Changed += percent => _brightnessCard?.Report(percent);
        _brightnessMonitor.Start();

        ApplyBrightnessHotkeys(_settings.Current);
        _settings.Changed += ApplyBrightnessHotkeys;
```

with these helpers on `App`:

```csharp
    /// <summary>
    /// The first device's reading expressed as 0 to 100, because the card shows one number and
    /// the devices may not share a span. The first device is the one the OSD reports, which is
    /// a simplification the spec records: they all move together.
    /// </summary>
    private int ToPercent(int value)
    {
        if (_brightnessDevices.Count == 0) return value;
        if (!_brightnessDevices[0].TryRead(out var reading)) return value;

        var span = reading.Max - reading.Min;
        if (span <= 0) return 100;
        return (int)Math.Round((value - reading.Min) * 100.0 / span);
    }

    private void ApplyBrightnessHotkeys(SettingsModel m)
    {
        var wanted = m.BrightnessEnabled && _brightnessDevices.Count > 0;

        if (!wanted)
        {
            _brightnessUpHotkey?.Apply(0, 0);
            _brightnessDownHotkey?.Apply(0, 0);
            return;
        }

        // noRepeat: false, so holding the key keeps moving the value. The coalescing writer
        // is what makes that safe at 56 ms per write.
        _brightnessUpHotkey ??= BuildBrightnessHotkey(hotkeyId: 2, up: true);
        _brightnessDownHotkey ??= BuildBrightnessHotkey(hotkeyId: 3, up: false);

        _brightnessUpHotkey.Apply(m.BrightnessUpHotkeyMods, m.BrightnessUpHotkeyKey);
        _brightnessDownHotkey.Apply(m.BrightnessDownHotkeyMods, m.BrightnessDownHotkeyKey);
    }

    private HotkeyService BuildBrightnessHotkey(int hotkeyId, bool up)
    {
        var service = new HotkeyService(_diagnosticLog, hotkeyId, noRepeat: false);
        service.Pressed += () => StepBrightness(up);
        return service;
    }

    private void StepBrightness(bool up)
    {
        if (_brightnessDevices.Count == 0 || _brightnessWriter is null) return;
        if (!_brightnessDevices[0].TryRead(out var reading)) return;

        _brightnessWriter.Request(
            BrightnessStep.Next(reading, _settings.Current.BrightnessStepPercent, up));
    }
```

Dispose them in the existing teardown, before `CardHost`:

```csharp
        DisposeStep("BrightnessMonitor",  () => _brightnessMonitor?.Dispose());
        DisposeStep("BrightnessHotkeys",  () => { _brightnessUpHotkey?.Dispose(); _brightnessDownHotkey?.Dispose(); });
        DisposeStep("BrightnessDevices",  () => { foreach (var d in _brightnessDevices) (d as IDisposable)?.Dispose(); });
```

- [ ] **Step 2: Add the Settings group**

In `SettingsWindow.xaml`, copy the shape of the existing media group (the one holding
`AutoShowMediaToggle`) and add a Brightness group with:

- `BrightnessToggle`, a `ToggleSwitchStyle` checkbox, labelled `BrightnessLabel` with the
  text "Brightness hotkeys" and the hint "Off by default. When on, the two keys below change
  every display that answered, and the OSD shows the new level."
- Two hotkey capture controls, reusing the control already bound for the summon hotkey,
  named `BrightnessUpHotkeyBox` and `BrightnessDownHotkeyBox`.
- A step slider or numeric box named `BrightnessStepBox`, 1 to 50.

Every interactive control needs `AutomationProperties.LabeledBy` pointing at its label, or
`check-a11y.ps1` fails the build.

In `SettingsWindow.xaml.cs`, load them next to `AutoShowMediaToggle.IsChecked = m.AutoShowOnMedia;`
and save them next to `m.AutoShowOnMedia = AutoShowMediaToggle.IsChecked == true;`.

**Hide the whole group when `BrightnessDiscovery.Discover()` found nothing.** A section
offering controls that do nothing is worse than no section. The count can be handed to the
window the same way the endpoint list already is.

- [ ] **Step 3: Build and run every gate**

```bash
taskkill //IM Plith.exe //F; taskkill //IM Plith.DropCatcher.exe //F
dotnet build Plith.slnx -m:1
dotnet test Plith.slnx -m:1 --nologo
powershell -File scripts/check-a11y.ps1
powershell -File scripts/check-contrast.ps1
powershell -File scripts/check-shared-xaml.ps1
```

Expected: build 3 warnings 0 errors, every test passes, all three scripts exit 0.

- [ ] **Step 4: Commit**

```bash
git add src/Plith/App.xaml.cs src/Plith/Views/SettingsWindow.xaml src/Plith/Views/SettingsWindow.xaml.cs
git commit -m "feat(brightness): wire the two halves to the OSD and to Settings"
```

- [ ] **Step 5: Drive it on hardware, and write down what happened**

**A green build reaches none of this.** The suite is not STA and the OSD renders in a layered
window that cannot be captured over RDP. A Phase 6 slice shipped green and crashed on the
first hover; the accessibility pass shipped green with four defects in it. Run the product.

Launch `src/Plith/bin/Debug/net10.0-windows10.0.22000.0/Plith.exe`, turn the feature on in
Settings, bind Ctrl+Alt+Up and Ctrl+Alt+Down, and check each of these:

1. One press changes the monitor's brightness, and the OSD appears showing the new level.
2. The level in the OSD matches what the monitor actually did. Confirm with an independent
   read rather than by eye.
3. Holding the key ramps smoothly and **stops when the key comes up**, with no run of writes
   continuing afterwards. That is the coalescing working, and it is the thing most likely to
   be wrong.
4. The brightness row is **not** present when the OSD appears for a volume key.
5. The OSD in notch mode uses the short HUD shape, the one a volume key gets, not the full
   frame.
6. Turning the feature off in Settings unbinds both keys immediately.

Record the answers in `docs/PHASE6-VERIFICATION.md` under a new Brightness section, including
anything that did not behave as written. **The sense half stays unverified**: there is no
laptop here, so `WmiMonitorBrightnessEvent` has never fired in this product. Say so rather
than implying coverage.

- [ ] **Step 6: Commit the record**

```bash
git add docs/PHASE6-VERIFICATION.md
git commit -m "docs(brightness): record what the hardware run showed"
```

---

## Self-Review

**Spec coverage:** §3.1 is Task 7, §3.2 is Tasks 1 and 2, §3.3 is Task 3, §3.4 is Tasks 4 and
5, §3.5 is Tasks 6 and 8, §3.6 is Task 8's `StepBrightness` writing to every device, §4
degradation is spread across Task 2 (nothing discovered), Task 3 (a refusing device) and
Task 7 (no event source), §5 is Task 8 Step 2, §6 is the test file in each task plus Task 8
Step 5. §7's out-of-scope items appear in no task, which is correct.

**Placeholders:** Task 8 Step 2 describes the Settings markup rather than printing it, and
that is a real limit of this plan: the group must match the surrounding markup, which is
long and is best read in place. It names every control, its label text, its hint text, and
the two methods to edit. Task 2 Step 6 and Task 7 Step 3 are deliberately temporary code,
marked as such, with an instruction to delete them.

**Type consistency:** `BrightnessReading(Min, Current, Max)`, `IBrightnessDevice.TryRead/TryWrite/Id`,
`BrightnessStep.Next(reading, stepPercent, up)`, `BrightnessDiscovery.KeepAnswering/Discover`,
`BrightnessWriter.Request/Wrote`, `BrightnessCard.Report/Vm/Order`,
`BrightnessCardViewModel.Percent/Normalized/DisplayText/AccessibleSummary`,
`HotkeyService(log, hotkeyId, noRepeat)` and `NoRepeat` are spelled the same in every task
that mentions them.

**The one thing that could still sink it:** Task 8's percentage. The card shows one number
while every device is written together, and the number comes from the first device's span. On
a machine whose two monitors report different ranges the OSD will be right about one of them
and approximate about the other. This machine has one monitor, so the plan cannot find out.
It is a display inaccuracy rather than a control bug, and the fix, per-device rows in the
card, is a bigger surface than this slice should carry.
