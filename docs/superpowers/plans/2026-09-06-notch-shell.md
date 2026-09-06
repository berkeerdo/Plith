# Notch Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `OsdHost` a presentation-mode seam so it can either fade in place (Classic OSD, unchanged) or park a thin strip at the top of the screen that slides down on an event (Ambient Notch).

**Architecture:** One host, two presentations. `OsdHost` keeps owning the band window, UIAccess z-band, positioning, hide timer, hover keep-alive and suppression wiring; an `IOsdPresentation` owns only resting state, transition and hit-testing. All decisions that can be made without a window live in two pure static classes (`NotchGeometry`, `PresentationPolicy`) so they are unit-testable on the existing headless suite; the presentation classes are thin WPF adapters over them.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`, x64), xunit 2.9.3, INI persistence via `SettingsService`.

**Spec:** `docs/superpowers/specs/2026-09-06-notch-shell-design.md`

## Global Constraints

- **All code, comments and commit messages in English.** Conventional Commits format.
- **No AI attribution anywhere** — not in commits, code, docs or config.
- Target framework `net10.0-windows10.0.22000.0`, `Platforms=x64`, `Nullable=enable`, `ImplicitUsings=enable`.
- INI persistence is locale-agnostic: every numeric/bool/enum conversion uses `CultureInfo.InvariantCulture` and `"G"` formatting.
- `tests/Plith.Tests` is headless and non-STA. Nothing in it may construct a `BandWindow`, `Window`, or any WPF visual requiring an `HwndSource`.
- `scripts/check-a11y.ps1` must pass (`pwsh -File scripts/check-a11y.ps1`, exit 0). Every interactive XAML control needs `AutomationProperties.Name` or `LabeledBy`; `AutomationProperties` must never sit on a peerless element (`Grid`, `Border`, `StackPanel`, shapes, …).
- `OsdContent` has a fixed `Width="440"` and its outer `Grid` carries `Margin="14"` to reserve drop-shadow space. That 14 DIP inset sits between the window origin and the card's visible border and must be threaded through every geometry calculation — call it `contentInset`.
- Default presentation mode is `ClassicOsd`, including for config files written before this change.
- Ambient strip height defaults inside the 4–6 px band `docs/ROADMAP.md` §3 specifies.

## File Structure

| File | Responsibility |
|---|---|
| `src/Plith/Views/Presentation/NotchGeometry.cs` (new) | Pure geometry: resting/descended content offsets, strip rectangle, physical→DIP conversion, hit test. No WPF window types. |
| `src/Plith/Views/Presentation/PresentationPolicy.cs` (new) | Pure decisions: edge margin per mode, `IsAtRest`, `WantsHitTesting`. |
| `src/Plith/Views/Presentation/IOsdPresentation.cs` (new) | The seam. Three behaviours, nothing else. |
| `src/Plith/Views/Presentation/ClassicPresentation.cs` (new) | Today's opacity fade, moved behind the seam verbatim. |
| `src/Plith/Views/Presentation/AmbientNotchPresentation.cs` (new) | Parked strip, `TranslateTransform` descent, click-through toggle. |
| `src/Plith/Views/Presentation/NotchHoverPoller.cs` (new) | `GetCursorPos` polling and strip enter/leave events. |
| `src/Plith/Views/OsdHost.cs` (modify) | Delegates the three behaviours; keeps everything else. |
| `src/Plith/Views/OsdContent.xaml` (modify) | Gains the notch strip visual and a `TranslateTransform` on its root. |
| `src/Plith/Services/SettingsModel.cs` (modify) | `PresentationMode` enum, `Presentation` and `NotchStripHeightDip` properties, `Clone`. |
| `src/Plith/Services/SettingsService.cs` (modify) | Load/save the two new keys. |
| `src/Plith/Services/FullscreenVideoWatcher.cs` (modify) | Publish `ForegroundCoversMonitorChanged` alongside suppression. |
| `src/Plith/Views/SettingsWindow.xaml{,.cs}` (modify) | Mode picker, strip height, disable Edit Position in notch mode. |
| `tests/Plith.Tests/NotchGeometryTests.cs` (new) | |
| `tests/Plith.Tests/PresentationPolicyTests.cs` (new) | |
| `tests/Plith.Tests/SettingsServiceTests.cs` (modify) | Defaulting and round-trip for the new keys. |

---

### Task 1: `NotchGeometry` — pure geometry

**Files:**
- Create: `src/Plith/Views/Presentation/NotchGeometry.cs`
- Test: `tests/Plith.Tests/NotchGeometryTests.cs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `static double RestingOffset(double contentHeight, double stripHeight, double contentInset)`
  - `const double DescendedOffset = 0.0`
  - `static Rect StripRect(double windowLeft, double windowTop, double contentWidth, double stripHeight, double contentInset)`
  - `static Point PhysicalToDip(int physicalX, int physicalY, double dpiScale)`
  - `static bool IsInsideStrip(Rect stripRect, Point cursorDip)`

