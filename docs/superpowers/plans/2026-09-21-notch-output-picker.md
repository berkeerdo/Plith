# Notch Output Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The media page's output control turns the page into a list of outputs; pressing one
changes the system's default render endpoint and returns to what is playing.

**Architecture:** Five pieces, in dependency order. A set-aware shortener fixes labels that
collide today. A pure model turns endpoints plus the current id into the six cells the grid
draws, including a named overflow door. An interop service writes the default through the
undocumented `IPolicyConfig`. The media page gains a second mode holding the grid. The driver
script presses it on hardware and puts the device back.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`), xunit, NAudio 2.3.0 for
enumeration, raw COM interop for `IPolicyConfig`, PowerShell 7 harnesses in `scripts/`.

**Spec:** `docs/superpowers/specs/2026-09-21-notch-output-picker-design.md`

## Global Constraints

- **The frame does not change.** `NotchGeometry.OpenFrameDip` is `356x116`; the picker lives in
  the same 73 DIP content band (14 top, 29 bottom of which 20 is the page rail's lane).
- **No absolute colours in the cells.** This is the defect that killed the System Controls page:
  hard-coded blue-grey tiles on an accent-tinted panel read as cold blue rectangles on a lime
  accent. Every colour comes from the notch palette.
- **All code, comments and commits in English.** Conventional Commits. No AI attribution.
- **No em dashes, and no hyphen as a sentence separator**, in code or docs.
- **`AutomationProperties` only on elements WPF gives a peer.** `check-a11y.ps1` enforces it and
  reads code-behind too. Every cell is therefore a `Button`, not a `Border`.
- **Icons are drawn geometry from `PlithIcons.xaml`.** A Segoe MDL2 code point fails the lint.
- **The suite is not STA**, so nothing that constructs a `UserControl` can be unit-tested.
  Testable logic goes in pure classes.
- **Baseline to keep green:** `dotnet build` 0 warnings for `Plith` (3 pre-existing CA1861
  warnings live in `Plith.Installer.Tests`), `dotnet test` at **537 passing** (520 + 17,
  measured 2026-09-21), `check-a11y.ps1` and `check-contrast.ps1` both passing.
- **A running Plith locks its own binary.** Stop `Plith` and `Plith.DropCatcher` before building
  after any driver run, or the build fails with MSB3021 naming a file copy.

---

### Task 1: Labels that stay distinct

Two endpoints on this machine currently shorten to the same string, and the Settings combo box
shows them as two identical rows. Uniqueness is a property of the set, so it is fixed where the
set is built.

**Files:**
- Modify: `src/Plith/Services/AudioLabel.cs`
- Modify: `src/Plith/Services/WindowsAudioClient.cs:217` (`EnumerateRenderEndpoints`)
- Test: `tests/Plith.Tests/AudioLabelTests.cs`

**Interfaces:**
- Consumes: the existing `AudioLabel.Shorten(string?)`.
- Produces: `static IReadOnlyList<string> AudioLabel.ShortenAll(IReadOnlyList<string> names)`.

- [x] **Step 1: Write the failing tests**

Append to `tests/Plith.Tests/AudioLabelTests.cs`:

```csharp
    [Fact]
    public void ShortenAll_LeavesDistinctNamesShortened()
    {
        var shortened = AudioLabel.ShortenAll([
            "Hoparlör (Realtek(R) Audio)",
            "PG27AQDM (NVIDIA High Definition Audio)",
        ]);

        Assert.Equal(["Hoparlör (Realtek(R) Audio)", "PG27AQDM (NVIDIA High)"], shortened);
    }

    [Fact]
    public void ShortenAll_GivesCollidingNamesTheirFullNamesBack()
    {
        // Measured on this machine on 2026-09-21, and the reason this method exists: both of
        // these keep the adapter's first two words, so Shorten reduces them to the same string
        // and the Settings endpoint combo shows two identical rows.
        var shortened = AudioLabel.ShortenAll([
            "Hoparlör (Steam Streaming Speakers)",
            "Hoparlör (Steam Streaming Microphone)",
        ]);

        Assert.Equal([
            "Hoparlör (Steam Streaming Speakers)",
            "Hoparlör (Steam Streaming Microphone)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_ExpandsOnlyTheCollidingGroup()
    {
        var shortened = AudioLabel.ShortenAll([
            "Hoparlör (Steam Streaming Speakers)",
            "PG27AQDM (NVIDIA High Definition Audio)",
            "Hoparlör (Steam Streaming Microphone)",
        ]);

        Assert.Equal([
            "Hoparlör (Steam Streaming Speakers)",
            "PG27AQDM (NVIDIA High)",
            "Hoparlör (Steam Streaming Microphone)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_WithAThreeWayCollisionExpandsAllThree()
    {
        var shortened = AudioLabel.ShortenAll([
            "Speakers (Virtual Audio Cable A)",
            "Speakers (Virtual Audio Cable B)",
            "Speakers (Virtual Audio Cable C)",
        ]);

        Assert.Equal([
            "Speakers (Virtual Audio Cable A)",
            "Speakers (Virtual Audio Cable B)",
            "Speakers (Virtual Audio Cable C)",
        ], shortened);
    }

    [Fact]
    public void ShortenAll_OfAnEmptyListIsEmpty()
    {
        Assert.Empty(AudioLabel.ShortenAll([]));
    }

    [Fact]
    public void ShortenAll_KeepsTheOrderItWasGiven()
    {
        // The caller pairs these back up with endpoint ids by index, so a reordering here would
        // route audio to the wrong device with a perfectly plausible label on it.
        var shortened = AudioLabel.ShortenAll([
            "PG27AQDM (NVIDIA High Definition Audio)",
            "Hoparlör (Realtek(R) Audio)",
        ]);

        Assert.Equal("PG27AQDM (NVIDIA High)", shortened[0]);
    }
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests --filter AudioLabelTests`
Expected: FAIL to compile, `ShortenAll` does not exist.

- [x] **Step 3: Write the implementation**

In `src/Plith/Services/AudioLabel.cs`, after `Shorten`:

```csharp
    /// <summary>
    /// Shorten a whole list, keeping every entry distinct.
    ///
    /// <see cref="Shorten"/> cannot do this on its own, and that is the point of having both:
    /// uniqueness is a property of the SET, and a function given one name cannot see the name it
    /// is about to collide with. Measured on 2026-09-21: "Hoparlör (Steam Streaming Speakers)"
    /// and "Hoparlör (Steam Streaming Microphone)" both keep the adapter's first two words, so
    /// both come back as "Hoparlör (Steam Streaming)" and the settings dropdown has been showing
    /// two identical rows for two phases.
    ///
    /// Any group whose shortened form collides gets its FULL names back, rather than a longer
    /// truncation: three words would collide on the next machine, and a device name is not worth
    /// guessing at when the cost of guessing is routing audio to the wrong output.
    ///
    /// Order is preserved, because callers pair the result back up with endpoint ids by index.
    /// </summary>
    public static IReadOnlyList<string> ShortenAll(IReadOnlyList<string> names)
    {
        ArgumentNullException.ThrowIfNull(names);

        var shortened = new string[names.Count];
        for (var i = 0; i < names.Count; i++) shortened[i] = Shorten(names[i]);

        var counts = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        foreach (var s in shortened) counts[s] = counts.GetValueOrDefault(s) + 1;

        for (var i = 0; i < shortened.Length; i++)
        {
            if (counts[shortened[i]] > 1) shortened[i] = (names[i] ?? string.Empty).Trim();
        }

        return shortened;
    }
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests --filter AudioLabelTests`
Expected: PASS.

- [x] **Step 5: Use it where the list is built**

In `src/Plith/Services/WindowsAudioClient.cs`, `EnumerateRenderEndpoints` currently shortens each
name as it adds it. Collect the raw names and ids first, then shorten the whole list:

```csharp
    public static IReadOnlyList<WindowsAudioEndpointInfo> EnumerateRenderEndpoints()
    {
        var ids = new List<string>();
        var names = new List<string>();
        try
        {
            using var en = new MMDeviceEnumerator();
            var devs = en.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active);
            foreach (var d in devs)
            {
                // Each device read in its own try: one that dies mid-enumeration must not take
                // the whole list with it, which is what the per-item try here has always done.
                try { ids.Add(d.ID); names.Add(d.FriendlyName); }
                catch { }
            }
        }
        catch { }

        // Shortened as a SET, so two devices sharing their adapter's first two words do not both
        // come back as the same string. See AudioLabel.ShortenAll.
        var labels = AudioLabel.ShortenAll(names);
        var list = new List<WindowsAudioEndpointInfo>(ids.Count);
        for (var i = 0; i < ids.Count; i++) list.Add(new WindowsAudioEndpointInfo(ids[i], labels[i]));
        return list;
    }
```

Read the existing method before replacing it and keep whatever else it does (the outer `try` and
the per-device `try` are both already there for reasons its own comments state).

- [x] **Step 6: Build, test and commit**

Run: `dotnet build` (0 warnings for Plith) and `dotnet test` (543 passing: 537 plus 6).

```bash
git add src/Plith/Services/AudioLabel.cs src/Plith/Services/WindowsAudioClient.cs \
        tests/Plith.Tests/AudioLabelTests.cs
git commit -m "fix(audio): keep endpoint labels distinct in a list

Shorten keeps the adapter's first two words, so on this machine
\"Steam Streaming Speakers\" and \"Steam Streaming Microphone\" both come
back as \"Hoparlör (Steam Streaming)\" and the settings endpoint combo has
been showing two identical rows. Uniqueness is a property of the set, so
ShortenAll expands any colliding group back to full names and the list is
shortened where it is built."
```

---

### Task 2: Writing the default endpoint

**Files:**
- Create: `src/Plith/Services/OutputDeviceSwitcher.cs`
- Create: `scripts/probe-output-switch.ps1`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `static bool OutputDeviceSwitcher.TrySetDefault(string endpointId, DiagnosticLog? log = null)`.

There is no unit test here and there cannot be: the whole content of this task is a call into an
undocumented COM interface on the live machine. Its verification is the probe in step 3, which is
the same trick the roadmap records for the earlier measurement: re-select the endpoint that is
ALREADY default, so the call path is proved without changing what anyone is listening to.

- [x] **Step 1: Write the interop**

Create `src/Plith/Services/OutputDeviceSwitcher.cs`:

```csharp
using System.Runtime.InteropServices;

namespace Plith.Services;

/// <summary>
/// Changes Windows' default render endpoint.
///
/// There is NO documented API for this. Windows' own Sound control panel uses an undocumented
/// COM interface, IPolicyConfig, and so does every application that offers the feature. The
/// GUIDs below are the ones docs/ROADMAP.md recorded when this was first measured here, during
/// the System Controls work: SetDefaultEndpoint returned S_OK. That code was never committed, so
/// this is a rewrite of something that was only ever proved by a probe.
///
/// The risk is bounded rather than absent: an interface with no contract behind it can break on
/// any Windows release, and when it does TrySetDefault returns false, the picker says so, and
/// the door to ms-settings:sound still works.
/// </summary>
public static class OutputDeviceSwitcher
{
    /// <summary>
    /// Make <paramref name="endpointId"/> the default output.
    ///
    /// Console and Multimedia only. That pair is what Windows' own "Set as Default Device"
    /// writes; Communications is a separate action there and moving it would move Discord and
    /// Teams audio, which nobody asked this for.
    /// </summary>
    public static bool TrySetDefault(string endpointId, DiagnosticLog? log = null)
    {
        if (string.IsNullOrWhiteSpace(endpointId)) return false;

        object? instance = null;
        try
        {
            var type = Type.GetTypeFromCLSID(PolicyConfigClsid, throwOnError: false);
            if (type is null)
            {
                log?.Warn("OutputDeviceSwitcher", "PolicyConfig class is not registered on this system.");
                return false;
            }

            instance = Activator.CreateInstance(type);
            if (instance is not IPolicyConfig config)
            {
                log?.Warn("OutputDeviceSwitcher", "PolicyConfig does not implement the expected interface.");
                return false;
            }

            // eConsole = 0, eMultimedia = 1. eCommunications = 2 is deliberately not written.
            var console = config.SetDefaultEndpoint(endpointId, 0);
            var multimedia = config.SetDefaultEndpoint(endpointId, 1);

            if (console != 0 || multimedia != 0)
            {
                log?.Warn("OutputDeviceSwitcher",
                    $"SetDefaultEndpoint failed: console=0x{console:X8}, multimedia=0x{multimedia:X8}");
                return false;
            }

            log?.Info("OutputDeviceSwitcher", "Default output set (console and multimedia).");
            return true;
        }
        catch (Exception ex)
        {
            log?.Warn("OutputDeviceSwitcher", $"SetDefaultEndpoint threw: {ExceptionText.Describe(ex)}");
            return false;
        }
        finally
        {
            if (instance is not null && Marshal.IsComObject(instance)) Marshal.ReleaseComObject(instance);
        }
    }

    private static readonly Guid PolicyConfigClsid = new("870af99c-171d-4f9e-af0d-e63df40c2bc9");

    /// <summary>
    /// The undocumented interface, declared only as far as the method this needs.
    ///
    /// The nine reserved slots are LOAD-BEARING: a COM interface is a vtable, so the methods
    /// before SetDefaultEndpoint have to be declared for the call to land on the right function
    /// pointer. Omitting them would not fail to compile, it would call something else.
    /// </summary>
    [ComImport]
    [Guid("f8679f50-850a-41cf-9c72-430f290290c8")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IPolicyConfig
    {
        int GetMixFormat_Reserved();
        int GetDeviceFormat_Reserved();
        int ResetDeviceFormat_Reserved();
        int SetDeviceFormat_Reserved();
        int GetProcessingPeriod_Reserved();
        int SetProcessingPeriod_Reserved();
        int GetShareMode_Reserved();
        int SetShareMode_Reserved();
        int GetPropertyValue_Reserved();

        [PreserveSig]
        int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string deviceId, uint role);
    }
}
```

Check `DiagnosticLog`'s actual method names before writing this (`Info` and `Warn` are used
elsewhere in `Services`; match what is there) and `ExceptionText`'s helper name the same way.

- [x] **Step 2: Build**

Run: `dotnet build`
Expected: succeeds, 0 warnings for Plith.

- [x] **Step 3: Write the probe and run it**

Create `scripts/probe-output-switch.ps1`. It reads the CURRENT default through NAudio, calls
`TrySetDefault` with that same id, and reports. A no-op switch proves the call path without
changing what anyone is listening to, which is exactly how this was measured the first time.

```powershell
#requires -Version 7
<#
.SYNOPSIS
  Proves OutputDeviceSwitcher's call path without changing anyone's audio.

.DESCRIPTION
  IPolicyConfig is undocumented, so "it compiles" says nothing. This asks the machine.

  It re-selects the endpoint that is ALREADY default, so a success is a real S_OK from the real
  interface and nothing anyone is listening to moves. That is the same trick docs/ROADMAP.md
  records from the System Controls measurement.

  Run with: pwsh -File scripts/probe-output-switch.ps1
#>
[CmdletBinding()]
param([string]$Configuration = 'Debug')

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $n = ($e.Name -split ',')[0]; $c = Join-Path $bin "$n.dll"
    if (Test-Path $c) { return [Reflection.Assembly]::LoadFrom($c) }
    $null
})
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'Plith.dll'))
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'NAudio.Wasapi.dll'))

