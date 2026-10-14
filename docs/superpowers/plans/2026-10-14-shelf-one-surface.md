# Shelf: one surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the page turn onto the shelf a single process's single animation, identical to the notch's other four pages, by having Plith draw the same shelf control the catcher draws.

**Architecture:** A new `Plith.Shelf` assembly becomes the single home for everything both processes share, replacing the five source links the catcher uses today. Plith's shelf page stops being blank and hosts the same `ShelfSurface` the catcher hosts, fed from the `ShelfStore` Plith already owns, so the handover covers one picture with an identical picture.

**Tech Stack:** .NET 10 (net10.0-windows10.0.22000.0), WPF, x64. PowerShell 7 for the render harness. xunit for the test projects.

**Spec:** `docs/superpowers/specs/2026-10-14-shelf-one-surface-design.md`

## Global Constraints

- All code, comments, identifiers, test names and commit messages in English. Only user-facing strings may be Turkish.
- Conventional Commits, subject at most 50 characters, body as bullets, no AI attribution anywhere.
- Never the em dash, in code, comments, docs or commit messages.
- `Plith.DropCatcher` must never reference `Plith.dll`. It may reference `Plith.Shelf`.
- `Plith.DropCatcher` stays `asInvoker` in every configuration. Its `app.manifest` is not touched by this plan.
- Namespaces do not change when a type moves assemblies: `NotchGeometry` stays `Plith.Views.Presentation`, the palette types stay `Plith.Services.Shelf`.
- Target framework, platform and analyzer settings of the new assembly match `Plith.DropCatcher.csproj`: `net10.0-windows10.0.22000.0`, `x64`, `Nullable enable`, `ImplicitUsings enable`, `LangVersion latest`, `UseWPF true`, `EnableNETAnalyzers true`, `AnalysisLevel latest`, `AnalysisMode Recommended`.
- Heavy commands (builds, test suites, render runs) go through `node ~/.claude/bin/heavy-run.mjs -- <command>`.
- A claim of "works" requires a render or a run, never a green build alone.

## Review Focus

1. **The store changes while the shelf page is on screen.** `ShelfWidget` deliberately stopped watching `ShelfStore` when it became blank ("It has nothing to repaint"). Drawing real content makes that subscription mandatory again; without it, a drop or a removal leaves Plith's page showing yesterday's files and the handover becomes visible again as the picture changes under the fade. Covered by Task 3.
2. **The store is empty.** Both sides must draw the same empty state, or the seam reappears in exactly the case a new user sees first. Covered by Task 4.
3. **A palette arrives after the page exists**, which is what a runtime theme or accent switch is. Plith's page must repaint from the new palette the same way the catcher's does. Covered by Task 3.
4. **The catcher cannot be reached.** `ShelfWidget.ShowUnavailable` puts the reason on the page, and that sentence must still be legible now that the page draws a shelf underneath it. Covered by Task 3.
5. **The first paint of the page extracts shell icons on Plith's UI thread.** The catcher's own first window cost 182 ms for an analogous reason. A page turn that blocks is the defect this whole plan exists to remove. Covered by Task 5.

---

### Task 1: `Plith.Shelf` exists and owns the five shared files

**Files:**
- Create: `src/Plith.Shelf/Plith.Shelf.csproj`
- Move: `src/Plith/Views/Presentation/NotchGeometry.cs` to `src/Plith.Shelf/NotchGeometry.cs`
- Move: `src/Plith/Services/Shelf/ShelfPaletteWire.cs` to `src/Plith.Shelf/ShelfPaletteWire.cs`
- Move: `src/Plith/Services/Shelf/ShelfLabel.cs` to `src/Plith.Shelf/ShelfLabel.cs`
- Move: `src/Plith/Services/Shelf/DropChannel.cs` to `src/Plith.Shelf/DropChannel.cs`
- Move: `src/Plith/Services/WheelDecoder.cs` to `src/Plith.Shelf/WheelDecoder.cs`
- Modify: `src/Plith.DropCatcher/Plith.DropCatcher.csproj` (delete all five `<Compile Include="..\Plith\...">` links, add a `ProjectReference`)
- Modify: `src/Plith/Plith.csproj` (add a `ProjectReference`)
- Modify: `tests/Plith.Tests/Plith.Tests.csproj` (add a `ProjectReference`)
- Modify: `Plith.sln`
- Test: `tests/Plith.Tests/SharedAssemblyTests.cs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: an assembly named `Plith.Shelf` holding `Plith.Views.Presentation.NotchGeometry`, `Plith.Services.Shelf.ShelfPalette`, `Plith.Services.Shelf.ShelfPaletteWire`, `Plith.Services.Shelf.ShelfLabel`, `Plith.Services.Shelf.DropChannelServer`, `Plith.Services.WheelDecoder`. Task 2 adds the shelf UI to the same assembly.

- [ ] **Step 1: Write the failing test**

```csharp
// tests/Plith.Tests/SharedAssemblyTests.cs
using Plith.Services.Shelf;
using Plith.Views.Presentation;
using Xunit;