`Rect` and `Point` are `System.Windows.Rect` / `System.Windows.Point` — structs, no window required.

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/NotchGeometryTests.cs`:

```csharp
using System.Windows;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchGeometryTests
{
    // OsdContent's outer Grid has Margin="14" to reserve drop-shadow space, so the card's
    // visible border starts 14 DIP below the content origin. Every case here uses that
    // real value rather than 0, because a geometry that only works at inset 0 would look
    // correct in tests and sit 14 px too low on screen.
    private const double Inset = 14;

    [Fact]
    public void RestingOffset_LeavesExactlyTheStripVisible()
    {
        // 200 tall content, 5 px strip: push up until only inset + strip remains on screen.
        Assert.Equal(-181, NotchGeometry.RestingOffset(contentHeight: 200, stripHeight: 5, contentInset: Inset));
    }

    [Fact]
    public void RestingOffset_ScalesWithContentHeight()
    {
        var shorter = NotchGeometry.RestingOffset(150, 5, Inset);
        var taller = NotchGeometry.RestingOffset(300, 5, Inset);
        Assert.Equal(-131, shorter);
        Assert.Equal(-281, taller);
    }

    [Fact]
    public void RestingOffset_NeverPushesDown_WhenContentIsShorterThanTheStrip()
    {
        // Degenerate but reachable during the first layout pass, when DesiredSize is still
        // zero. A positive offset there would drop the card into the middle of the screen
        // for one frame.
        Assert.Equal(0, NotchGeometry.RestingOffset(contentHeight: 0, stripHeight: 5, contentInset: Inset));
    }

    [Fact]
    public void DescendedOffset_IsZero()
    {
        Assert.Equal(0.0, NotchGeometry.DescendedOffset);
    }

    [Fact]
    public void StripRect_SitsAtTheWindowTopAndInsideTheShadowInset()
    {
        var r = NotchGeometry.StripRect(
            windowLeft: 740, windowTop: 0, contentWidth: 440, stripHeight: 5, contentInset: Inset);

        Assert.Equal(754, r.Left);    // 740 + 14
        Assert.Equal(0, r.Top);
        Assert.Equal(412, r.Width);   // 440 - 14 * 2
        Assert.Equal(5, r.Height);
    }

    [Fact]
    public void StripRect_FollowsTheWindowOrigin()
    {
        // The window origin comes from Reposition(), which anchors on Screen.WorkingArea.
        // A taskbar docked to the top therefore moves the strip down with it, and this is
        // the only thing NotchGeometry needs to know about that.
        var r = NotchGeometry.StripRect(740, 48, 440, 5, Inset);
        Assert.Equal(48, r.Top);
    }

    [Fact]
    public void StripRect_DoesNotGoNegative_ForContentNarrowerThanTheInset()
    {
        var r = NotchGeometry.StripRect(0, 0, contentWidth: 10, stripHeight: 5, contentInset: Inset);
        Assert.Equal(0, r.Width);
    }

    [Fact]
    public void PhysicalToDip_At100Percent_IsIdentity()
    {
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(800, 12, 1.0));
    }

    [Fact]
    public void PhysicalToDip_At125Percent_DividesOut()
    {
        // GetCursorPos reports physical pixels; Left/Top and WorkingArea are DIP. This is
        // the conversion the spec requires to be explicit and covered at a non-100 % scale.
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(1000, 15, 1.25));
    }

    [Fact]
    public void PhysicalToDip_At150Percent_DividesOut()
    {
        Assert.Equal(new Point(640, 8), NotchGeometry.PhysicalToDip(960, 12, 1.5));
    }

    [Fact]
    public void PhysicalToDip_TreatsANonPositiveScaleAsOneToOne()
    {
        // A failed DPI query must not divide by zero and teleport the cursor to infinity.
        Assert.Equal(new Point(800, 12), NotchGeometry.PhysicalToDip(800, 12, 0));
    }

    [Fact]
    public void IsInsideStrip_JustInside()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.True(NotchGeometry.IsInsideStrip(r, new Point(755, 2)));
    }

    [Fact]
    public void IsInsideStrip_JustBelow()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.False(NotchGeometry.IsInsideStrip(r, new Point(755, 6)));
    }

    [Fact]
    public void IsInsideStrip_JustLeftOfIt()
    {
        var r = NotchGeometry.StripRect(740, 0, 440, 5, Inset);
        Assert.False(NotchGeometry.IsInsideStrip(r, new Point(753, 2)));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter "FullyQualifiedName~NotchGeometryTests"`
Expected: build failure — `NotchGeometry` does not exist.

- [ ] **Step 3: Write the implementation**

Create `src/Plith/Views/Presentation/NotchGeometry.cs`:

```csharp
using System.Windows;

namespace Plith.Views.Presentation;

/// <summary>
/// Geometry for the Ambient Notch, kept free of every WPF window type so it can be tested
/// on the headless suite. This mirrors the FullscreenVideoDetector / FullscreenVideoWatcher
/// split: the watcher gathers, the detector decides, and only the decision is testable.
///
/// Every length here is in device-independent units. The one place physical pixels enter
/// is <see cref="PhysicalToDip"/>, which exists so that conversion happens once, at a named
/// boundary, instead of being spread across call sites — mixing the two spaces silently
/// breaks every comparison on a non-100 % display.
/// </summary>
public static class NotchGeometry
{
    /// <summary>Content offset when the notch is fully descended.</summary>
    public const double DescendedOffset = 0.0;

    /// <summary>
    /// Y offset applied to the OSD content while the notch is parked: negative, pushing all
    /// but the strip above the top of the window.
    ///
    /// <paramref name="contentInset"/> is the drop-shadow margin on OsdContent's outer Grid
    /// (14 DIP today). The card's visible border starts that far below the content origin,
    /// so it has to be subtracted or the strip renders that much too short.
    ///
    /// Clamped at zero: during the first layout pass DesiredSize is still zero, and a
    /// positive offset there would drop the card into mid-screen for a frame.
    /// </summary>
    public static double RestingOffset(double contentHeight, double stripHeight, double contentInset)
        => -Math.Max(0, contentHeight - stripHeight - contentInset);

    /// <summary>
    /// Screen rectangle of the visible strip, in DIP. <paramref name="windowLeft"/> and
    /// <paramref name="windowTop"/> are the values Reposition() computed, which are derived
    /// from Screen.WorkingArea — so a taskbar docked to the top moves this rectangle down
    /// with it and needs no special case here.
    /// </summary>
    public static Rect StripRect(
        double windowLeft, double windowTop, double contentWidth, double stripHeight, double contentInset)
    {
        var left = windowLeft + contentInset;
        var width = Math.Max(0, contentWidth - contentInset * 2);
        return new Rect(left, windowTop, width, Math.Max(0, stripHeight));
    }

    /// <summary>
    /// Convert a GetCursorPos result (physical pixels) into DIP.
    ///
    /// A non-positive scale means the DPI query failed; treat it as 1:1 rather than
    /// dividing by zero, which would send the cursor to infinity and make the strip
    /// permanently unhoverable.
    /// </summary>
    public static Point PhysicalToDip(int physicalX, int physicalY, double dpiScale)
    {
        if (dpiScale <= 0) return new Point(physicalX, physicalY);
        return new Point(physicalX / dpiScale, physicalY / dpiScale);
    }

    /// <summary>Hit test, both operands in DIP. Rect.Contains is inclusive on left/top and
    /// exclusive on right/bottom, which is the behaviour we want at the screen edge.</summary>
    public static bool IsInsideStrip(Rect stripRect, Point cursorDip) => stripRect.Contains(cursorDip);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter "FullyQualifiedName~NotchGeometryTests"`
Expected: PASS, 13 tests.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/Presentation/NotchGeometry.cs tests/Plith.Tests/NotchGeometryTests.cs
git commit -m "feat(notch): add pure notch geometry with an explicit DPI boundary"
```

---

### Task 2: `PresentationMode` setting and `PresentationPolicy`

**Files:**
- Create: `src/Plith/Views/Presentation/PresentationPolicy.cs`
- Create: `tests/Plith.Tests/PresentationPolicyTests.cs`
- Modify: `src/Plith/Services/SettingsModel.cs`
- Modify: `src/Plith/Services/SettingsService.cs` (`Load` around line 54, `Save` around line 111)
- Modify: `tests/Plith.Tests/SettingsServiceTests.cs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `enum PresentationMode { ClassicOsd, AmbientNotch }` in `Plith.Services`
  - `SettingsModel.Presentation` (`PresentationMode`, default `ClassicOsd`)
  - `SettingsModel.NotchStripHeightDip` (`double`, default `5`)
  - `PresentationPolicy.EdgeMarginDip(PresentationMode)`
  - `PresentationPolicy.IsAtRest(PresentationMode mode, double opacity, double targetOpacity, bool isParked)`
  - `PresentationPolicy.WantsHitTesting(PresentationMode mode, bool isParked)`
  - `PresentationPolicy.ClassicEdgeMarginDip` = `96`

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/PresentationPolicyTests.cs`:

```csharp
using Plith.Services;
using Plith.Views.Presentation;

namespace Plith.Tests;

public class PresentationPolicyTests
{
    [Fact]
    public void Classic_KeepsTheExistingEdgeMargin()
    {
        // 96 is the value OsdHost.EdgeMarginDip has always used. Classic must be
        // byte-identical to today, so this is a regression pin, not a preference.
        Assert.Equal(96, PresentationPolicy.EdgeMarginDip(PresentationMode.ClassicOsd));
    }

    [Fact]
    public void Notch_SitsFlushWithTheEdge()
    {
        Assert.Equal(0, PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch));
    }

    [Fact]
    public void Classic_IsAtRest_WhenOpacityIsBelowTarget()
    {
        Assert.True(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 0, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_IsNotAtRest_WhenFullyVisible()
    {
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 1, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_IsNotAtRest_AtAReducedTargetOpacity()
    {
        // OsdOpacityPercent can be as low as 50. "At rest" means below the target, not
        // below 1.0 — reading it as the latter would treat a 70 %-opacity OSD as hidden
        // and restart its fade-in on every repeat, which is the Phase 5 flicker defect.
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, opacity: 0.7, targetOpacity: 0.7, isParked: false));
    }

    [Fact]
    public void Notch_IsAtRest_DependsOnParkedNotOpacity()
    {
        // The notch is fully opaque while parked, so opacity says nothing about it.
        Assert.True(PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, opacity: 1, targetOpacity: 1, isParked: true));
        Assert.False(PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, opacity: 1, targetOpacity: 1, isParked: false));
    }

    [Fact]
    public void Classic_AlwaysWantsHitTesting()
    {
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: true));
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: false));
    }

    [Fact]
    public void Notch_WantsHitTesting_OnlyWhileDescended()
    {
        // The top edge is where users drag windows and reach Snap Layouts. A parked strip
        // that swallowed clicks there would be a defect, not a feature.
        Assert.False(PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, isParked: true));
        Assert.True(PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, isParked: false));
    }
}
```

Append to `tests/Plith.Tests/SettingsServiceTests.cs` (inside the existing class, matching its existing temp-path helper style):

```csharp
    [Fact]
    public void Load_ConfigWithoutPresentationKeys_DefaultsToClassic()
    {
        // Every existing 0.1.5 install has a config.ini with no [Osd] Presentation key.
        // They must keep the OSD they already have rather than being moved to a notch.
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "[Osd]\nShowDurationMs=2000\n");

        var svc = new SettingsService(path);
        svc.Load();

        Assert.Equal(PresentationMode.ClassicOsd, svc.Current.Presentation);
        Assert.Equal(5, svc.Current.NotchStripHeightDip);
    }

    [Fact]
    public void Save_Then_Load_RoundTripsPresentationSettings()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var write = new SettingsService(path);
        var m = write.Current.Clone();
        m.Presentation = PresentationMode.AmbientNotch;
        m.NotchStripHeightDip = 6;
        write.Save(m);

        var read = new SettingsService(path);
        read.Load();

        Assert.Equal(PresentationMode.AmbientNotch, read.Current.Presentation);
        Assert.Equal(6, read.Current.NotchStripHeightDip);
    }

    [Fact]
    public void Load_ClampsAnAbsurdStripHeight()
    {
        // A hand-edited config must not be able to park a 900 px "strip" across the screen.
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "[Osd]\nNotchStripHeightDip=900\n");

        var svc = new SettingsService(path);
        svc.Load();

        Assert.Equal(24, svc.Current.NotchStripHeightDip);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter "FullyQualifiedName~PresentationPolicyTests|FullyQualifiedName~SettingsServiceTests"`
Expected: build failure — `PresentationMode`, `PresentationPolicy`, `Presentation` and `NotchStripHeightDip` do not exist.

- [ ] **Step 3: Add the enum and settings properties**

In `src/Plith/Services/SettingsModel.cs`, add the enum next to `OsdPosition`:

```csharp
public enum PresentationMode
{
    /// <summary>Invisible at rest; fades in at the configured anchor. Behaviour through 0.1.5.</summary>
    ClassicOsd,
    /// <summary>A thin strip parked at the top edge that slides down on an event or hover.</summary>
    AmbientNotch,
    // FullNotch is deliberately absent until the cards that would fill its strip exist
    // (mic status ships with the System Controls card; there is no Clock card yet).
    // Adding an enum value nothing implements invites a silent fallthrough to Classic
    // that reads as a bug rather than as a deferral.
}
```

Add to `SettingsModel`, after `CustomPositionMonitorDeviceName`:

```csharp
    /// <summary>Which shell the OSD renders as. Defaults to Classic so existing installs
    /// keep the OSD they already have.</summary>
    public PresentationMode Presentation { get; set; } = PresentationMode.ClassicOsd;

    /// <summary>Height in DIP of the strip left visible while the Ambient Notch is parked.
    /// ROADMAP §3 specifies a 4–6 px band; 5 sits in the middle of it.</summary>
    public double NotchStripHeightDip { get; set; } = 5;
```

Add both to `Clone()`, after `CustomPositionMonitorDeviceName`:

```csharp
        Presentation = Presentation,
        NotchStripHeightDip = NotchStripHeightDip,
```

- [ ] **Step 4: Persist them**

In `src/Plith/Services/SettingsService.cs`, in `Load`'s initializer after `CustomPositionMonitorDeviceName`:

```csharp
                Presentation = ParseEnum(data[SectionOsd]["Presentation"], PresentationMode.ClassicOsd),
                NotchStripHeightDip = ParseDouble(data[SectionOsd]["NotchStripHeightDip"], 5, 2, 24),
```

In `Save`, after the `CustomPositionMonitorDeviceName` line:

```csharp
        data[SectionOsd]["Presentation"] = m.Presentation.ToString();
        data[SectionOsd]["NotchStripHeightDip"] = m.NotchStripHeightDip.ToString("G", inv);
```

- [ ] **Step 5: Write `PresentationPolicy`**

Create `src/Plith/Views/Presentation/PresentationPolicy.cs`:

```csharp
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// The decisions that differ between presentation modes, extracted from the WPF adapters
/// so they can be tested. The adapters hold a BandWindow — a ContentControl behind an
/// HwndSource — which the headless test suite cannot construct at all.
/// </summary>
public static class PresentationPolicy
{
    /// <summary>The margin the OSD has always kept from the working-area edge.</summary>
    public const double ClassicEdgeMarginDip = 96;

    /// <summary>The notch is flush with the top edge; a margin would leave a floating bar.</summary>
    public const double NotchEdgeMarginDip = 0;

    public static double EdgeMarginDip(PresentationMode mode)
        => mode == PresentationMode.AmbientNotch ? NotchEdgeMarginDip : ClassicEdgeMarginDip;

    /// <summary>
    /// True when the host is showing nothing the user would read as "the OSD is up".
    ///
    /// Classic answers with opacity. Notch cannot: it is fully opaque while parked, so it
    /// answers with whether the content is still pushed above the top edge. This is what
    /// OsdHost's ShowOsd and HideOsd both consult; reading opacity there would make ShowOsd
    /// think the notch was already visible and skip the descent entirely.
    /// </summary>
    public static bool IsAtRest(PresentationMode mode, double opacity, double targetOpacity, bool isParked)
        => mode == PresentationMode.AmbientNotch ? isParked : opacity < targetOpacity - 0.01;

    /// <summary>
    /// Whether the window should accept mouse messages right now.
    ///
    /// Classic always does — hover keep-alive needs it. The notch does not while parked:
    /// the top edge of the screen is where users drag windows to maximise, reach browser
    /// tabs and trigger Snap Layouts, so a strip that swallowed clicks there would be a
    /// defect rather than a feature.
    /// </summary>
    public static bool WantsHitTesting(PresentationMode mode, bool isParked)
        => mode != PresentationMode.AmbientNotch || !isParked;
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS, all tests including the 3 new settings tests and 8 policy tests.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Views/Presentation/PresentationPolicy.cs \
        src/Plith/Services/SettingsModel.cs src/Plith/Services/SettingsService.cs \
        tests/Plith.Tests/PresentationPolicyTests.cs tests/Plith.Tests/SettingsServiceTests.cs
git commit -m "feat(settings): add a presentation mode defaulting to the classic OSD"
```

---

### Task 3: The `IOsdPresentation` seam and `ClassicPresentation`

This task must produce **no user-visible change**. Classic is the only mode wired up at its
end; the notch arrives in Task 4.

**Files:**
- Create: `src/Plith/Views/Presentation/IOsdPresentation.cs`
- Create: `src/Plith/Views/Presentation/ClassicPresentation.cs`
- Modify: `src/Plith/Views/OsdHost.cs`

**Interfaces:**
- Consumes: `PresentationPolicy` (Task 2).
- Produces: `IOsdPresentation` with members `EdgeMarginDip`, `IsAtRest`, `WantsHitTesting`, `PrepareShow()`, `AnimateToVisible(double, Action)`, `SnapToVisible(double)`, `AnimateToRest(Action)`, `OnContentMeasured(Size)`.

- [ ] **Step 1: Define the seam**

Create `src/Plith/Views/Presentation/IOsdPresentation.cs`:

```csharp
using System.Windows;

namespace Plith.Views.Presentation;

/// <summary>
/// How the OSD rests, transitions and hit-tests. Deliberately three behaviours and no more.
///
/// OsdHost keeps everything else: band-window creation, the UIAccess z-band, the accent
/// resource mirror, positioning, the hide timer, hover keep-alive policy, suppression
/// wiring and position edit mode. Ambient Notch needs every one of those identically, which
/// is why it is a mode inside OsdHost rather than a second host — a second host would have
/// to re-earn each defect fixed during Phase 5 verification, including the fade-in
/// generation counter and the deliberately absent BeginAnimation(OpacityProperty, null).
/// </summary>
internal interface IOsdPresentation
{
    /// <summary>Distance Reposition() keeps from the working-area edge.</summary>
    double EdgeMarginDip { get; }

    /// <summary>True when nothing is on screen that the user would read as "the OSD is up".</summary>
    bool IsAtRest { get; }

    /// <summary>Whether the window should accept mouse messages in its current state.</summary>
    bool WantsHitTesting { get; }

    /// <summary>Called once before a show transition begins, after Reposition().</summary>
    void PrepareShow();

    /// <summary>
    /// Animate to fully visible, invoking <paramref name="onCompleted"/> at the end.
    ///
    /// Must hand off from the current animated value rather than restarting from a base
    /// value — that hand-off is what makes interrupting a hide look continuous, and losing
    /// it is exactly the flicker defect fixed in Phase 5.
    /// </summary>
    void AnimateToVisible(double targetOpacity, Action onCompleted);

    /// <summary>Go fully visible with no animation. Used when a show arrives while already up.</summary>
    void SnapToVisible(double targetOpacity);

    /// <summary>Animate back to the resting state.</summary>
    void AnimateToRest(Action onCompleted);

    /// <summary>Called after OsdHost measures its content, so a mode whose resting state
    /// depends on content height can recompute it.</summary>
    void OnContentMeasured(Size contentSize);
}
```

- [ ] **Step 2: Move today's animations behind it**

Create `src/Plith/Views/Presentation/ClassicPresentation.cs`. The animation bodies are moved
verbatim from `OsdHost.ShowOsd` / `OsdHost.FadeOutAndHide` — same durations, same easing:

```csharp
using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// The OSD as it behaved through 0.1.5: invisible at rest, opacity fade in and out at a
/// fixed anchor. Nothing here is new — the animations are moved from OsdHost unchanged so
/// that selecting Classic is byte-identical to the previous release.
/// </summary>
internal sealed class ClassicPresentation : IOsdPresentation
{
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly BandWindow _window;

    public ClassicPresentation(BandWindow window) => _window = window;

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.ClassicOsd);

    public bool IsAtRest =>
        PresentationPolicy.IsAtRest(PresentationMode.ClassicOsd, _window.Opacity, TargetOpacity, isParked: false);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.ClassicOsd, isParked: false);

    // Recorded on each transition so the policy above and the animations below agree on what
    // "fully visible" means for the current settings. OsdOpacityPercent can be as low as 50,
    // so "at rest" has to mean "below the target", not "below 1.0".
    public double TargetOpacity { get; private set; } = 1.0;

    public void PrepareShow() => _window.Show();

    public void AnimateToVisible(double targetOpacity, Action onCompleted)
    {
        TargetOpacity = targetOpacity;

        // Note the absence of BeginAnimation(OpacityProperty, null) here. Clearing an
        // animation reverts the property to its base value, which is 0 while the window is
        // hidden, so a clear-then-restart snapped the OSD back to fully invisible on every
        // repeat. A From-less DoubleAnimation hands off from the current animated value,
        // which is what makes interrupting a fade-out look continuous.
        var fadeIn = new DoubleAnimation(targetOpacity, TimeSpan.FromMilliseconds(FadeInMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        fadeIn.Completed += (_, _) => onCompleted();
        _window.BeginAnimation(UIElement.OpacityProperty, fadeIn);
    }

    public void SnapToVisible(double targetOpacity)
    {
        TargetOpacity = targetOpacity;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
    }

    public void AnimateToRest(Action onCompleted)
    {
        var fadeOut = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(FadeOutMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        fadeOut.Completed += (_, _) => onCompleted();
        _window.BeginAnimation(UIElement.OpacityProperty, fadeOut);
    }

    public void OnContentMeasured(Size contentSize) { /* Classic's resting state is content-independent. */ }
}
```

- [ ] **Step 3: Delegate from `OsdHost`**

In `src/Plith/Views/OsdHost.cs`:

Remove the `FadeInMs` / `FadeOutMs` constants and change `EdgeMarginDip` from a constant to
a read of the active presentation. Add the field and construct it in the constructor,
immediately after `Shell = new OsdShellViewModel(cardHost);`:

```csharp
    private IOsdPresentation _presentation;
```

```csharp
        _presentation = new ClassicPresentation(this);
```

Replace `ShowOsd`'s body from `bool wasHidden = ...` through the end of the `else` branch with:

```csharp
        double targetOpacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        bool wasAtRest = _presentation.IsAtRest;

        if (wasAtRest)
        {
            Reposition();
            _presentation.PrepareShow();

            if (wasFadingOut || !_isFadingIn)
            {
                _log?.Info("OsdHost",
                    $"Show: transition at {Left:0},{Top:0} for {visibleFor.TotalMilliseconds:0}ms" +
                    (wasFadingOut ? " (interrupting hide)" : string.Empty));

                _isFadingIn = true;
                int gen = ++_fadeInGeneration;
                _presentation.AnimateToVisible(targetOpacity, () =>
                {
                    if (_fadeInGeneration == gen) _isFadingIn = false;
                });
            }
        }
        else
        {
            _presentation.SnapToVisible(targetOpacity);
            _isFadingIn = false;
        }
```

Replace `HideOsd`'s opacity guard (conflict §6.4 in the spec):

```csharp
        if (_presentation.IsAtRest) return;
```

Replace `FadeOutAndHide`'s animation with:

```csharp
        _presentation.AnimateToRest(() =>
        {
            if (_showGeneration == gen) _isFadingOut = false;
        });
```

In `OnMouseEnter`, replace the two direct-opacity lines with the mode's own "stay up"
statement:

```csharp
        _presentation.SnapToVisible(Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0);
```

In `Reposition`, replace the four `EdgeMarginDip` references with `_presentation.EdgeMarginDip`,
and after the `_content.UpdateLayout()` / size read, add:

```csharp
        _presentation.OnContentMeasured(new Size(w, h));
```

In `EnterPositionEditMode`, replace the direct opacity assignment with:

```csharp
        _presentation.SnapToVisible(Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0);
```

Leave `MaybeSnapCenter` using the literal `96` it needs for the 3×3 hotspot grid — that grid
is Classic's position editor and is disabled in notch mode by Task 8, so it must not follow
`_presentation.EdgeMarginDip`. Introduce a named constant for it so the intent is explicit:

```csharp
    // The position editor's 3x3 hotspot grid is a Classic-only affordance (Task 8 disables
    // the editor in notch mode). It keeps its own margin rather than following the active
    // presentation's, whose notch value of 0 would collapse the outer hotspots onto the
    // screen edges.
    private const double EditorHotspotMarginDip = 96;
```

- [ ] **Step 4: Verify nothing changed**

Run: `dotnet build src/Plith/Plith.csproj -c Debug`
Expected: build succeeds, 0 warnings.

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS, no test count change from Task 2.

Run: `pwsh -File scripts/check-a11y.ps1`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/Presentation/IOsdPresentation.cs \
        src/Plith/Views/Presentation/ClassicPresentation.cs src/Plith/Views/OsdHost.cs
git commit -m "refactor(osd): move resting state and transitions behind a presentation seam"
```

---

### Task 4: Widen `ResolveTargetScreen` to the notch

**Files:**
- Modify: `src/Plith/Views/OsdHost.cs:322-333`

**Interfaces:**
- Consumes: `PresentationMode` (Task 2).
- Produces: no new API.

Spec §6.1. As written, `ResolveTargetScreen` honours the saved monitor device name only when
`Position == OsdPosition.Custom`. The notch is not `Custom`, so on a multi-monitor setup it
would always land on the primary display — and `docs/ROADMAP.md` §10 lists "notch pins to
which monitor" as an open question this answers.

- [ ] **Step 1: Widen the condition**

Replace the guard in `ResolveTargetScreen`:

```csharp
    private static Screen? ResolveTargetScreen(SettingsModel m)
    {
        // The saved device name is honoured for Custom placement and for the notch. Both
        // are "the user chose a display"; only the built-in anchors are display-agnostic.
        // ROADMAP §10 asked which monitor the notch pins to — this is the answer: the same
        // saved device name, matched the same way, falling back to primary when that
        // display is unplugged.
        bool usesSavedMonitor =
            m.Position == OsdPosition.Custom || m.Presentation == PresentationMode.AmbientNotch;

        if (usesSavedMonitor && !string.IsNullOrEmpty(m.CustomPositionMonitorDeviceName))
        {
            foreach (var s in Screen.AllScreens)
            {
                if (string.Equals(s.DeviceName, m.CustomPositionMonitorDeviceName, StringComparison.OrdinalIgnoreCase))
                    return s;
            }
        }
        return Screen.PrimaryScreen;
    }
```

- [ ] **Step 2: Build and test**

Run: `dotnet build src/Plith/Plith.csproj -c Debug && dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: build succeeds, tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/Plith/Views/OsdHost.cs
git commit -m "fix(osd): honour the saved monitor for the notch, not only for custom placement"
```

---

### Task 5: The notch strip visual and content transform

**Files:**
- Modify: `src/Plith/Views/OsdContent.xaml`
- Modify: `src/Plith/Views/OsdContent.xaml.cs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `OsdContent.ContentOffset` (`TranslateTransform` Y, a `double` dependency property, animatable)
  - `OsdContent.NotchStrip` (the `Border` named in XAML, for `AmbientNotchPresentation` to show/hide)
  - `OsdContent.ContentInsetDip` (`const double` = 14) so no other file hardcodes the shadow margin

- [ ] **Step 1: Add the transform and the strip**

In `src/Plith/Views/OsdContent.xaml`, wrap the existing outer `Grid` so it can slide, and add
the strip as a sibling pinned to the top:

```xml
    <Grid ClipToBounds="True">
        <!-- The card, offset upward while the notch is parked. Animating this transform
             rather than the window's Top means no SetWindowPos per frame: WPF composites
             the translation, so the descent stays smooth.

             This Grid is the file's existing outer Grid, unchanged except for the x:Name
             and the RenderTransform. Its Margin="14" and Background="Transparent" keep
             their original reasons: the margin reserves drop-shadow space so it is not
             clipped at the window bounds, and the transparent background makes the whole
             area including the margin accept hit-tests, which the position editor's drag
             grab needs (it was only firing on the inner Border before). Everything inside
             it - the Border, its DropShadowEffect and the ItemsControl with its
             AlternationCount and divider trigger - is untouched. -->
        <Grid x:Name="SlidingRoot" Margin="14" Background="Transparent">
            <Grid.RenderTransform>
                <TranslateTransform x:Name="ContentSlide" Y="0" />
            </Grid.RenderTransform>

            <Border CornerRadius="14"
                    Padding="20,16"
                    Background="{DynamicResource OsdSurfaceBrush}"
                    BorderBrush="{DynamicResource OsdBorder}"
                    BorderThickness="1"
                    SnapsToDevicePixels="True">
                <Border.Effect>
                    <DropShadowEffect ShadowDepth="0" BlurRadius="28" Opacity="0.55" Color="Black" />
                </Border.Effect>
                <!-- The existing ItemsControl block moves here verbatim. Do not retype it:
                     its comments document two non-obvious decisions (why AlternationCount
                     is 64, and why the AlternationIndex binding must stay element-level
                     rather than being "simplified" into the DataTrigger) that a rewrite
                     would drop. Cut and paste it. -->
            </Border>
        </Grid>

        <!-- The parked strip. Purely decorative: it is never keyboard-reachable and carries
             no information a screen reader could act on, so it declares an explicitly empty
             accessible name rather than none at all. AutomationProperties sits on this
             Border only because check-a11y.ps1's peerless-element rule covers Name and
             LabeledBy on layout elements — see Step 3, which moves it if the lint objects. -->
        <Border x:Name="NotchStrip"
                VerticalAlignment="Top"
                HorizontalAlignment="Stretch"
                Margin="14,0,14,0"
                Height="5"
                CornerRadius="0,0,6,6"
                Visibility="Collapsed"
                Background="{DynamicResource OsdSurfaceBrush}"
                BorderBrush="{DynamicResource OsdBorder}"
                BorderThickness="1,0,1,1" />
    </Grid>
```

- [ ] **Step 2: Expose the offset and the inset**

In `src/Plith/Views/OsdContent.xaml.cs`, add:

```csharp
    /// <summary>
    /// The drop-shadow margin on the sliding root. The card's visible border starts this far
    /// below and inside the content origin, so every geometry calculation that maps between
    /// the window rectangle and what the user actually sees has to subtract it. Exposed as a
    /// constant so no other file repeats the literal.
    /// </summary>
    public const double ContentInsetDip = 14;

    public static readonly DependencyProperty ContentOffsetProperty =
        DependencyProperty.Register(
            nameof(ContentOffset), typeof(double), typeof(OsdContent),
            new PropertyMetadata(0.0, OnContentOffsetChanged));

    /// <summary>Vertical offset of the card, in DIP. Negative parks it above the top edge.
    /// A dependency property so it can be the target of a DoubleAnimation.</summary>
    public double ContentOffset
    {
        get => (double)GetValue(ContentOffsetProperty);
        set => SetValue(ContentOffsetProperty, value);
    }

    private static void OnContentOffsetChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is OsdContent c) c.ContentSlide.Y = (double)e.NewValue;
    }

    /// <summary>Show or hide the parked strip. Height follows the user's setting.</summary>
    public void SetStrip(bool visible, double heightDip)
    {
        NotchStrip.Height = heightDip;
        NotchStrip.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
    }
```

- [ ] **Step 3: Build and lint**

Run: `dotnet build src/Plith/Plith.csproj -c Debug`
Expected: build succeeds.

Run: `pwsh -File scripts/check-a11y.ps1`
Expected: exit 0. If it reports `<Border> sets AutomationProperties.*, but WPF gives Border
no automation peer`, that is the check doing its job — remove the attribute from the Border
entirely rather than moving it, because a purely decorative strip needs no name at all and
the surrounding `UserControl` root already carries the OSD's.

- [ ] **Step 4: Commit**

```bash
git add src/Plith/Views/OsdContent.xaml src/Plith/Views/OsdContent.xaml.cs
git commit -m "feat(notch): add the parked strip and an animatable content offset"
```

---

### Task 6: `AmbientNotchPresentation`

**Files:**
- Create: `src/Plith/Views/Presentation/AmbientNotchPresentation.cs`
- Modify: `src/Plith/Views/OsdHost.cs`

**Interfaces:**
- Consumes: `NotchGeometry` (Task 1), `PresentationPolicy` (Task 2), `IOsdPresentation` (Task 3), `OsdContent.ContentOffset` / `SetStrip` / `ContentInsetDip` (Task 5).
- Produces: `AmbientNotchPresentation(BandWindow window, OsdContent content, Func<double> stripHeight)`, plus `bool IsParked`, `void Park()` and `void Retract()` for Tasks 7 and 8.

- [ ] **Step 1: Write the presentation**

Create `src/Plith/Views/Presentation/AmbientNotchPresentation.cs`:

```csharp
using System.Windows;
using System.Windows.Media.Animation;
using Plith.Interop;
using Plith.Services;

namespace Plith.Views.Presentation;

/// <summary>
/// A thin strip parked at the top edge that slides down into the full card on an event or
/// hover, then retracts. The window itself never moves and is never hidden: it stays at the
/// descended size and the content translates inside it.
/// </summary>
internal sealed class AmbientNotchPresentation : IOsdPresentation
{
    private const int DescendMs = 220;
    private const int RetractMs = 260;

    private readonly BandWindow _window;
    private readonly OsdContent _content;
    private readonly Func<double> _stripHeight;

    private double _restingOffset;
    private bool _isParked = true;

    public AmbientNotchPresentation(BandWindow window, OsdContent content, Func<double> stripHeight)
    {
        _window = window;
        _content = content;
        _stripHeight = stripHeight;
    }

    public double EdgeMarginDip => PresentationPolicy.EdgeMarginDip(PresentationMode.AmbientNotch);

    public bool IsAtRest =>
        PresentationPolicy.IsAtRest(PresentationMode.AmbientNotch, _window.Opacity, TargetOpacity, _isParked);

    public bool WantsHitTesting => PresentationPolicy.WantsHitTesting(PresentationMode.AmbientNotch, _isParked);

    public double TargetOpacity { get; private set; } = 1.0;

    /// <summary>True while only the strip is showing. Read by the hover poller and the
    /// retraction signal.</summary>
    public bool IsParked => _isParked;

    public void OnContentMeasured(Size contentSize)
    {
        _restingOffset = NotchGeometry.RestingOffset(
            contentSize.Height, _stripHeight(), OsdContent.ContentInsetDip);

        // Re-park at the new offset if the content grew or shrank (the media card appearing
        // or going away) while the notch was parked. Without this the strip would show a
        // slice of the middle of the card instead of its top edge.
        if (_isParked) _content.ContentOffset = _restingOffset;
    }

    public void PrepareShow()
    {
        // The window is permanently visible in this mode, so Show() is a first-activation
        // concern rather than a per-event one. BandWindow.Show is idempotent.
        _window.Show();
        _content.SetStrip(visible: true, heightDip: _stripHeight());
    }

    public void AnimateToVisible(double targetOpacity, Action onCompleted)
    {
        TargetOpacity = targetOpacity;
        _isParked = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;

        // From-less, exactly as in ClassicPresentation: hand off from wherever a retraction
        // in flight had got to, rather than snapping back to the parked offset first.
        var descend = new DoubleAnimation(NotchGeometry.DescendedOffset, TimeSpan.FromMilliseconds(DescendMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        descend.Completed += (_, _) => onCompleted();
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, descend);
    }

    public void SnapToVisible(double targetOpacity)
    {
        TargetOpacity = targetOpacity;
        _isParked = false;
        _window.BeginAnimation(UIElement.OpacityProperty, null);
        _window.Opacity = targetOpacity;
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = NotchGeometry.DescendedOffset;
    }

    public void AnimateToRest(Action onCompleted)
    {
        var retract = new DoubleAnimation(_restingOffset, TimeSpan.FromMilliseconds(RetractMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        retract.Completed += (_, _) => { _isParked = true; onCompleted(); };
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, retract);
    }

    /// <summary>Park immediately with no animation, strip still showing. Used when settling
    /// into notch mode, where an animated descent-then-retract would be a pointless flash.</summary>
    public void Park()
    {
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = _restingOffset;
        _isParked = true;
        _content.SetStrip(visible: true, heightDip: _stripHeight());
    }

    /// <summary>Park immediately and hide the strip entirely. Used when a window covers the
    /// monitor — see the retraction signal in FullscreenVideoWatcher.</summary>
    public void Retract()
    {
        Park();
        _content.SetStrip(visible: false, heightDip: _stripHeight());
    }
}
```

- [ ] **Step 2: Select the mode in `OsdHost`**

In `src/Plith/Views/OsdHost.cs`, replace the unconditional `_presentation = new ClassicPresentation(this);`
with a factory, and re-run it when the setting changes. Add:

```csharp
    private IOsdPresentation BuildPresentation() => _settings.Current.Presentation switch
    {
        PresentationMode.AmbientNotch =>
            new AmbientNotchPresentation(this, _content, () => _settings.Current.NotchStripHeightDip),
        _ => new ClassicPresentation(this),
    };

    // Switching modes rebuilds the presentation and returns the window to that mode's rest
    // state. Both directions need cleaning up after the other: Classic leaves Opacity at 0
    // and the strip hidden, the notch leaves a content offset and a visible strip.
    private void ApplyPresentationMode()
    {
        _hideTimer?.Stop();
        _isFadingIn = false;
        _isFadingOut = false;

        BeginAnimation(OpacityProperty, null);
        _content.BeginAnimation(OsdContent.ContentOffsetProperty, null);
        _content.ContentOffset = 0;
        _content.SetStrip(visible: false, heightDip: _settings.Current.NotchStripHeightDip);

        _presentation = BuildPresentation();
        IsClickThrough = !_presentation.WantsHitTesting;

        if (_presentation is AmbientNotchPresentation notch)
        {
            Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
            Reposition();               // measures content, which calls OnContentMeasured
            notch.PrepareShow();
            notch.Park();               // settle straight into rest; no descent flash
        }
        else
        {
            Opacity = 0;
            Hide();
        }
    }
```

Call `ApplyPresentationMode()` once at the end of the constructor (after `CreateWindow()`),
and from the existing settings subscription. Replace:

```csharp
        _settings.Changed += _ => Dispatcher.BeginInvoke(() => Reposition());
```

with:

```csharp
        _settings.Changed += _ => Dispatcher.BeginInvoke(() =>
        {
            if (_settings.Current.Presentation != _activeMode)
            {
                _activeMode = _settings.Current.Presentation;
                ApplyPresentationMode();
                return;   // ApplyPresentationMode repositions as part of settling the mode
            }
            Reposition();
        });
```

with the field:

```csharp
    private PresentationMode _activeMode;
```

- [ ] **Step 3: Build and test**

Run: `dotnet build src/Plith/Plith.csproj -c Debug && dotnet test tests/Plith.Tests/Plith.Tests.csproj && pwsh -File scripts/check-a11y.ps1`
Expected: build succeeds, tests PASS, lint exit 0.

- [ ] **Step 4: Manual check — the descent**

Run the Debug build. In `%LOCALAPPDATA%\Plith\config.ini` set `Presentation=AmbientNotch`
under `[Osd]`, then restart Plith and press a volume key.

Expected: a thin strip sits at the top-center of the screen; a volume key slides the card
down out of it and it retracts after the hide timer.

Record what you observe in `docs/PHASE6-VERIFICATION.md` (create it, following the structure
of `docs/PHASE5-VERIFICATION.md`). If it cannot be observed — for example over RDP, where the
layered window cannot be captured by any means — record it as NOT VERIFIED rather than as
passed.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/Presentation/AmbientNotchPresentation.cs src/Plith/Views/OsdHost.cs docs/PHASE6-VERIFICATION.md
git commit -m "feat(notch): add the ambient notch presentation and mode switching"
```

---

### Task 7: Hover polling

**Files:**
- Create: `src/Plith/Views/Presentation/NotchHoverPoller.cs`
- Modify: `src/Plith/Views/OsdHost.cs`

**Interfaces:**
- Consumes: `NotchGeometry` (Task 1).
- Produces: `NotchHoverPoller(Dispatcher dispatcher)` with `Rect StripRect { get; set; }`, `double DpiScale { get; set; }`, `event Action<bool>? HoverChanged`, `Start()`, `Stop()`, `Dispose()`.

- [ ] **Step 1: Write the poller**

Create `src/Plith/Views/Presentation/NotchHoverPoller.cs`:

```csharp
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;

namespace Plith.Views.Presentation;

/// <summary>
/// Reports when the cursor enters or leaves the parked strip.
///
/// This polls GetCursorPos rather than installing WH_MOUSE_LL on purpose. Mouse hooks run on
/// the input hot path, where a slow callback lags the whole system's cursor, and Plith
/// already carries one global hook (WH_KEYBOARD_LL) — a second one on a hotter path is the
/// larger risk. The strip is a few pixels tall and only needs to feel responsive to a
/// deliberate move toward it, which 60 ms comfortably covers.
///
/// The timer runs only while the notch is the active presentation and stops the moment the
/// strip retracts, so Classic pays nothing for this.
/// </summary>
internal sealed class NotchHoverPoller : IDisposable
{
    private static readonly TimeSpan Interval = TimeSpan.FromMilliseconds(60);

    private readonly DispatcherTimer _timer;
    private bool _wasInside;

    public NotchHoverPoller(Dispatcher dispatcher)
    {
        _timer = new DispatcherTimer(DispatcherPriority.Background, dispatcher) { Interval = Interval };
        _timer.Tick += (_, _) => Poll();
    }

    /// <summary>Strip rectangle in DIP. Set by OsdHost after every reposition.</summary>
    public Rect StripRect { get; set; }

    /// <summary>Scale factor of the display the strip is on. GetCursorPos reports physical
    /// pixels while StripRect is in DIP; this is the only place the two spaces meet.</summary>
    public double DpiScale { get; set; } = 1.0;

    /// <summary>True on entering the strip, false on leaving. Raised on transitions only.</summary>
    public event Action<bool>? HoverChanged;

    public void Start()
    {
        _wasInside = false;
        _timer.Start();
    }

    public void Stop()
    {
        _timer.Stop();
        // Leave the world believing the cursor is outside, so a restart cannot open with a
        // stale "still inside" that never produces an enter transition.
        if (_wasInside)
        {
            _wasInside = false;
            HoverChanged?.Invoke(false);
        }
    }

    private void Poll()
    {
        if (!GetCursorPos(out var p)) return;

        var dip = NotchGeometry.PhysicalToDip(p.X, p.Y, DpiScale);
        bool inside = NotchGeometry.IsInsideStrip(StripRect, dip);
        if (inside == _wasInside) return;

        _wasInside = inside;
        HoverChanged?.Invoke(inside);
    }

    public void Dispose() => _timer.Stop();

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X, Y; }

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);
}
```

- [ ] **Step 2: Wire it into `OsdHost`**

Add the field and construct it in the constructor:

```csharp
    private readonly NotchHoverPoller _hoverPoller;