$en = New-Object NAudio.CoreAudioApi.MMDeviceEnumerator
$before = $en.GetDefaultAudioEndpoint('Render', 'Multimedia')
"default before: $($before.FriendlyName)"
"              : $($before.ID)"

$ok = [Plith.Services.OutputDeviceSwitcher]::TrySetDefault($before.ID, $null)
"TrySetDefault : $ok"

$after = $en.GetDefaultAudioEndpoint('Render', 'Multimedia')
"default after : $($after.FriendlyName)"

if (-not $ok) { throw 'TrySetDefault returned false: the interop does not work on this system.' }
if ($after.ID -ne $before.ID) { throw 'The default moved. It was asked to stay where it was.' }
'PASS: the call path works and nothing moved.'
```

Run: `pwsh -File scripts/probe-output-switch.ps1`
Expected: `TrySetDefault : True` and `PASS`.

If it returns false, read the `Warn` line the switcher logs. The two likely causes are the class
not being registered (a stripped SKU) and the interface GUID having changed; both are reportable
facts, not things to work around by guessing another GUID.

- [x] **Step 4: Commit**

```bash
git add src/Plith/Services/OutputDeviceSwitcher.cs scripts/probe-output-switch.ps1
git commit -m "feat(audio): write the default output through IPolicyConfig