namespace Plith.Tests;

/// <summary>
/// The point of Plith.Shelf is that a type crossing the process boundary exists ONCE. Linking a
/// source file into two projects produces two CLR types that merely share a name, which already
/// cost this repo a workaround in render-widgets.ps1. These assertions are what stop a future
/// change putting a link back.
/// </summary>
public class SharedAssemblyTests
{
    [Theory]
    [InlineData(typeof(NotchGeometry))]
    [InlineData(typeof(ShelfPalette))]
    [InlineData(typeof(ShelfLabel))]
    public void SharedTypesLiveInTheSharedAssembly(Type type) =>
        Assert.Equal("Plith.Shelf", type.Assembly.GetName().Name);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter SharedAssemblyTests`
Expected: FAIL, three times, each reporting `Assert.Equal() Failure: Expected: Plith.Shelf, Actual: Plith`.

- [ ] **Step 3: Create the project**

```xml
<!-- src/Plith.Shelf/Plith.Shelf.csproj -->
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net10.0-windows10.0.22000.0</TargetFramework>
    <SupportedOSPlatformVersion>10.0.22000.0</SupportedOSPlatformVersion>
    <RootNamespace>Plith.Shelf</RootNamespace>
    <AssemblyName>Plith.Shelf</AssemblyName>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <LangVersion>latest</LangVersion>
    <UseWPF>true</UseWPF>
    <Platforms>x64</Platforms>
    <PlatformTarget>x64</PlatformTarget>
    <EnableNETAnalyzers>true</EnableNETAnalyzers>
    <AnalysisLevel>latest</AnalysisLevel>
    <AnalysisMode>Recommended</AnalysisMode>
  </PropertyGroup>

</Project>
```

`RootNamespace` is `Plith.Shelf` but no moved file changes its own `namespace` declaration. The assembly name and the namespaces are independent, and keeping the namespaces is what makes this a move rather than a rewrite of every call site.

- [ ] **Step 4: Move the five files with git, so history follows them**

```bash
git mv src/Plith/Views/Presentation/NotchGeometry.cs src/Plith.Shelf/NotchGeometry.cs
git mv src/Plith/Services/Shelf/ShelfPaletteWire.cs src/Plith.Shelf/ShelfPaletteWire.cs
git mv src/Plith/Services/Shelf/ShelfLabel.cs src/Plith.Shelf/ShelfLabel.cs
git mv src/Plith/Services/Shelf/DropChannel.cs src/Plith.Shelf/DropChannel.cs
git mv src/Plith/Services/WheelDecoder.cs src/Plith.Shelf/WheelDecoder.cs
```

- [ ] **Step 5: Delete every link and add the references**

In `src/Plith.DropCatcher/Plith.DropCatcher.csproj`, delete the whole `<ItemGroup>` holding the five `<Compile Include="..\Plith\...">` entries and their comments, and replace it with:

```xml
  <ItemGroup>
    <!-- A reference, not five source links. Linking compiled each shared file into both
         assemblies, which made two CLR types per file that merely shared a name: see
         SharedAssemblyTests. This project still does NOT reference Plith.dll, and the reason
         given for linking (a reference would pull Plith's assembly into a process that must stay
         small) does not apply to an assembly holding only what both ends already share. -->
    <ProjectReference Include="..\Plith.Shelf\Plith.Shelf.csproj" />
  </ItemGroup>
```

Add the same `ProjectReference` line to `src/Plith/Plith.csproj` and `tests/Plith.Tests/Plith.Tests.csproj`, and add the project to `Plith.sln` with `dotnet sln add src/Plith.Shelf/Plith.Shelf.csproj`.

- [ ] **Step 6: Run the test to verify it passes**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter SharedAssemblyTests`
Expected: PASS, 3 of 3.

- [ ] **Step 7: Run both suites and both renders**

Stop any running Plith first, or the build cannot overwrite `Plith.exe`: `sudo taskkill /F /IM Plith.exe /T`.

Run, and all four must be clean:
```
node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug
node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj -c Debug
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/check-shared-xaml.ps1
```
Expected: 773 and 17 passing, the render reaching `Done.` with every shelf check passing, and the shared-XAML check passing. The last one matters more than it looks: 0.1.6 shipped a broken installer because shared XAML named an assembly, and this task adds an assembly.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(shelf): one assembly for what both processes share

- five source links became one project reference
- a linked file compiles into two CLR types that share a name only
- SharedAssemblyTests is the guard against a link coming back"
```

---

### Task 2: The shelf UI moves into `Plith.Shelf`

**Files:**
- Move: `src/Plith.DropCatcher/Shelf/ShelfModel.cs` to `src/Plith.Shelf/ShelfModel.cs`
- Move: `src/Plith.DropCatcher/Shelf/ShellIcons.cs` to `src/Plith.Shelf/ShellIcons.cs`
- Move: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml` to `src/Plith.Shelf/ShelfSurface.xaml`
- Move: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs` to `src/Plith.Shelf/ShelfSurface.xaml.cs`
- Modify: `tests/Plith.Tests/Plith.Tests.csproj` (delete the `<Compile Include="..\..\src\Plith.DropCatcher\Shelf\ShelfModel.cs">` link)
- Modify: `tests/Plith.Tests/SharedAssemblyTests.cs`
- Modify: `scripts/render-widgets.ps1` (delete the two-assembly palette workaround)

**Interfaces:**
- Consumes: the `Plith.Shelf` assembly and its project references from Task 1.
- Produces: `Plith.DropCatcher.Shelf.ShelfSurface` resolvable from both processes as ONE type, with its existing public surface unchanged: `Apply(ShelfPalette)`, `Render(ShelfModel)`, `ShowName(string)`, `ShowCount()`.

- [ ] **Step 1: Write the failing test**

Add to `tests/Plith.Tests/SharedAssemblyTests.cs`:

```csharp
    /// <summary>
    /// The UI itself, which is the point of the exercise: Plith's shelf page and the catcher's
    /// window draw the SAME control, so there is no second implementation that can look different.
    /// </summary>
    [Theory]
    [InlineData(typeof(Plith.DropCatcher.Shelf.ShelfSurface))]
    [InlineData(typeof(Plith.DropCatcher.Shelf.ShelfModel))]
    public void TheShelfUiLivesInTheSharedAssembly(Type type) =>
        Assert.Equal("Plith.Shelf", type.Assembly.GetName().Name);
```

The namespace stays `Plith.DropCatcher.Shelf` even though the assembly is `Plith.Shelf`. Renaming the namespace would touch every call site in the catcher for no behaviour, and this plan keeps moves and renames apart.

- [ ] **Step 2: Run the test to verify it fails**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter SharedAssemblyTests`
Expected: FAIL on the two new cases with `Expected: Plith.Shelf, Actual: Plith.DropCatcher`. It may instead fail to COMPILE with `CS0234` because the test project cannot see `Plith.DropCatcher.Shelf.ShelfSurface` yet; that is the same failure and the same fix.

- [ ] **Step 3: Move the files**

```bash
git mv src/Plith.DropCatcher/Shelf/ShelfModel.cs src/Plith.Shelf/ShelfModel.cs
git mv src/Plith.DropCatcher/Shelf/ShellIcons.cs src/Plith.Shelf/ShellIcons.cs
git mv src/Plith.DropCatcher/Shelf/ShelfSurface.xaml src/Plith.Shelf/ShelfSurface.xaml
git mv src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs src/Plith.Shelf/ShelfSurface.xaml.cs
```

Then delete the `ShelfModel.cs` `<Compile Include=...>` link from `tests/Plith.Tests/Plith.Tests.csproj`: the test project references `Plith.Shelf` now and a link would reintroduce the duplicate type the whole task exists to remove.

- [ ] **Step 4: Run the test to verify it passes**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter SharedAssemblyTests`
Expected: PASS, 5 of 5.

- [ ] **Step 5: Delete the harness workaround, which is the proof**

In `scripts/render-widgets.ps1`, the palette is built through the catcher assembly's own `Type` object because the name bound to two assemblies. One type means the workaround is dead code. Replace this:

```powershell
$shelfPaletteType = $dcAssembly.GetType('Plith.Services.Shelf.ShelfPalette')
$shelfPalette = [Activator]::CreateInstance($shelfPaletteType, @(
    $wirePalette.SurfaceStart, $wirePalette.SurfaceEnd, $wirePalette.Ink, $wirePalette.InkMuted,
    $wirePalette.Track, $wirePalette.Accent, $wirePalette.SelectionRing, $wirePalette.IsDark))
```

with this, and delete the paragraph of comment above it that explains the two-assembly hazard, replacing it with one line saying the hazard is gone and naming `SharedAssemblyTests` as the guard:

```powershell
# ONE type now: ShelfPalette lives in Plith.Shelf and both processes reference it, so the bare
# literal binds to the only one there is. SharedAssemblyTests is what keeps that true.
$shelfPalette = [Plith.Services.Shelf.ShelfPalette]::new(
    $wirePalette.SurfaceStart, $wirePalette.SurfaceEnd, $wirePalette.Ink, $wirePalette.InkMuted,
    $wirePalette.Track, $wirePalette.Accent, $wirePalette.SelectionRing, $wirePalette.IsDark)
```

- [ ] **Step 6: Run the render and both suites**

Run:
```
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1 -Theme Light
node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug
node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj -c Debug
```
Expected: both renders reach `Done.` with `floor check`, `literal check`, `hover check` and `tile-hit check` all passing, and both suites green. **Open `shelf-surface.png` from both runs and look at it.** The control has not changed, so the picture must not have either; a difference here means the move changed something, which is the one thing it must not do.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(shelf): move the surface into the shared assembly

- ShelfSurface, ShellIcons and ShelfModel now have one type identity
- the render harness workaround for the duplicate type is deleted,
  which is the cheapest proof the hazard is gone"
```

---

### Task 3: Plith's shelf page draws the real shelf

**Files:**
- Modify: `src/Plith/Views/Widgets/ShelfWidget.xaml`
- Modify: `src/Plith/Views/Widgets/ShelfWidget.cs` (keep `ShowUnavailable`, add `ModelFor`)
- Modify: `src/Plith/Views/OsdHost.cs` (where the shelf page is constructed)
- Modify: `src/Plith/App.xaml.cs` (hand the palette resolver to the page)
- Test: `tests/Plith.Tests/ShelfWidgetTests.cs`

**Interfaces:**
- Consumes: `Plith.DropCatcher.Shelf.ShelfSurface` and `ShelfModel` from Task 2, `ShelfPalette` from Task 1.
- Produces: `ShelfWidget(ShelfStore store, Func<ShelfPalette> palette)`, with `ShowUnavailable(string reason)` unchanged. Task 4 asserts this page's tree against the catcher's.

- [ ] **Step 1: Write the failing test**

`ShelfWidget` is a WPF `UserControl`, so the test drives the pure part: the model the page builds
from the store. Extract that decision into a static method the test can call with no UI thread,
and have the control use the same method.

```csharp
// tests/Plith.Tests/ShelfWidgetTests.cs
using Plith.DropCatcher.Shelf;
using Plith.Services.Shelf;
using Xunit;

namespace Plith.Tests;

/// <summary>
/// The page is no longer blank, so what is worth testing without a UI thread is the model it
/// builds. ShelfWidget stopped watching ShelfStore when it drew nothing ("It has nothing to
/// repaint"); drawing real content makes that subscription mandatory, and a page showing
/// yesterday's files would make the handover visible again as the picture changes under the fade.
/// </summary>
public class ShelfWidgetTests
{
    [Fact]
    public void BuildsAModelCarryingEveryStoredPathInOrder()
    {
        var store = new ShelfStore(Path.Combine(Path.GetTempPath(), Path.GetRandomFileName()));
        var first = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());
        var second = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());
        File.WriteAllText(first, "x");
        File.WriteAllText(second, "x");
        store.Add([first, second]);

        var model = ShelfWidget.ModelFor(store);

        Assert.Equal([first, second], model.Items.Select(i => i.Path));
        Assert.Empty(model.Selection);
    }

    [Fact]
    public void BuildsAnEmptyModelForAnEmptyStore()
    {
        var store = new ShelfStore(Path.Combine(Path.GetTempPath(), Path.GetRandomFileName()));

        Assert.Empty(ShelfWidget.ModelFor(store).Items);
    }
}
```

`ShelfStore.Add(IEnumerable<string> paths)` returns the number it kept; the count is ignored here
because what is under test is the model, not the store. `ShelfStore` refuses a path it cannot
stat, which is why the test writes two real temporary files rather than passing invented names.

- [ ] **Step 2: Run the test to verify it fails**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter ShelfWidgetTests`
Expected: FAIL, because `ShelfWidget` takes no palette and subscribes to nothing.

- [ ] **Step 3: Implement**

`ShelfWidget.xaml` gets a single `ContentControl x:Name="SurfaceHost"` filling the page, with the unavailable sentence kept as a sibling drawn ON TOP, so a reachable catcher shows the shelf and an unreachable one still shows why. The code behind:

```csharp
public ShelfWidget(ShelfStore store, Func<ShelfPalette> palette)
{
    InitializeComponent();
    _store = store;
    _palette = palette;

    _surface = new ShelfSurface();
    SurfaceHost.Content = _surface;

    // SUBSCRIBED AGAIN, and the comment that said this page has nothing to repaint is deleted
    // with it. It had nothing to repaint while it drew nothing.
    _store.Changed += OnStoreChanged;
    Loaded += (_, _) => Repaint();
    Unloaded += (_, _) => _store.Changed -= OnStoreChanged;
}

private void OnStoreChanged(object? sender, EventArgs e) => Dispatcher.BeginInvoke(Repaint);

/// <summary>Palette first, then content: Apply fills the control's own Resources and the tiles
/// resolve their brushes from there as they are built. Read fresh on every repaint so a runtime
/// theme or accent switch lands here the same way it lands on the catcher's copy.</summary>
private void Repaint()
{
    _surface.Apply(_palette());
    _surface.Render(ModelFor(_store));
}

/// <summary>The store to the control's model, static and UI-free so a test can call it. Nothing
/// is selected: selection is a gesture, gestures land on the catcher's window, and this page is
/// the picture that the window covers.</summary>
public static ShelfModel ModelFor(ShelfStore store)
{
    var model = new ShelfModel();
    model.SetItems([.. store.Items.Select(i => i.Path)]);
    return model;
}
```

`OsdHost`'s shelf page construction and `App.xaml.cs` pass `ResolveShelfPalette` through. `ResolveShelfPalette` is private in `App` today; make it internal or pass it as the `Func<ShelfPalette>` the `ShelfSession` already receives, whichever touches less.

- [ ] **Step 4: Run the test to verify it passes**

Run: `node ~/.claude/bin/heavy-run.mjs -- dotnet test tests/Plith.Tests/Plith.Tests.csproj -c Debug --filter ShelfWidgetTests`
Expected: PASS.

- [ ] **Step 5: Run the full suites and the renders**

Run the four commands from Task 1 Step 7. Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(shelf): Plith's shelf page draws the real shelf

- the page hosted nothing, so the notch emptied before the handover
- it hosts the shared ShelfSurface now, from the store Plith owns
- it watches the store again, which a blank page did not need"
```

---

### Task 4: The parity guard, which is the design's only real assertion

**Files:**
- Modify: `scripts/render-widgets.ps1`
- Create: `docs/screenshots/shelf-parity-dark.png` and `-light.png` (written by the harness into `-OutDir`, not committed; the paths here name what to look at)

**Interfaces:**
- Consumes: `ShelfWidget` from Task 3 and `ShelfSurface` from Task 2.
- Produces: a `parity check` line in the harness output, and a build failure when the two trees differ.

- [ ] **Step 1: Write the failing check**

In `scripts/render-widgets.ps1`, after the catcher's `$shelfSurface` is built and laid out, build Plith's page against the SAME store and the SAME palette, lay it out at the same size, and compare. Add:

```powershell
# --- the two pictures are one picture ---------------------------------------------------------
#
# The whole design rests on this and nothing else can see it. Plith draws the shelf during the
# page turn and the catcher's window covers it a moment later; if the two differ by a tile, a
# rectangle or a brush, the handover becomes visible again and the reported symptom comes back.
$plithPage = [Plith.Views.Widgets.ShelfWidget]::new($store, [Func[Plith.Services.Shelf.ShelfPalette]]{ $shelfPalette })
$parityHost = [Windows.Controls.Border]::new()
$parityHost.Width = $shelfSurfaceW
$parityHost.Height = $shelfSurfaceH
$parityHost.Child = $plithPage
$parityHost.Measure([Windows.Size]::new($shelfSurfaceW, $shelfSurfaceH))
$parityHost.Arrange([Windows.Rect]::new(0, 0, $shelfSurfaceW, $shelfSurfaceH))
$parityHost.UpdateLayout()

$describe = {
    param($root, $relativeTo)
    $tiles = @(Find-VisualDescendants -Root $root -Predicate $findAnyTile)
    $tiles | ForEach-Object {
        $p = $_.TranslatePoint([Windows.Point]::new(0, 0), $relativeTo)
        '{0:0.0},{1:0.0} {2:0.0}x{3:0.0} bg={4} border={5}' -f $p.X, $p.Y, $_.ActualWidth, $_.ActualHeight,
            $_.Background.Color, $_.BorderBrush.Color
    }
}
$catcherTiles = & $describe $shelfSurface $layoutHost
$plithTiles = & $describe $plithPage $parityHost

if ($catcherTiles.Count -ne $plithTiles.Count) {
    throw "parity check: the catcher drew $($catcherTiles.Count) tile(s) and Plith's page drew $($plithTiles.Count)."
}
for ($i = 0; $i -lt $catcherTiles.Count; $i++) {
    if ($catcherTiles[$i] -ne $plithTiles[$i]) {
        throw "parity check: tile $i differs.`n  catcher: $($catcherTiles[$i])`n  plith:   $($plithTiles[$i])"
    }
}
"  parity check passed: $($catcherTiles.Count) tile(s) identical in position, size and brush"
```

- [ ] **Step 2: Run it to verify it fails before Task 3's page exists**

If Tasks 3 and 4 are done in order this check passes immediately, which proves nothing. Prove it can fail: temporarily change Plith's page to render an empty model, run the harness, and confirm the throw names the tile counts. Put the page back and rerun.

Run: `node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1`
Expected: FAIL with "the catcher drew 8 tile(s) and Plith's page drew 0", then PASS once reverted.

- [ ] **Step 3: Cover the empty store, which is Review Focus item 2**

Repeat the comparison with an empty store, so the empty state is proved identical too:

```powershell
$emptyStoreFile = Join-Path $OutDir 'shelf-store-empty.txt'
$emptyStore = [Plith.Services.Shelf.ShelfStore]::new($emptyStoreFile)
# ... build both sides against $emptyStore, lay out, and run the same comparison, then:
"  parity check passed: both sides draw the same empty state"
```

- [ ] **Step 4: Run both themes**

Run:
```
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/render-widgets.ps1 -Theme Light
```
Expected: `parity check passed` twice in each run, and `Done.`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(shelf): assert the two pictures are one picture

- the design rests on Plith's page and the catcher's window being
  identical, and nothing else can see a divergence
- covers the filled and the empty store, both themes"
```

---

### Task 5: Prove it on hardware, and measure what the page now costs

**Files:**
- Modify: `docs/SHELF-VERIFICATION.md`
- Modify: `docs/PERF-VERIFICATION.md`
- Modify: `docs/ROADMAP.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the record. No code.

- [ ] **Step 1: Measure the page turn's cost, which is Review Focus item 5**

The page now extracts shell icons on Plith's UI thread the first time it is shown. Run the notch-open and page-turn instruments and compare against the figures already in `PERF-VERIFICATION.md`:

```
node ~/.claude/bin/heavy-run.mjs -- pwsh -File scripts/measure-notch-open.ps1
```

Expected: no new blocking phase. If the first turn onto the shelf costs materially more than the other four, warm the page the way `ShelfWindow.Warm()` warms the catcher's window and measure again.

- [ ] **Step 2: Look at it, at the CONSOLE, not over Remote Desktop**

`SystemParameters.ClientAreaAnimation` is false in a remote session, so no page slides at all there and this check would measure a code path the product does not use. At the console: open the notch, page round onto the shelf, and watch the moment of the handover. The question is one question: does anything change on screen when the catcher's window arrives?

- [ ] **Step 3: Write the record**

Add a section to `docs/SHELF-VERIFICATION.md` saying which half was measured and which was judged by eye, in this document's usual terms. State plainly if the symptom survives. Update `docs/PERF-VERIFICATION.md` 13.4's open question if the capture answered the reported size change, and say so if it did not.

- [ ] **Step 4: Update the roadmap**

Mark the item in `docs/ROADMAP.md` with its actual duration beside the estimate, and move the finished plan to `docs/superpowers/plans/done/` with the honest banner:

> A ticked box means the step was taken, not that nothing deviated.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs(shelf): record the one-surface result

- what was measured on hardware and what was judged by eye
- whether the reported seam survives"
```