```

```csharp
        _hoverPoller = new NotchHoverPoller(Dispatcher);
        _hoverPoller.HoverChanged += OnStripHoverChanged;
```

Add the handler:

```csharp
    // Entering the parked strip descends the notch and makes the panel interactive; leaving
    // hands back to the ordinary hide timer. IsClickThrough is toggled here rather than
    // inside the presentation because it is a window-level concern and OsdHost owns the
    // window. Note this is the first code in Plith to change IsClickThrough after the HWND
    // exists — see the manual check in Step 4.
    private void OnStripHoverChanged(bool inside)
    {
        if (_isEditMode) return;
        if (!_settings.Current.HoverKeepAlive) return;
        if (_cardHost.Suppressor?.IsSuppressed == true) return;
        if (_presentation is not AmbientNotchPresentation) return;

        if (inside)
        {
            IsClickThrough = false;
            _hideTimer?.Stop();
            ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs));
        }
        else
        {
            IsClickThrough = true;
            if (_currentVisibleFor > TimeSpan.Zero && !_isFadingOut)
                RestartHideTimer(_currentVisibleFor);
        }
    }
```

At the end of `Reposition`, publish the strip rectangle and DPI to the poller:

```csharp
        if (_presentation is AmbientNotchPresentation)
        {
            _hoverPoller.StripRect = NotchGeometry.StripRect(
                Left, Top, w, _settings.Current.NotchStripHeightDip, OsdContent.ContentInsetDip);
            _hoverPoller.DpiScale = VisualTreeHelper.GetDpi(_content).DpiScaleX;
        }