There is no documented API for this; Windows' own Sound panel uses the
same undocumented interface. The nine reserved vtable slots before
SetDefaultEndpoint are load-bearing: a COM interface is a vtable, so
omitting them would compile and then call the wrong function.

Console and Multimedia only. Communications is a separate action in
Windows' own UI and moving it would move Discord and Teams audio.

Proved by scripts/probe-output-switch.ps1, which re-selects the endpoint
that is already default: a real S_OK from the real interface with nobody's
audio moving."
```

---

### Task 3: The grid's contents, as a pure model

**Files:**
- Create: `src/Plith/Services/OutputPickerModel.cs`
- Modify: `src/Plith/Views/Presentation/NotchGeometry.cs`
- Test: `tests/Plith.Tests/OutputPickerModelTests.cs`
- Test: `tests/Plith.Tests/NotchGeometryTests.cs`

**Interfaces:**
- Consumes: `WindowsAudioEndpointInfo(string Id, string FriendlyName)` from Task 1's file.
- Produces:
  - `sealed record OutputChoice(string Id, string Label, bool IsCurrent, bool IsOverflow = false)`
  - `static IReadOnlyList<OutputChoice> OutputPickerModel.Cells(IReadOnlyList<WindowsAudioEndpointInfo> endpoints, string currentId, int capacity)`
  - `NotchGeometry.OutputPickerColumns`, `OutputPickerRows`, `OutputPickerCapacity`

- [x] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/OutputPickerModelTests.cs`:

```csharp
using Plith.Services;

namespace Plith.Tests;

public class OutputPickerModelTests
{
    private static WindowsAudioEndpointInfo Ep(string id, string name) => new(id, name);

    private static readonly IReadOnlyList<WindowsAudioEndpointInfo> Five =
    [
        Ep("steam-speakers", "Hoparlör (Steam Streaming Speakers)"),
        Ep("realtek", "Hoparlör (Realtek(R) Audio)"),
        Ep("steam-mic", "Hoparlör (Steam Streaming Microphone)"),
        Ep("nvidia", "PG27AQDM (NVIDIA High)"),
        Ep("g733", "Hoparlör (Logitech G733)"),
    ];

    [Fact]
    public void TheCurrentOutputComesFirst()
    {
        // The one you are on is the anchor: it is what tells you the list is about the thing you
        // are already hearing.
        var cells = OutputPickerModel.Cells(Five, "g733", capacity: 6);

        Assert.Equal("g733", cells[0].Id);
        Assert.True(cells[0].IsCurrent);
    }

    [Fact]
    public void TheRestKeepEnumerationOrder()
    {
        var cells = OutputPickerModel.Cells(Five, "g733", capacity: 6);

        Assert.Equal(["g733", "steam-speakers", "realtek", "steam-mic", "nvidia"],
                     cells.Select(c => c.Id));
    }

    [Fact]
    public void NothingIsMarkedCurrentWhenTheDefaultIsNotInTheList()
    {
        // Reachable: the default id and the enumeration are two reads, and a device can be
        // unplugged between them.
        var cells = OutputPickerModel.Cells(Five, "a-device-that-left", capacity: 6);

        Assert.Equal(5, cells.Count);
        Assert.DoesNotContain(cells, c => c.IsCurrent);
        Assert.Equal("steam-speakers", cells[0].Id);
    }

    [Fact]
    public void ExactlyCapacityDrawsEveryDeviceAndNoOverflow()
    {
        var six = Five.Append(Ep("sixth", "Speakers (Sixth)")).ToList();

        var cells = OutputPickerModel.Cells(six, "g733", capacity: 6);

        Assert.Equal(6, cells.Count);
        Assert.DoesNotContain(cells, c => c.IsOverflow);
    }

    [Fact]
    public void MoreThanCapacityKeepsTheLastCellForTheDoor()
    {
        // Nothing is hidden behind a count, which is the shelf's most expensive lesson: a folded
        // tile is in no UIA tree at all. So the overflow is a NAMED cell that opens Windows'
        // own sound settings, and five devices are drawn rather than six.
        var seven = Five
            .Append(Ep("sixth", "Speakers (Sixth)"))
            .Append(Ep("seventh", "Speakers (Seventh)"))
            .ToList();

        var cells = OutputPickerModel.Cells(seven, "g733", capacity: 6);

        Assert.Equal(6, cells.Count);
        Assert.True(cells[^1].IsOverflow);
        Assert.Equal(5, cells.Count(c => !c.IsOverflow));
    }

    [Fact]
    public void TheOverflowCellIsNotADeviceAndCarriesNoId()
    {
        var seven = Five
            .Append(Ep("sixth", "Speakers (Sixth)"))
            .Append(Ep("seventh", "Speakers (Seventh)"))
            .ToList();

        var overflow = OutputPickerModel.Cells(seven, "g733", capacity: 6)[^1];

        Assert.Equal(string.Empty, overflow.Id);
        Assert.False(overflow.IsCurrent);
        Assert.NotEmpty(overflow.Label);
    }

    [Fact]
    public void NoEndpointsIsAnEmptyList()
    {
        // Possible: a machine with every output disabled. The page says so rather than drawing
        // an empty grid.
        Assert.Empty(OutputPickerModel.Cells([], "g733", capacity: 6));
    }
}
```

Append to `tests/Plith.Tests/NotchGeometryTests.cs`:

```csharp
    [Fact]
    public void OutputPickerCapacityIsTheProductOfItsGrid()
    {
        // Defined as the product for the same reason ShelfCapacity is: a literal beside a grid is
        // free to stop matching it, and the number that drifts is the one that decides whether a
        // device is drawn at all.
        Assert.Equal(NotchGeometry.OutputPickerColumns * NotchGeometry.OutputPickerRows,
                     NotchGeometry.OutputPickerCapacity);
    }
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests --filter "OutputPickerModelTests|NotchGeometryTests"`
Expected: FAIL to compile.

- [x] **Step 3: Write the geometry**

In `src/Plith/Views/Presentation/NotchGeometry.cs`, beside the shelf's grid constants:

```csharp
    /// <summary>Columns in the output picker's grid.</summary>
    public const int OutputPickerColumns = 2;

    /// <summary>Rows in the output picker's grid.</summary>
    public const int OutputPickerRows = 3;

    /// <summary>
    /// How many cells the picker draws, which is also how many outputs it can show.
    ///
    /// The product of the grid rather than a literal, exactly like <see cref="ShelfCapacity"/>:
    /// the number that decides whether a device appears at all must not be free to drift from
    /// the grid that draws it. With more endpoints than this, the last cell becomes a door to
    /// Windows' own sound settings rather than a silent fold, because a folded cell is in no UIA
    /// tree and so is invisible to a screen reader and reachable by no key.
    /// </summary>
    public const int OutputPickerCapacity = OutputPickerColumns * OutputPickerRows;
```

- [x] **Step 4: Write the model**

Create `src/Plith/Services/OutputPickerModel.cs`:

```csharp
namespace Plith.Services;

/// <summary>One cell of the output picker: a device, or the door to Windows' sound settings.</summary>
public sealed record OutputChoice(string Id, string Label, bool IsCurrent, bool IsOverflow = false);

/// <summary>
/// What the output picker's grid holds.
///
/// Pure, and a class of its own for the reason NotchOpeningPolicy is: the page that draws this
/// lives in a BandWindow the test project cannot construct, so a rule written inside it has no
/// test at all.
/// </summary>
public static class OutputPickerModel
{
    /// <summary>What a cell says when there are more devices than cells.</summary>
    public const string OverflowLabel = "More in Windows settings";

    /// <summary>
    /// The cells to draw, in order.
    ///
    /// The current output comes first because it is the anchor: it says the list is about the
    /// thing you are already hearing. The rest keep enumeration order, which is the order
    /// Windows itself lists them in.
    ///
    /// With more endpoints than cells, the LAST cell is the overflow door and only
    /// <c>capacity - 1</c> devices are drawn. Nothing is folded behind a count: a folded cell is
    /// in no UIA tree, invisible to a screen reader and reachable by no key, which is the
    /// shelf's own most expensive lesson.
    /// </summary>
    public static IReadOnlyList<OutputChoice> Cells(
        IReadOnlyList<WindowsAudioEndpointInfo> endpoints, string currentId, int capacity)
    {
        ArgumentNullException.ThrowIfNull(endpoints);
        ArgumentOutOfRangeException.ThrowIfLessThan(capacity, 2);

        if (endpoints.Count == 0) return [];

        // The current one first, if it is still here at all: the default id and the enumeration
        // are two separate reads and a device can leave between them.
        var ordered = endpoints
            .Where(e => string.Equals(e.Id, currentId, StringComparison.Ordinal))
            .Concat(endpoints.Where(e => !string.Equals(e.Id, currentId, StringComparison.Ordinal)))
            .ToList();

        var overflowing = ordered.Count > capacity;
        var room = overflowing ? capacity - 1 : capacity;

        var cells = ordered
            .Take(room)
            .Select(e => new OutputChoice(e.Id, e.FriendlyName,
                                          string.Equals(e.Id, currentId, StringComparison.Ordinal)))
            .ToList();

        if (overflowing)
            cells.Add(new OutputChoice(string.Empty, OverflowLabel, IsCurrent: false, IsOverflow: true));

        return cells;
    }
}
```

- [x] **Step 5: Run the tests to verify they pass**

Run: `dotnet test`
Expected: 551 passing (543 plus 8).

- [x] **Step 6: Commit**

```bash
git add src/Plith/Services/OutputPickerModel.cs src/Plith/Views/Presentation/NotchGeometry.cs \
        tests/Plith.Tests/OutputPickerModelTests.cs tests/Plith.Tests/NotchGeometryTests.cs
git commit -m "feat(notch): model the output picker's cells

The current output first, the rest in enumeration order, and with more
devices than cells the last one becomes a named door to Windows' sound
settings rather than a silent fold. A folded cell is in no UIA tree, which
is the shelf's own most expensive lesson.

Capacity is the product of the grid, like ShelfCapacity: the number that
decides whether a device is drawn at all must not drift from the grid
that draws it."
```

---

### Task 4: The picker on the page

**Files:**
- Modify: `src/Plith/Views/Widgets/MediaWidget.xaml`
- Modify: `src/Plith/Views/Widgets/MediaWidget.cs`
- Modify: `scripts/render-widgets.ps1`

**Interfaces:**
- Consumes: `OutputPickerModel.Cells`, `OutputChoice`, `NotchGeometry.OutputPicker*`,
  `OutputDeviceSwitcher.TrySetDefault`, `WindowsAudioClient.EnumerateRenderEndpoints`,
  `SystemSoundPanel.TryOpen`.
- Produces: nothing further.

No unit test is possible: the suite is not STA. The test cycle is the render harness in both
themes plus both lints, which is where every layout defect on this branch has actually been
found.

- [x] **Step 1: Add the picker's markup**

In `MediaWidget.xaml`, inside `Root`, add a second child that spans all three rows and is
collapsed by default. The media content (the tile row and the progress row) and this are the two
modes; exactly one is visible.