```

In `ApplyPresentationMode`, start or stop the poller with the mode:

```csharp
        if (_presentation is AmbientNotchPresentation) _hoverPoller.Start();
        else _hoverPoller.Stop();
```

Add `using System.Windows.Media;` for `VisualTreeHelper` if it is not already present.

- [ ] **Step 3: Build and test**

Run: `dotnet build src/Plith/Plith.csproj -c Debug && dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: build succeeds, tests PASS.

- [ ] **Step 4: Manual check — hover and the click-through toggle**

This is the check the spec calls out as §8.5, and it must not be skipped: `BandWindow.IsClickThrough`
supports assignment after creation, but that path has never run in Plith. `OsdHost` assigns it
once in its constructor, before `CreateWindow()`, when `HasSourceCreated` is false — so
`ToggleClickThrough` has always returned early at `if (!IsLoaded || !HasSourceCreated) return;`.
Task 7 is its first real caller, and a silently-skipped toggle leaves the strip either
swallowing clicks at the top of the screen or refusing them once descended.

With the notch active, verify each of these and record the result in `docs/PHASE6-VERIFICATION.md`:

1. Move the cursor onto the strip — the notch descends.
2. Move it away — the notch retracts after the hide timer.
3. While descended, click a media transport button — it responds.
4. While parked, drag a window to the top edge of the screen — it maximises, i.e. the strip
   did not eat the drag.
5. While parked, click a browser tab directly under the strip — it activates.

If 4 or 5 fail, `ToggleClickThrough`'s `IsLoaded` guard is the first thing to check: the
window content may not report `IsLoaded` at the moment of the toggle, in which case the fix
is to route through the same `ApplyWindowStyles` path `Activatable` and `TopMost` use, which
have no such guard.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/Presentation/NotchHoverPoller.cs src/Plith/Views/OsdHost.cs docs/PHASE6-VERIFICATION.md
git commit -m "feat(notch): descend on hover via cursor polling rather than a mouse hook"
```

---

### Task 8: Retract while a window covers the monitor

**Files:**
- Modify: `src/Plith/Services/FullscreenVideoWatcher.cs`
- Modify: `src/Plith/Views/OsdHost.cs`
- Modify: `src/Plith/App.xaml.cs:59-61` and its `Dispose` path at `:146`

**Interfaces:**
- Consumes: `AmbientNotchPresentation.Retract()` and `PrepareShow()` (Task 6).
- Produces: `FullscreenVideoWatcher.ForegroundCoversMonitor` (`bool`) and `event Action<bool>? ForegroundCoversMonitorChanged`.

Spec §4. This is **not** suppression: `IShowSuppressor` means "do not show at all", while this
means "behave like Classic". Folding one into the other would break the gate Phase 5 §2 exists
to provide.

- [ ] **Step 1: Publish the gather the watcher already performs**

`ForegroundCoversMonitor()` already computes exactly this and runs on every `Evaluate`,
regardless of the `HideDuringFullscreenVideo` flag — only `ShouldSuppress` gates on that. So
no second polling loop is added; the existing 1 Hz timer plus `EVENT_SYSTEM_FOREGROUND` hook
feeds both.

In `src/Plith/Services/FullscreenVideoWatcher.cs`, add the field and event:

```csharp
    private bool _coversMonitor;

    /// <summary>True while the foreground window covers its monitor. Published separately
    /// from suppression because the notch reacts to it differently: suppression means "do not
    /// show at all", this means "retract the parked strip and behave like Classic".</summary>
    public bool ForegroundCoversMonitor => _coversMonitor;

    public event Action<bool>? ForegroundCoversMonitorChanged;