```xml
        <!-- The picker mode. Spans the whole band, because it replaces both media rows rather
             than sitting beside them: the page becomes the list.

             A Grid of Buttons rather than a ListBox: every cell needs its own automation peer
             and its own accessible name carrying the FULL device name, and a ListBox would give
             the container the peer and the item template the name. The shelf's tiles are laid
             out the same way for the same reason. -->
        <Grid x:Name="PickerMode" Grid.Row="0" Grid.RowSpan="3" Visibility="Collapsed">
            <Grid.RowDefinitions>
                <RowDefinition Height="14" />
                <RowDefinition Height="3" />
                <RowDefinition Height="*" />
            </Grid.RowDefinitions>

            <!-- 14 + 3 + 56 = 73, the content band. The 56 is three 16 DIP rows and two 4 DIP
                 gaps, built in code from NotchGeometry's grid constants. -->
            <Button x:Name="PickerBack" Grid.Row="0" HorizontalAlignment="Left"
                    Style="{StaticResource PickerBackStyle}"
                    AutomationProperties.Name="Back to now playing">
                <StackPanel Orientation="Horizontal">
                    <Viewbox Width="10" Height="10" VerticalAlignment="Center">
                        <Canvas Width="24" Height="24">
                            <Path Data="{StaticResource IconChevronLeft}"
                                  Stroke="{DynamicResource NotchInkMuted}" StrokeThickness="2"
                                  StrokeStartLineCap="Round" StrokeEndLineCap="Round"
                                  StrokeLineJoin="Round" />
                        </Canvas>
                    </Viewbox>
                    <TextBlock x:Name="PickerHeader" Margin="6,0,0,0"
                               FontSize="10.5" VerticalAlignment="Center"
                               Foreground="{DynamicResource NotchInkMuted}" />
                </StackPanel>
            </Button>

            <UniformGrid x:Name="PickerCells" Grid.Row="2"
                         Columns="2" Rows="3" />
        </Grid>
```