```

In `Evaluate`, hoist the covers-monitor result out of the argument list so it can be published,
and invert the failure direction for this one output:

```csharp
    private void Evaluate()
    {
        if (_disposed) return;

        bool next;
        bool covers;
        try
        {
            covers = ForegroundCoversMonitor(out var processName);
            next = FullscreenVideoDetector.ShouldSuppress(
                enabled: _settings.Current.HideDuringFullscreenVideo,
                foregroundCoversMonitor: covers,
                notificationState: QueryNotificationState(),
                foregroundOwnsPlayingSmtc: ForegroundOwnsPlayingSmtc(processName),
                foregroundProcessName: processName,
                hideList: FullscreenVideoDetector.ParseHideList(_settings.Current.FullscreenVideoHideList));
        }
        catch (Exception ex)
        {
            _log?.Warn("FullscreenVideo", $"Evaluate threw: {ex.GetType().Name}: {ex.Message}");
            // Suppression fails toward SHOWING the OSD: a bug that hides it is worse than one
            // that shows it. Retraction fails the other way, deliberately — a strip left
            // sitting over a game is worse than one retracted when it need not have been.
            next = false;
            covers = true;
        }

        if (covers != _coversMonitor)
        {
            _coversMonitor = covers;
            _log?.Info("FullscreenVideo", $"ForegroundCoversMonitor -> {covers}");
            ForegroundCoversMonitorChanged?.Invoke(covers);
        }

        if (next == _suppressed) return;
        _suppressed = next;
        _log?.Info("FullscreenVideo", $"Suppression -> {next}");
        SuppressionChanged?.Invoke(next);
    }