`IconChevronLeft` does not exist yet. Check `Resources/PlithIcons.xaml` first: if there is no
left-pointing chevron, add one there as drawn geometry (a two-segment polyline, 24x24 viewbox, to
match the other icons' stroke weight). A Segoe MDL2 code point fails `check-a11y.ps1`.

Add `PickerBackStyle` beside the other styles: a transparent-background `Button` whose template
is a `Border` with `CornerRadius="7"`, `Background="Transparent"` (not null: WPF hit-tests a
Transparent brush and not a null one, which is the defect `ShelfWindow.xaml` records in its own
comment), and `#1FFFFFFF` on hover.

And a `PickerCellStyle` for the cells: `Height="16"`, `Margin="0,0,0,4"` on the first two rows,
`CornerRadius="5"`, `Background="Transparent"`, hover `#14FFFFFF`, keyboard focus
`{DynamicResource OsdHighlight}`, and a `ContentPresenter` centred vertically with 8 DIP of left
padding. Cell content is built in code.

- [x] **Step 2: Build the cells in code**

In `MediaWidget.cs`:

```csharp
    /// <summary>
    /// Show the outputs, in place of what is playing.
    ///
    /// The list is read once, when the picker opens, and not refreshed while it is up. A device
    /// can sleep or be unplugged mid-gesture, and a grid that rearranged itself under a pointer
    /// already moving toward a cell would route audio somewhere nobody chose. A press on a device
    /// that has since gone returns false and lands in the header instead.
    /// </summary>
    private void OpenPicker()
    {
        var endpoints = WindowsAudioClient.EnumerateRenderEndpoints();
        var current = WindowsAudioClient.TryGetDefaultRenderEndpointId() ?? string.Empty;
        var cells = OutputPickerModel.Cells(endpoints, current,
                                            Presentation.NotchGeometry.OutputPickerCapacity);

        PickerHeader.Text = cells.Count == 0 ? "No outputs available" : "Output";
        PickerCells.Columns = Presentation.NotchGeometry.OutputPickerColumns;
        PickerCells.Rows = Presentation.NotchGeometry.OutputPickerRows;
        PickerCells.Children.Clear();
        foreach (var cell in cells) PickerCells.Children.Add(BuildCell(cell));

        MediaMode.Visibility = Visibility.Collapsed;
        PickerMode.Visibility = Visibility.Visible;
        PickerBack.Focus();
    }

    private void ClosePicker()
    {
        PickerMode.Visibility = Visibility.Collapsed;
        MediaMode.Visibility = Visibility.Visible;
    }
```

`MediaMode` is a new name for the existing media content: wrap the tile row and the progress row
in one `Grid x:Name="MediaMode"` with the same three `RowDefinition`s, so the two modes are
siblings and each can be collapsed whole.

`WindowsAudioClient.TryGetDefaultRenderEndpointId()` does not exist. Add it beside
`EnumerateRenderEndpoints`, in the same shape (a static that swallows a COM failure and returns
null), since the picker needs the default's id and nothing else exposes it.

The cell itself:

```csharp
    private Button BuildCell(OutputChoice choice)
    {
        var button = new Button
        {
            Style = (Style)FindResource("PickerCellStyle"),
            Tag = choice,
            // The FULL name, not the label the cell draws. The label is trimmed to 155 DIP and a
            // screen reader must not inherit that trimming, which is the whole reason this design
            // refuses a "distinguishing token" heuristic: the truth lives here.
            Content = new TextBlock
            {
                Text = choice.Label,
                FontSize = 11,
                TextTrimming = TextTrimming.CharacterEllipsis,
                VerticalAlignment = VerticalAlignment.Center,
                Foreground = (Brush)FindResource(choice.IsCurrent ? "NotchInk" : "NotchInkMuted"),
            },
            ToolTip = choice.Label,
        };

        AutomationProperties.SetName(button,
            choice.IsOverflow ? choice.Label
                              : choice.IsCurrent ? $"{choice.Label}, current output"
                                                 : choice.Label);

        button.Click += (_, _) => Choose(choice);
        return button;
    }

    private void Choose(OutputChoice choice)
    {
        if (choice.IsOverflow)
        {
            SystemSoundPanel.TryOpen();
            ClosePicker();
            return;
        }

        if (OutputDeviceSwitcher.TrySetDefault(choice.Id))
        {
            // Back to what is playing. The level the notch shows follows on its own:
            // WindowsAudioClient implements IMMNotificationClient and re-attaches on a default
            // change, a path hardened in 9de0664 so it can no longer fail silently.
            ClosePicker();
            return;
        }

        // Stays open and says so. Silently doing nothing is what every other control on this
        // page is written not to do.
        PickerHeader.Text = "Could not switch output";
    }
```

Wire the entry, the exit and the key:

```csharp
        Output.Click += (_, _) => OpenPicker();
        PickerBack.Click += (_, _) => ClosePicker();
        PreviewKeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape && PickerMode.Visibility == Visibility.Visible)
            {
                ClosePicker();
                e.Handled = true;
            }
        };
```

The dot marking the current output: draw it as a 6 DIP `Ellipse` in the cell's content, ahead of
the label, filled with `NotchInk` and collapsed when `IsCurrent` is false. Put the `Ellipse` and
the `TextBlock` in a `StackPanel` rather than reserving space with a margin, so a non-current
cell's label starts where the dot would have been and the column does not look ragged.

**A track change must not close the picker.** `Render` already runs on every view-model change;
it must not touch either mode's visibility. Check that when wiring: the only writers of
`PickerMode.Visibility` are `OpenPicker`, `ClosePicker` and the initial XAML.

- [x] **Step 3: Give the harness both fixtures**

In `scripts/render-widgets.ps1`, after the existing media renders, add two more. The picker needs
no SMTC session, but it does read the real endpoint list, so the fixture has to come in from
outside rather than from the machine. Add an internal seam for it: an optional
`IReadOnlyList<WindowsAudioEndpointInfo>` and `string` on `MediaWidget`'s constructor, defaulting
to null, and `OpenPicker` uses them when they are set.

```powershell
# The picker, with this machine's own five-device shape and with a seven-device one so the
# overflow door is in shot. Injected rather than read from the machine: a harness that renders
# whatever is plugged in today renders something different tomorrow.
$five = [System.Collections.Generic.List[Plith.Services.WindowsAudioEndpointInfo]]::new()
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('g733', 'Hoparlör (Logitech G733 Gaming Headset)'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('realtek', 'Hoparlör (Realtek(R) Audio)'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('nvidia', 'PG27AQDM (NVIDIA High)'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('steam-spk', 'Hoparlör (Steam Streaming Speakers)'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('steam-mic', 'Hoparlör (Steam Streaming Microphone)'))

$pickerFive = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null, $five, 'g733')
$pickerFive.ShowPickerForRender()
Save-Visual -Element $pickerFive -W $frameW -H $frameH -Name 'widget-media-picker'

$seven = [System.Collections.Generic.List[Plith.Services.WindowsAudioEndpointInfo]]::new($five)
$seven.Add([Plith.Services.WindowsAudioEndpointInfo]::new('sixth', 'Speakers (Sixth Device)'))
$seven.Add([Plith.Services.WindowsAudioEndpointInfo]::new('seventh', 'Speakers (Seventh Device)'))
$pickerSeven = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null, $seven, 'g733')
$pickerSeven.ShowPickerForRender()
Save-Visual -Element $pickerSeven -W $frameW -H $frameH -Name 'widget-media-picker-overflow'
```

`ShowPickerForRender()` is a public method that calls `OpenPicker()`. It exists because the
harness cannot click a button, and it is named for what it is rather than hidden behind an
`internal` the harness would have to reach around.

- [x] **Step 4: Build, render both themes, and look**

Run:
```
dotnet build
pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark
pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -OutDir "$env:TEMP\plith-render-light"
```

**Then open the four PNGs and check:** the 16 DIP rows are legible rather than cramped (spec risk
1, and the number most likely to be wrong), the current device's dot and brighter ink read as
"this is the one you are on", the long names trim rather than overflowing their cell, the
overflow cell reads as a door and not as a device, and NOTHING is a cold blue rectangle on the
tinted panel, which is the defect that killed the System Controls page.

- [x] **Step 5: Run the lints**

Run:
```
pwsh -File scripts/check-a11y.ps1
pwsh -File scripts/check-contrast.ps1
```
Expected: both pass. The cells' `Foreground` and `Background` come from the palette, so the
contrast script will measure them; a failure is a real finding and the ratio belongs in the
report before any colour changes.

- [x] **Step 6: Commit**

```bash
git add src/Plith/Views/Widgets/MediaWidget.xaml src/Plith/Views/Widgets/MediaWidget.cs \
        src/Plith/Services/WindowsAudioClient.cs scripts/render-widgets.ps1
git commit -m "feat(notch): let the media page become the output list

The output control turns the page into a grid of outputs rather than
opening a popup: WPF opens a Popup in its own window, outside the notch's
layered surface, where it would draw as a square rectangle in the wrong
theme. Pressing a cell writes the default and returns to what is playing;
a failed write keeps the picker open and says so.

Every cell is a Button, so it has an automation peer, and its accessible
name carries the FULL device name rather than the trimmed label the cell
draws."
```

---

### Task 5: Press it on hardware, and put the device back

**Files:**
- Modify: `scripts/drive-media-page.ps1`
- Modify: `docs/PHASE6-VERIFICATION.md`
- Modify: `docs/ROADMAP.md`
- Modify: `CLAUDE.md`
- Modify: `docs/superpowers/plans/2026-09-21-notch-output-picker.md` (tick the boxes)

- [ ] **Step 1: Add the stage**

In `scripts/drive-media-page.ps1`, after the existing media-page verdicts, add a stage that opens
the picker and presses the second cell. It must record the default before, and put it back after,
whatever the verdict:

```powershell
# --- the output picker -------------------------------------------------------------------------
#
# This stage CHANGES A SYSTEM SETTING, so it restores it in a finally. A check that leaves
# someone's audio coming out of a different device is a rude check, and one that leaves it there
# only when it fails is worse: the failure is exactly when nobody is watching the cleanup.
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'NAudio.Wasapi.dll'))
$en = New-Object NAudio.CoreAudioApi.MMDeviceEnumerator
$originalId = $en.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
try {
    $outputBtn = Get-Element -Hwnd $notch.Hwnd -Name 'Change output device' -Type 'Button'
    if (-not $outputBtn) {
        Add-Verdict 'the output control is in the tree' $false 'not found by name'
    } else {
        Move-Pointer -X $outputBtn.CX -Y $outputBtn.CY -Settle 250
        [MediaInput]::LeftClick()
        Start-Sleep -Milliseconds 600

        $names = Get-Names -Hwnd $notch.Hwnd
        $onPicker = $names -contains 'Back to now playing'
        Add-Verdict 'the output control opens the picker' $onPicker `
            "names in the tree: $($names -join ' | ')"

        # A device that is NOT the current one, found by its own accessible name: the current one
        # carries ", current output" and pressing it would prove nothing.
        $target = $names | Where-Object {
            $_ -notmatch 'current output' -and $_ -notmatch 'Back to now playing' -and
            $_ -ne 'Output' -and $_ -notmatch 'More in Windows settings'
        } | Select-Object -First 1

        if (-not $target) {
            Add-Verdict 'a second output is offered' $false "names: $($names -join ' | ')"
        } else {
            $cell = Get-Element -Hwnd $notch.Hwnd -Name $target -Type 'Button'
            Move-Pointer -X $cell.CX -Y $cell.CY -Settle 250
            [MediaInput]::LeftClick()
            Start-Sleep -Milliseconds 900

            $nowId = $en.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
            Add-Verdict 'pressing a cell changes the system default output' ($nowId -ne $originalId) `
                "target '$target', default was $originalId, now $nowId"

            $back = Get-Names -Hwnd $notch.Hwnd
            Add-Verdict 'the page returns to now playing after a switch' `
                ($back -contains 'Now playing') "names after: $($back -join ' | ')"
        }
    }
}
finally {
    $restored = [Plith.Services.OutputDeviceSwitcher]::TrySetDefault($originalId, $null)
    $endId = $en.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
    "output restored: $restored (default is now $endId, was $originalId at the start)"
    if ($endId -ne $originalId) {
        Write-Host "WARNING: the default output was NOT restored. Set it back by hand." -ForegroundColor Red
    }
}
```

- [ ] **Step 2: Run it**

Run: `pwsh -File scripts/drive-media-page.ps1`

Expected: every verdict PASS, and the closing line reporting the output restored to the id it
started at. The preconditions still apply: an Active session, no game holding the pointer, and
the notch presentation. If `Assert-InputWorks` throws because something owns the pointer, that is
the instrument doing its job and not a product failure.

- [ ] **Step 3: Record it**

Add a section 21 to `docs/PHASE6-VERIFICATION.md`, in the shape of section 20: the date, the
probe's `TrySetDefault` result, every verdict with its evidence string, the endpoint list as
measured, and whether the output was restored. Anything the renders showed that this plan did not
predict goes here too, especially if the 16 DIP row had to change.

Update `docs/ROADMAP.md`'s Phase 6 entry: the output picker exists, and the `IPolicyConfig` note
should now say the interop is committed rather than measured-and-discarded.

Update `CLAUDE.md`'s Status section, reading its banner about the split section first.

- [ ] **Step 4: Tick this plan's boxes and commit**

Boxes are ticked as work proceeds, not at the end: a plan whose boxes are all empty reads as
"never started", which is how 199 boxes came to sit unticked across three finished phases here.

```bash
git add scripts/drive-media-page.ps1 docs/PHASE6-VERIFICATION.md docs/ROADMAP.md CLAUDE.md \
        docs/superpowers/plans/2026-09-21-notch-output-picker.md