```

Rename the private helper to avoid colliding with the new property:

```csharp
    private static bool ForegroundCoversItsMonitor(out string processName)
```

and update its single call site above.

- [ ] **Step 2: React in `OsdHost`**

Add a public entry point rather than having `OsdHost` reach into the watcher, so the wiring
direction matches how suppression is already wired through `CardHost`:

```csharp
    /// <summary>Called when a window starts or stops covering its monitor. The Ambient Notch
    /// retracts entirely while covered and returns afterwards; Classic ignores it.
    ///
    /// One rule, no game-versus-video classifier: a persistent strip over a fullscreen film is
    /// as unwelcome as one over a game, and a rule with no classifier in it has no classifier
    /// to get wrong.</summary>
    public void OnForegroundCoversMonitorChanged(bool covers)
    {
        if (_presentation is not AmbientNotchPresentation notch) return;

        if (covers)
        {
            _hideTimer?.Stop();
            _hoverPoller.Stop();
            IsClickThrough = true;
            notch.Retract();
        }
        else
        {
            notch.PrepareShow();
            Reposition();
            _hoverPoller.Start();
        }
    }
```

In `ApplyPresentationMode`, the existing `_hoverPoller.Start()` call must not resurrect the
strip over a game. Guard it:

```csharp
        if (_presentation is AmbientNotchPresentation && !_coversMonitor) _hoverPoller.Start();
        else _hoverPoller.Stop();
```

with `OnForegroundCoversMonitorChanged` recording `_coversMonitor = covers;` first, and the
field:

```csharp
    private bool _coversMonitor;
```

- [ ] **Step 3: Wire it up**

The watcher is built in `src/Plith/App.xaml.cs`, not in `OsdOrchestrator`. Suppression reaches
`CardHost` by constructor injection, so there is no existing subscription to sit next to:

```csharp
_fullscreenWatcher = new FullscreenVideoWatcher(_settings, _mediaSession, Dispatcher, _diagnosticLog);   // :59
_cardHost = new CardHost(_settings, _fullscreenWatcher);                                                  // :61
```

Retraction is a different channel to a different consumer, so it is an explicit subscription.
Add it after `_osd = new OsdHost(...)` at `:65`, alongside the two `_cardHost` subscriptions
at `:66-67`:

```csharp
        // Suppression reaches CardHost by injection above; this is the separate signal the
        // notch needs. Deliberately not routed through IShowSuppressor: that means "do not
        // show at all", while this means "retract the strip and behave like Classic".
        _fullscreenWatcher.ForegroundCoversMonitorChanged += _osd.OnForegroundCoversMonitorChanged;
```

Unsubscribe in the shutdown path, before the existing watcher disposal at `:146`:

```csharp
        DisposeStep("FullscreenVideoWatcher", () =>
        {
            if (_fullscreenWatcher is not null && _osd is not null)
                _fullscreenWatcher.ForegroundCoversMonitorChanged -= _osd.OnForegroundCoversMonitorChanged;
            _fullscreenWatcher?.Dispose();
        });
```

- [ ] **Step 4: Build and test**

Run: `dotnet build src/Plith/Plith.csproj -c Debug && dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: build succeeds, tests PASS (the existing `FullscreenVideoDetectorTests` must be
untouched — `ShouldSuppress`'s signature and behaviour do not change).

- [ ] **Step 5: Manual check — retraction**

Per spec §8.4, on the **installed Program Files build** (Debug builds out of `bin\` have no
UIAccess, which silently changes what the OSD can draw over — see `Plith.Interop.UiAccess`):

1. Open a game, alt-tab into it. Confirm `ForegroundCoversMonitor -> True` in `plith.log` and
   that the strip is gone.
2. Press a volume key while in the game. The OSD must still appear — retraction is not
   suppression.
3. Alt-tab out. Confirm `-> False` and the strip returns.

Record the result in `docs/PHASE6-VERIFICATION.md`, including the log lines. If the game
cannot be run, record NOT VERIFIED — do not infer the result from the code.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Services/FullscreenVideoWatcher.cs src/Plith/Services/OsdOrchestrator.cs \
        src/Plith/Views/OsdHost.cs docs/PHASE6-VERIFICATION.md
git commit -m "feat(notch): retract the strip while a window covers the monitor"
```

---

### Task 9: Settings UI

**Files:**
- Modify: `src/Plith/Views/SettingsWindow.xaml`
- Modify: `src/Plith/Views/SettingsWindow.xaml.cs`

**Interfaces:**
- Consumes: `PresentationMode`, `SettingsModel.Presentation`, `SettingsModel.NotchStripHeightDip` (Task 2).
- Produces: no new API.

- [ ] **Step 1: Add the mode picker and strip height**

In `src/Plith/Views/SettingsWindow.xaml`, insert two rows immediately above the existing
Position row (the `Border Style="{StaticResource RowStyle}"` at `:320` containing
`PositionSummary` and `OpenPositionOverlayButton`). Follow that row's structure exactly —
two-column `Grid`, `RowLabelStyle` for the label, `RowHintStyle` for the explanatory line,
control in column 1 — and `ThemeCombo` at `:162` for the combo's own styling:

```xml
                        <Border Style="{StaticResource RowStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*" />
                                    <ColumnDefinition Width="Auto" />
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,24,0">
                                    <TextBlock Style="{StaticResource RowLabelStyle}" Text="Presentation" />
                                    <TextBlock Style="{StaticResource RowHintStyle}"
                                               Text="Classic OSD fades in place wherever you put it. Ambient Notch parks a thin strip at the top of the screen that slides down when something changes." />
                                </StackPanel>
                                <ComboBox x:Name="PresentationCombo"
                                          Grid.Column="1"
                                          VerticalAlignment="Center"
                                          AutomationProperties.Name="Presentation mode" />
                            </Grid>
                        </Border>

                        <Border x:Name="StripHeightRow" Style="{StaticResource RowStyle}" Visibility="Collapsed">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*" />
                                    <ColumnDefinition Width="Auto" />
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,24,0">
                                    <TextBlock Style="{StaticResource RowLabelStyle}" Text="Notch strip height" />
                                    <TextBlock Style="{StaticResource RowHintStyle}"
                                               Text="How tall the parked strip is, in pixels." />
                                </StackPanel>
                                <Slider x:Name="StripHeightSlider"
                                        Grid.Column="1"
                                        Width="180"
                                        VerticalAlignment="Center"
                                        Minimum="2" Maximum="24" TickFrequency="1" IsSnapToTickEnabled="True"
                                        AutomationProperties.Name="Notch strip height in pixels" />
                            </Grid>
                        </Border>
```

Copy the `Style` attribute from `ThemeCombo` onto `PresentationCombo` verbatim; do not guess
a style name. Visibility is toggled on the whole `StripHeightRow` border rather than on the
label and slider separately, so the row's padding collapses with it.

- [ ] **Step 2: Bind them, and disable Edit Position in notch mode**

In `src/Plith/Views/SettingsWindow.xaml.cs`, populate `PresentationCombo` from the enum and
persist on change, following the file's existing pattern for `ThemeCombo`. Bind
`StripHeightSlider` to `NotchStripHeightDip` the same way the other numeric settings rows are
bound. Then add the guard the spec requires (§6.2):

```csharp
    // Position edit mode drags the OSD and saves the result as OsdPosition.Custom. The notch
    // is pinned to top-center, so letting the user through here would silently overwrite the
    // pinned anchor with a Custom one and leave the notch somewhere it can never return from.
    // The strip height row is Classic-irrelevant, so it appears under the inverse rule.
    private void UpdatePresentationDependentControls()
    {
        bool isNotch = _settings.Current.Presentation == PresentationMode.AmbientNotch;

        OpenPositionOverlayButton.IsEnabled = !isNotch;
        OpenPositionOverlayButton.ToolTip = isNotch
            ? "The Ambient Notch is pinned to the top of the screen. Switch to Classic OSD to place the OSD yourself."
            : null;

        // The hint text next to the button explains what the button does. When the button is
        // disabled that sentence describes something the user cannot do, so say why instead.
        PositionSummary.Text = isNotch
            ? "The Ambient Notch is pinned to the top of the screen, so there is nothing to place. Switch to Classic OSD to choose a position."
            : "Click 'Set position' to place the OSD anywhere on any monitor. A dim overlay with nine snap hotspots opens over your desktop.";

        StripHeightRow.Visibility = isNotch ? Visibility.Visible : Visibility.Collapsed;
    }
```

Call it from the settings-load path and from `PresentationCombo`'s change handler.

- [ ] **Step 3: Build, test and lint**

Run: `dotnet build src/Plith/Plith.csproj -c Debug && dotnet test tests/Plith.Tests/Plith.Tests.csproj && pwsh -File scripts/check-a11y.ps1`
Expected: build succeeds, tests PASS, lint exit 0. The lint will fail on the new `ComboBox` and
`Slider` if either is missing `AutomationProperties.Name` — that is the guard working.

- [ ] **Step 4: Manual check**

Open Settings. Switch to Ambient Notch: the notch appears live, Edit Position greys out with
the tooltip, and the strip height slider appears. Switch back to Classic: the notch is gone,
the OSD behaves exactly as before, and Edit Position is enabled again. Record in
`docs/PHASE6-VERIFICATION.md`.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/SettingsWindow.xaml src/Plith/Views/SettingsWindow.xaml.cs docs/PHASE6-VERIFICATION.md
git commit -m "feat(settings): add the presentation picker and pin position editing to classic"
```

---

### Task 10: Documentation and the verification ledger

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/PHASE6-VERIFICATION.md`

- [ ] **Step 1: Complete the verification ledger**

`docs/PHASE6-VERIFICATION.md` has been accumulating entries since Task 6. Finish it by adding
the checks that no task produced, taken from spec §8, each recorded as NOT VERIFIED until
someone actually runs it:

1. The strip does not interfere with Snap Layouts hover or auto-hide taskbar reveal.
2. Idle resource measurement with the window never hidden. Phase 5 measured GDI handles flat
   at 22 across 160 volume changes, but that scenario hid the window between events, so it
   does not carry over.
3. The descent reads as motion, not a jump, on both a 60 Hz and a high-refresh display.

Write NOT VERIFIED where a check has not been run. Phase 5 found four real defects inside work
already marked complete, every one of them behind a green build, green tests and a green lint —
an unrun check recorded as passed is the mechanism by which that happens.

- [ ] **Step 2: Update `CHANGELOG.md`**

Add an entry describing the presentation modes. State only what was verified: that Classic is
unchanged, that the notch descends and retracts, and that retraction over a game either was or
was not confirmed on hardware. Do not claim a protection that has not been observed — a Phase 5
changelog entry claimed exactly that about the borderless D3D veto and was wrong.

- [ ] **Step 3: Update `CLAUDE.md` and `docs/ROADMAP.md`**

In `CLAUDE.md`, update the Status block: Phase 6 slice 1 is code-complete on
`feature/phase-6-notch-shell`, with the open items from the verification ledger listed.

In `docs/ROADMAP.md`:
- Mark the notch geometry and Ambient Notch items under Phase 6 as shipped.
- Note that Full Notch is deferred to a later slice and why.
- Close the §10 open question "Multi-monitor: notch pins to which monitor?" — answered by
  Task 4, the saved device name with a primary fallback.

- [ ] **Step 4: Full verification pass**

```bash
dotnet build src/Plith/Plith.csproj -c Release
dotnet test tests/Plith.Tests/Plith.Tests.csproj
dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj
pwsh -File scripts/check-a11y.ps1
```
Expected: Release build succeeds; both suites pass; lint exit 0.

- [ ] **Step 5: Commit**

```bash
git add CHANGELOG.md CLAUDE.md docs/ROADMAP.md docs/PHASE6-VERIFICATION.md
git commit -m "docs: record the notch shell and what about it is still unverified"
```

---

## Notes for the executor

- **Do not push.** This branch is `feature/phase-6-notch-shell`; the user approves every push
  separately.
- **Never delete the `CN=Plith Self-Signed` certificate.** `tests/Plith.Installer.Tests` filters
  on subject `Plith Test`, which cannot match it — leave that filter alone.
- **Do not drive the GUI with synthetic input.** No injected mouse or keyboard events, no
  activating or focusing windows, no launching and driving the app interactively. Every manual
  check in this plan is a human step; if you cannot observe a result, record NOT VERIFIED.
- If a manual check contradicts this plan, the plan is wrong. Say so and stop rather than
  adjusting the check.