git commit -m "docs(notch): record the output picker's hardware run

The driver opens the picker, presses a device that is not the current one,
confirms through NAudio that the system default actually moved, and puts
it back in a finally: a check that leaves someone's audio on a different
device is a rude check, and one that only leaks on failure leaks exactly
when nobody is watching."
```

---

## Self-Review

**Spec coverage.** Every section of the spec maps to a task:

| Spec section | Task |
|---|---|
| Naming, and the collision measured in Settings | 1 |
| Switching the default, Console + Multimedia, failure handling | 2 (interop), 4 (the message) |
| Capacity, the overflow door, ordering | 3 |
| Layout, the mode, accessibility | 4 |
| Testing: unit | 1, 3 |
| Testing: render, lints, hardware | 4, 5 |
| Risk 1 (16 DIP rows) | 4 step 4, named as the thing to judge |
| Risk 2 (undocumented interface) | 2 step 3, the probe, plus the false path in 4 |
| Risk 3 (the list changing while open) | 4 step 2, read once on open |

**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Three steps
deliberately require reading a file before editing rather than quoting it whole, and each says
which file and what to preserve: Task 1 step 5 (`EnumerateRenderEndpoints`'s existing try
structure), Task 2 step 1 (`DiagnosticLog`'s and `ExceptionText`'s real method names), Task 4
step 1 (whether a left chevron icon already exists). Quoting those whole would have put a stale
copy in this plan.

**Type consistency.** `WindowsAudioEndpointInfo(Id, FriendlyName)` is used in that order in Tasks
1, 3 and 4. `OutputChoice(Id, Label, IsCurrent, IsOverflow)` is constructed in Task 3 and read by
those names in Task 4. `OutputPickerModel.Cells(endpoints, currentId, capacity)` matches between
its tests and its caller. `OutputDeviceSwitcher.TrySetDefault(string, DiagnosticLog?)` is called
with one argument in Task 4 (the log defaults) and with two in Task 5's PowerShell, where `$null`
is passed explicitly because PowerShell does not apply C# default parameters.

**One new API this plan adds without a task of its own:**
`WindowsAudioClient.TryGetDefaultRenderEndpointId()`, in Task 4 step 2. It is named there with
its shape and its failure behaviour, and it sits beside `EnumerateRenderEndpoints` in the same
file, so it does not need its own task; but it IS new surface, and a reviewer should see it
declared rather than discover it.
