# The Notch in Loft Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the notch off its accent-tinted surface onto praxvon-ui's Loft direction, so the accent becomes a mark rather than a flood and the surface is a fixed floor.

**Architecture:** The surface colour stops being derived from the user's accent and becomes two fixed Loft floors (dark and light). The accent derives instead into four roles by an OKLCH tone walk held to contrast ratios, replacing the HSL walk that exists for the old model. Because the floor is now known at compile time, five palette keys that only existed to cope with an unpredictable surface become constants and are deleted. Typography and an ornament layer follow on top.

**Tech Stack:** WPF, .NET 10, xunit, `ResourceDictionary` overrides built in `ThemeService`, PowerShell render and contrast harnesses under `scripts/`.

**Spec:** `docs/superpowers/specs/2026-10-12-notch-loft-design.md`

## Global Constraints

- All code, comments, identifiers, test names and commit messages in English. Only user-facing strings may be Turkish.
- Conventional Commits, subject max 50 characters, body as bullets, no AI attribution anywhere.
- Never use the em dash in code, comments, docs or commit messages.
- Loft dark floor: `#0B0B0A` (background) and `#151412` (surface). Loft light floor: `#E7E6E0` (background) and `#F3F2ED` (surface). Copied verbatim from `praxvon-ui/tokens/themes/praxvon.json`.
- `accent-mark` must clear **3:1** against every surface it is drawn on. `accent-text` must clear **4.5:1**. `accent-foreground` must clear **4.5:1** on the accent fill. From ADR 0014.
- The notch keeps `CornerRadius(0, 0, radius, radius)` with radius 8. This is the one deliberate departure from Loft and must not be "fixed" to square.
- `OsdPalette.HighContrast.xaml` is not touched. Ornament tokens resolve to their off values there.
- Fonts are Public Sans (body) and Martian Mono (labels and values), both OFL, both shipped with the app and attributed in `NOTICE.md`.
- Nothing is claimed to work until it has been rendered and measured. A green build does not count.

## Review Focus

Input classes the spec implies but no task's own tests would otherwise exercise. Each line names the test that pins it and the task that owns it.

1. **A custom accent whose hue cannot reach 3:1 at any lightness.** A near-black or near-white seed walked in OKLCH can run out of room. Expected: the derivation returns the closest passing tone it found and never returns a failing one. Pinned in Task 2, `DeriveLoftRoles_ExtremeSeedsStillPassTheirRatios`.
2. **The light floor and the dark floor disagreeing about one accent.** A tone that passes on `#0B0B0A` can fail on `#E7E6E0`. Expected: `accent-mark` and `accent-text` are derived per mode, and each is held against its own floor. Pinned in Task 2, `DeriveLoftRoles_HoldsRatiosOnBothFloors`.
3. **A theme swap while the notch is open.** `ThemeService` rebuilds the override dictionary live. Expected: no key disappears mid-swap, so no binding falls back to an unstyled default. Pinned in Task 3, `BuildAccentOverride_KeySetIsStableAcrossThemesAndAccents`.
4. **High contrast reaching the Loft code path.** `BuildAccentOverride` returns an empty dictionary when `IsHighContrast`, and the ornament tokens must resolve to off rather than to null. Pinned in Task 5, `Ornaments_ResolveToOffUnderHighContrast`.
5. **A missing font file at runtime.** A packaged font that fails to load silently falls back to a system face, and tabular figures go with it. Expected: the font resource names a fallback chain so text never disappears. Pinned in Task 4, `Typography_FontFamiliesNameAFallbackChain`.

---

### Task 1: OKLCH conversion, with a tone walk

**Files:**
- Create: `src/Plith/Services/Oklch.cs`
- Test: `tests/Plith.Tests/OklchTests.cs`

**Interfaces:**
- Consumes: `ContrastInk.ContrastRatio(Color, Color)`, which already exists.
- Produces: `Oklch.FromColor(Color) -> (double L, double C, double H)`, `Oklch.ToColor(double L, double C, double H) -> Color`, and `Oklch.Walk(Color seed, Func<Color, bool> passes, int direction) -> Color`. `direction` is `+1` to lighten and `-1` to darken. `Walk` returns the first tone that passes, or the endpoint tone it reached if none did.

Why OKLCH rather than the HSL already in `AccentTheme`: HSL lightness is not perceptual, so a walk in it changes apparent chroma as it moves and two accents of the same HSL lightness read as different brightnesses. praxvon-ui settled this in `packages/cli/src/accent.ts`; this is the same math so the two can be compared.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class OklchTests
{
    [Theory]
    [InlineData(0xCA, 0xFF, 0x33)]  // lime, the praxvon seed
    [InlineData(0x0B, 0x0B, 0x0A)]  // the dark floor
    [InlineData(0xE7, 0xE6, 0xE0)]  // the light floor
    [InlineData(0xFF, 0xFF, 0xFF)]
    [InlineData(0x00, 0x00, 0x00)]
    public void RoundTrip_PreservesTheColour(byte r, byte g, byte b)
    {
        var start = Color.FromRgb(r, g, b);
        var (l, c, h) = Oklch.FromColor(start);
        var back = Oklch.ToColor(l, c, h);

        // One step per channel: the conversion runs through floating point and back to bytes.
        Assert.InRange(back.R, start.R - 1, start.R + 1);
        Assert.InRange(back.G, start.G - 1, start.G + 1);
        Assert.InRange(back.B, start.B - 1, start.B + 1);
    }

    [Fact]
    public void Walk_LightensUntilThePredicatePasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);  // prussian, too dark on the dark floor

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);
    }

    [Fact]
    public void Walk_KeepsHue()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);
        var (_, _, seedHue) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);
        var (_, _, walkedHue) = Oklch.FromColor(walked);

        // Two degrees of drift is the gamut clamp, not a hue change.
        Assert.InRange(Math.Abs(walkedHue - seedHue), 0.0, 2.0);
    }

    [Fact]
    public void Walk_ReturnsTheEndpointWhenNothingPasses()
    {
        var impossible = Color.FromRgb(0x80, 0x80, 0x80);

        // No tone of any colour clears 21:1 against mid grey.
        var walked = Oklch.Walk(impossible, _ => false, direction: 1);

        Assert.NotEqual(default, walked);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OklchTests`
Expected: FAIL, `The type or namespace name 'Oklch' could not be found`.

- [ ] **Step 3: Write minimal implementation**

```csharp
using System.Windows.Media;

namespace Plith.Services;

/// <summary>
/// OKLab/OKLCH conversion and a lightness walk, ported from praxvon-ui's
/// `packages/cli/src/accent.ts` so the two derive the same roles from the same seed.
///
/// Why not the HSL already in <see cref="AccentTheme"/>: HSL lightness is not perceptual. A walk
/// in it changes apparent chroma as it moves, and two accents at the same HSL lightness read as
/// different brightnesses, so a ratio held in HSL holds by luck rather than by construction.
/// </summary>
public static class Oklch
{
    private const double Epsilon = 1e-9;

    public static (double L, double C, double H) FromColor(Color c)
    {
        var r = ToLinear(c.R / 255.0);
        var g = ToLinear(c.G / 255.0);
        var b = ToLinear(c.B / 255.0);

        var l = Math.Cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
        var m = Math.Cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
        var s = Math.Cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);

        var okL = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        var okA = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        var okB = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;

        var chroma = Math.Sqrt(okA * okA + okB * okB);
        var hue = chroma < Epsilon ? 0.0 : Math.Atan2(okB, okA) * 180.0 / Math.PI;
        if (hue < 0) hue += 360.0;

        return (okL, chroma, hue);
    }

    public static Color ToColor(double okL, double chroma, double hue)
    {
        var rad = hue * Math.PI / 180.0;
        var okA = chroma * Math.Cos(rad);
        var okB = chroma * Math.Sin(rad);

        var l = okL + 0.3963377774 * okA + 0.2158037573 * okB;
        var m = okL - 0.1055613458 * okA - 0.0638541728 * okB;
        var s = okL - 0.0894841775 * okA - 1.2914855480 * okB;

        l = l * l * l; m = m * m * m; s = s * s * s;

        var r = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        var g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        var b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

        return Color.FromRgb(ToByte(r), ToByte(g), ToByte(b));
    }

    /// <summary>
    /// Step the seed's lightness in <paramref name="direction"/> until
    /// <paramref name="passes"/> accepts the result. Hue and chroma are held, so the walk changes
    /// how light a colour is and not which colour it is.
    ///
    /// Returns the endpoint tone when nothing passes, rather than the seed: a caller asking for a
    /// ratio gets the closest this hue can come to it, and the decision about what to do with a
    /// colour that cannot reach its ratio belongs to the caller, not to the walk.
    /// </summary>
    public static Color Walk(Color seed, Func<Color, bool> passes, int direction)
    {
        ArgumentNullException.ThrowIfNull(passes);
        if (passes(seed)) return seed;

        var (l, c, h) = FromColor(seed);
        const double Step = 0.01;

        var last = seed;
        for (var i = 1; i <= 100; i++)
        {
            var next = l + direction * Step * i;
            if (next < 0.0 || next > 1.0) break;
            last = ToColor(next, c, h);
            if (passes(last)) return last;
        }
        return last;
    }

    private static double ToLinear(double channel) =>
        channel <= 0.04045 ? channel / 12.92 : Math.Pow((channel + 0.055) / 1.055, 2.4);

    private static byte ToByte(double linear)
    {
        var srgb = linear <= 0.0031308 ? linear * 12.92 : 1.055 * Math.Pow(linear, 1.0 / 2.4) - 0.055;
        return (byte)Math.Clamp(Math.Round(srgb * 255.0), 0, 255);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OklchTests`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Services/Oklch.cs tests/Plith.Tests/OklchTests.cs
git commit -m "feat(theme): add OKLCH conversion and a tone walk"
```

---

### Task 2: Derive the four Loft accent roles

**Files:**
- Create: `src/Plith/Services/LoftPalette.cs`
- Create: `scripts/build-loft-tokens.ps1`
- Modify: `src/Plith/Services/AccentTheme.cs` (add `DeriveLoftRoles`; leave `Derive` and `DeriveOsdSurfaces` in place for now, Task 3 removes the second)
- Test: `tests/Plith.Tests/LoftAccentTests.cs`

**Interfaces:**
- Consumes: `Oklch.Walk`, `ContrastInk.ContrastRatio`.
- Produces: `LoftPalette.DarkFloor` and `LoftPalette.LightFloor`, each a `LoftFloor(Color Background, Color Surface, Color Foreground, Color TextSecondary, Color TextTertiary, Color Border, Color BorderStrong)`; and `AccentTheme.DeriveLoftRoles(Color seed, bool isDark) -> LoftAccentRoles(Color Fill, Color OnFill, Color Mark, Color Text)`.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class LoftAccentTests
{
    public static TheoryData<Color> Seeds() => new()
    {
        Color.FromRgb(0xCA, 0xFF, 0x33),  // lime
        Color.FromRgb(0xC2, 0x40, 0x1A),  // red-lead
        Color.FromRgb(0x1D, 0x4E, 0x89),  // prussian
        Color.FromRgb(0x16, 0x16, 0x16),  // ink
        Color.FromRgb(0x25, 0x63, 0xEB),  // blue
        Color.FromRgb(0x6D, 0x28, 0xD9),  // violet
        Color.FromRgb(0x4A, 0xD6, 0x95),  // emerald, Plith's historical default
    };

    [Theory]
    [MemberData(nameof(Seeds))]
    public void DeriveLoftRoles_HoldsRatiosOnBothFloors(Color seed)
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = isDark ? LoftPalette.DarkFloor : LoftPalette.LightFloor;
            var roles = AccentTheme.DeriveLoftRoles(seed, isDark);

            foreach (var surface in new[] { floor.Background, floor.Surface })
            {
                Assert.True(ContrastInk.ContrastRatio(roles.Mark, surface) >= 3.0,
                    $"mark {roles.Mark} on {surface} (isDark={isDark})");
                Assert.True(ContrastInk.ContrastRatio(roles.Text, surface) >= 4.5,
                    $"text {roles.Text} on {surface} (isDark={isDark})");
            }

            Assert.True(ContrastInk.ContrastRatio(roles.OnFill, roles.Fill) >= 4.5,
                $"on-fill {roles.OnFill} on fill {roles.Fill} (isDark={isDark})");
        }
    }

    [Theory]
    [InlineData(0x00, 0x00, 0x00)]
    [InlineData(0xFF, 0xFF, 0xFF)]
    [InlineData(0x01, 0x00, 0x01)]
    public void DeriveLoftRoles_ExtremeSeedsStillPassTheirRatios(byte r, byte g, byte b)
    {
        var seed = Color.FromRgb(r, g, b);

        foreach (var isDark in new[] { true, false })
        {
            var floor = isDark ? LoftPalette.DarkFloor : LoftPalette.LightFloor;
            var roles = AccentTheme.DeriveLoftRoles(seed, isDark);

            Assert.True(ContrastInk.ContrastRatio(roles.Mark, floor.Surface) >= 3.0);
            Assert.True(ContrastInk.ContrastRatio(roles.Text, floor.Surface) >= 4.5);
        }
    }

    [Fact]
    public void DeriveLoftRoles_KeepsTheSeedAsFillWhenItAlreadyPasses()
    {
        // Lime on the paper floor is the case ADR 0014 names: the fill keeps the seed when it can.
        var lime = Color.FromRgb(0xCA, 0xFF, 0x33);
        var roles = AccentTheme.DeriveLoftRoles(lime, isDark: false);

        Assert.Equal(lime, roles.Fill);
    }

    [Fact]
    public void DarkFloor_MatchesTheSpec()
    {
        Assert.Equal(Color.FromRgb(0x0B, 0x0B, 0x0A), LoftPalette.DarkFloor.Background);
        Assert.Equal(Color.FromRgb(0x15, 0x14, 0x12), LoftPalette.DarkFloor.Surface);
        Assert.Equal(Color.FromRgb(0xE7, 0xE6, 0xE0), LoftPalette.LightFloor.Background);
        Assert.Equal(Color.FromRgb(0xF3, 0xF2, 0xED), LoftPalette.LightFloor.Surface);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests`
Expected: FAIL, `The type or namespace name 'LoftPalette' could not be found`.

- [ ] **Step 3: Write minimal implementation**

Create `src/Plith/Services/LoftPalette.cs`:

```csharp
using System.Windows.Media;

namespace Plith.Services;

/// <summary>One mode's floor, from praxvon-ui `tokens/themes/praxvon.json`.</summary>
public sealed record LoftFloor(
    Color Background,
    Color Surface,
    Color Foreground,
    Color TextSecondary,
    Color TextTertiary,
    Color Border,
    Color BorderStrong);

/// <summary>The four roles one accent seed derives into, per ADR 0014.</summary>
public sealed record LoftAccentRoles(Color Fill, Color OnFill, Color Mark, Color Text);

/// <summary>
/// Loft's fixed floors. The surface no longer moves with the user's accent, which is the whole
/// change: ink against a known floor is a constant, and the five palette keys that existed to
/// compute ink against an unknown one are gone.
/// </summary>
public static class LoftPalette
{
    public static readonly LoftFloor DarkFloor = new(
        Background:    Rgb(0x0B, 0x0B, 0x0A),
        Surface:       Rgb(0x15, 0x14, 0x12),
        Foreground:    Rgb(0xEC, 0xE9, 0xE2),
        TextSecondary: Rgb(0xA3, 0x9E, 0x94),
        TextTertiary:  Rgb(0x7A, 0x75, 0x6C),
        Border:        Rgb(0x2B, 0x28, 0x24),
        BorderStrong:  Rgb(0x7A, 0x75, 0x6C));

    public static readonly LoftFloor LightFloor = new(
        Background:    Rgb(0xE7, 0xE6, 0xE0),
        Surface:       Rgb(0xF3, 0xF2, 0xED),
        Foreground:    Rgb(0x16, 0x16, 0x16),
        TextSecondary: Rgb(0x55, 0x54, 0x4F),
        TextTertiary:  Rgb(0x77, 0x75, 0x6D),
        Border:        Rgb(0xD0, 0xCE, 0xC6),
        BorderStrong:  Rgb(0x77, 0x75, 0x6D));

    public static LoftFloor For(bool isDark) => isDark ? DarkFloor : LightFloor;

    private static Color Rgb(byte r, byte g, byte b) => Color.FromRgb(r, g, b);
}
```

Add to `src/Plith/Services/AccentTheme.cs`, inside `public static class AccentTheme`:

```csharp
    /// <summary>
    /// One seed into the four Loft roles, per ADR 0014. The fill keeps the seed when it can;
    /// the mark and the text walk in OKLCH, hue and chroma held, until each clears its ratio
    /// against BOTH of the mode's surfaces rather than against one of them. A tone that passes on
    /// the background and fails on the raised surface is a tone that fails, because the notch
    /// draws on both.
    /// </summary>
    public static LoftAccentRoles DeriveLoftRoles(Color seed, bool isDark)
    {
        var floor = LoftPalette.For(isDark);
        var surfaces = new[] { floor.Background, floor.Surface };

        double MinOn(Color c) => surfaces.Min(s => ContrastInk.ContrastRatio(c, s));

        // The fill is the seed unless its own label cannot sit on it. Walking away from the floor
        // is the direction that gains contrast: lighter on a dark floor, darker on a light one.
        var away = isDark ? 1 : -1;

        var fill = seed;
        var onFill = ContrastInk.On(fill);
        if (ContrastInk.ContrastRatio(onFill, fill) < 4.5)
        {
            fill = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(ContrastInk.On(c), c) >= 4.5, away);
            onFill = ContrastInk.On(fill);
        }

        var mark = MinOn(seed) >= 3.0 ? seed : Oklch.Walk(seed, c => MinOn(c) >= 3.0, away);
        var text = MinOn(seed) >= 4.5 ? seed : Oklch.Walk(seed, c => MinOn(c) >= 4.5, away);

        return new LoftAccentRoles(fill, onFill, mark, text);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests`
Expected: PASS. If an extreme seed fails, the walk ran out of range rather than the math being wrong: fix by walking the other direction when the first endpoint fails, not by lowering a ratio.

- [ ] **Step 5: Make the floors generated rather than copied**

The spec asks for this and the reason is specific: a hand copy of fourteen colours drifts the first
time either side is tuned, and the drift is silent because both sides still compile. The floors
stay checked in, so a build never needs the praxvon-ui checkout; the script is what proves they
still match, and `DarkFloor_MatchesTheSpec` is what fails when they do not.

Create `scripts/build-loft-tokens.ps1`:

```powershell
# Regenerates the floor constants in src/Plith/Services/LoftPalette.cs from praxvon-ui's
# theme tokens, the sibling of that repo's own `build-tokens`.
#
# The generated values are checked in on purpose: a Plith build must not need the praxvon-ui
# checkout. This script is run by a person when praxvon-ui's theme moves, and its output is
# reviewed like any other change. LoftAccentTests.DarkFloor_MatchesTheSpec is the guard that
# fails if someone edits the constants by hand instead.

[CmdletBinding()]
param(
    [string]$PraxvonUi = 'C:\Projects\praxvon-ui',
    [string]$Target = (Join-Path $PSScriptRoot '..\src\Plith\Services\LoftPalette.cs')
)

$ErrorActionPreference = 'Stop'

$themePath = Join-Path $PraxvonUi 'tokens\themes\praxvon.json'
if (-not (Test-Path $themePath)) { throw "praxvon theme not found at $themePath" }

$theme = Get-Content $themePath -Raw | ConvertFrom-Json

function Emit([object]$mode, [string]$name) {
    $fields = 'background', 'surface', 'foreground', 'text-secondary', 'text-tertiary', 'border', 'border-strong'
    $names  = 'Background', 'Surface', 'Foreground', 'TextSecondary', 'TextTertiary', 'Border', 'BorderStrong'

    $lines = for ($i = 0; $i -lt $fields.Count; $i++) {
        $hex = $mode.($fields[$i])
        if ($hex -notmatch '^#[0-9A-Fa-f]{6}$') { throw "$name.$($fields[$i]) is not a 6 digit hex: $hex" }
        $r = "0x$($hex.Substring(1,2))"; $g = "0x$($hex.Substring(3,2))"; $b = "0x$($hex.Substring(5,2))"
        "        {0,-14} Rgb({1}, {2}, {3})," -f "$($names[$i]):", $r, $g, $b
    }
    $lines -join "`r`n"
}

$dark  = Emit $theme.dark  'dark'
$light = Emit $theme.light 'light'

Write-Host "dark floor:"  -ForegroundColor Cyan; Write-Host $dark
Write-Host "light floor:" -ForegroundColor Cyan; Write-Host $light
Write-Host ""
Write-Host "Paste these into $Target, replacing the two initialiser blocks, then run:" -ForegroundColor Yellow
Write-Host "  dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests"
```

Run it and confirm its output matches what Step 3 wrote by hand:

Run: `pwsh -File scripts/build-loft-tokens.ps1`
Expected: the printed blocks are identical to the two initialisers in `LoftPalette.cs`. If they
differ, the hand-written constants are wrong and the script is right.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Services/LoftPalette.cs src/Plith/Services/AccentTheme.cs scripts/build-loft-tokens.ps1 tests/Plith.Tests/LoftAccentTests.cs
git commit -m "feat(theme): derive the four Loft accent roles"
```

---

### Task 3: Put the floor under the notch, and delete what it replaces

**Files:**
- Modify: `src/Plith/Services/ThemeService.cs:228-300` (`BuildAccentOverride`)
- Modify: `src/Plith/Services/AccentTheme.cs` (remove `DeriveOsdSurfaces` and the `OsdSurfaceDerived` record)
- Modify: `src/Plith/Resources/OsdPalette.Dark.xaml`, `OsdPalette.Light.xaml` (surface brushes become the Loft floors)
- Test: `tests/Plith.Tests/ThemeServiceLoftTests.cs`

**Interfaces:**
- Consumes: `AccentTheme.DeriveLoftRoles`, `LoftPalette.For`.
- Produces: the override dictionary's key set, which the render harness and `OsdHost.RefreshAccentMirror` both mirror. Keys after this task: `AccentBrush`, `AccentHover`, `AccentPressed`, `AccentGlow`, `OsdAccent`, `OsdAccentMark`, `OsdAccentText`, `OsdSurfaceBrush`, `OsdBorder`, `OsdTrackBg`, `OsdDivider`, `OnAccent`.

Removed here, with the reason in one line each: `NotchInk`, `NotchInkMuted`, `NotchTrack` were computed from `osd.SurfaceEnd` because the surface moved; the floor's `Foreground`, `TextSecondary` and a fixed track now say the same thing without a computation. `AccentTrack` was derived from the accent because a progress fill had to read against its own track on an unpredictable panel; on a fixed floor the track is `Border`.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows;
using System.Windows.Media;
using Plith.Services;

namespace Plith.Tests;

public class ThemeServiceLoftTests
{
    private static readonly Color[] Accents =
    [
        Color.FromRgb(0xCA, 0xFF, 0x33),
        Color.FromRgb(0x6D, 0x28, 0xD9),
        Color.FromRgb(0x4A, 0xD6, 0x95),
    ];

    [Fact]
    public void BuildAccentOverride_SurfaceIsTheFloorAndNotTheAccent()
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = LoftPalette.For(isDark);

            foreach (var accent in Accents)
            {
                var dict = ThemeService.BuildAccentOverride(accent, isDark);
                var brush = Assert.IsType<LinearGradientBrush>(dict["OsdSurfaceBrush"]);

                Assert.Equal(floor.Background, brush.GradientStops[0].Color);
                Assert.Equal(floor.Surface, brush.GradientStops[1].Color);
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_KeySetIsStableAcrossThemesAndAccents()
    {
        var expected = ThemeService.BuildAccentOverride(Accents[0], isDark: true).Keys
            .Cast<object>().Select(k => k.ToString()).OrderBy(k => k).ToArray();

        foreach (var isDark in new[] { true, false })
        {
            foreach (var accent in Accents)
            {
                var keys = ThemeService.BuildAccentOverride(accent, isDark).Keys
                    .Cast<object>().Select(k => k.ToString()).OrderBy(k => k).ToArray();

                Assert.Equal(expected, keys);
            }
        }
    }

    [Fact]
    public void BuildAccentOverride_DropsTheKeysTheFixedFloorMakesConstant()
    {
        var dict = ThemeService.BuildAccentOverride(Accents[0], isDark: true);

        Assert.False(dict.Contains("NotchInk"));
        Assert.False(dict.Contains("NotchInkMuted"));
        Assert.False(dict.Contains("NotchTrack"));
        Assert.False(dict.Contains("AccentTrack"));
    }

    [Fact]
    public void BuildAccentOverride_CarriesTheMarkAndTextRoles()
    {
        foreach (var isDark in new[] { true, false })
        {
            var floor = LoftPalette.For(isDark);

            foreach (var accent in Accents)
            {
                var dict = ThemeService.BuildAccentOverride(accent, isDark);
                var mark = Assert.IsType<SolidColorBrush>(dict["OsdAccentMark"]);
                var text = Assert.IsType<SolidColorBrush>(dict["OsdAccentText"]);

                Assert.True(ContrastInk.ContrastRatio(mark.Color, floor.Surface) >= 3.0);
                Assert.True(ContrastInk.ContrastRatio(text.Color, floor.Surface) >= 4.5);
            }
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~ThemeServiceLoftTests`
Expected: FAIL. `OsdSurfaceBrush` still carries the accent-derived gradient and `OsdAccentMark` does not exist.

- [ ] **Step 3: Write minimal implementation**

In `ThemeService.cs`, replace the body of `public static ResourceDictionary BuildAccentOverride(Color baseColor, bool isDark)`:

```csharp
    public static ResourceDictionary BuildAccentOverride(Color baseColor, bool isDark)
    {
        var derived = AccentTheme.Derive(baseColor, isDark);
        var roles = AccentTheme.DeriveLoftRoles(baseColor, isDark);
        var floor = LoftPalette.For(isDark);

        return new ResourceDictionary
        {
            [KeyAccent]        = FrozenBrush(derived.Accent),
            [KeyAccentHover]   = FrozenBrush(derived.Hover),
            [KeyAccentPressed] = FrozenBrush(derived.Pressed),
            [KeyAccentGlow]    = FrozenBrush(derived.Glow),

            // The fill, for filled parts only: a primary button, a checked box. Never a panel.
            [KeyOsdAccent]     = FrozenBrush(roles.Fill),
            [KeyOnAccent]      = FrozenBrush(roles.OnFill),

            // Lines, borders and markers take the mark; accent-coloured text takes the text role.
            // ADR 0014: a surface is never filled with the accent, so there is no third option.
            [KeyOsdAccentMark] = FrozenBrush(roles.Mark),
            [KeyOsdAccentText] = FrozenBrush(roles.Text),

            // The floor. Two stops so the panel keeps the vertical modelling the tinted gradient
            // had, and both stops are fixed, which is the point of the change.
            [KeyOsdSurface]    = BuildFloorBrush(floor),
            [KeyOsdBorder]     = FrozenBrush(floor.Border),
            [KeyOsdTrackBg]    = FrozenBrush(WithAlpha(floor.Border, OsdTrackAlpha)),
            [KeyOsdDivider]    = FrozenBrush(WithAlpha(floor.Border, OsdDividerAlpha)),
        };
    }

    private static LinearGradientBrush BuildFloorBrush(LoftFloor floor)
    {
        var brush = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = new Point(0, 1),
        };
        brush.GradientStops.Add(new GradientStop(floor.Background, 0));
        brush.GradientStops.Add(new GradientStop(floor.Surface, 1));
        brush.Freeze();
        return brush;
    }
```

Add the two key constants beside the existing ones:

```csharp
    private const string KeyOsdAccentMark = "OsdAccentMark";
    private const string KeyOsdAccentText = "OsdAccentText";
```

Delete `BuildOsdSurfaceBrush`, `AccentTheme.DeriveOsdSurfaces`, the `OsdSurfaceDerived` record, and the `KeyNotchInk`, `KeyNotchInkMuted`, `KeyNotchTrack`, `KeyAccentTrack` constants. The compiler will name every remaining consumer; point each at the floor's own value (`NotchInk` becomes `floor.Foreground`, `NotchInkMuted` becomes `floor.TextSecondary`, `NotchTrack` and `AccentTrack` become `floor.Border`), declared in `OsdPalette.Dark.xaml` and `OsdPalette.Light.xaml` as static brushes rather than rebuilt per accent.

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS, the whole suite. Existing tests that asserted the old tinted surface will fail and must be rewritten to assert the floor, not deleted: each one is a recorded decision and the rewrite says which way it went.

- [ ] **Step 5: Look at it**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-3 -Theme Dark -Accent '#6D28D9'`
Then: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-3-light -Theme Light -Accent '#6D28D9'`
Expected: the panel is the floor in both, violet appears only in lines and text. Open `frame-page1.png` and `widget-media.png` and confirm with your eyes. A passing test here means the numbers are right; only the render shows whether the thing is right.

- [ ] **Step 6: Run the contrast harness**

Run: `pwsh -File scripts/check-contrast.ps1`
Expected: PASS on both themes and every accent.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Services/ThemeService.cs src/Plith/Services/AccentTheme.cs src/Plith/Resources/OsdPalette.Dark.xaml src/Plith/Resources/OsdPalette.Light.xaml tests/Plith.Tests/
git commit -m "feat(theme): put the Loft floor under the notch"
```

---

### Task 4: Typography

**Files:**
- Create: `src/Plith/Assets/Fonts/PublicSans-Regular.ttf`, `PublicSans-SemiBold.ttf`, `MartianMono-Regular.ttf` (download from the upstream OFL releases)
- Modify: `src/Plith/Plith.csproj` (embed the fonts as `Resource`)
- Modify: `src/Plith/Resources/Theme.xaml:8` (`OsdUiFont`, plus a new `OsdMonoFont`)
- Modify: `src/Plith/Views/AmbientCardView.xaml`, `MediaCardView.xaml`, `AudioCardView.xaml`, `BrightnessCardView.xaml` (tabular figures on every changing number)
- Modify: `NOTICE.md`
- Test: `tests/Plith.Tests/TypographyTests.cs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: resource keys `OsdUiFont` and `OsdMonoFont`, both `FontFamily`.

Tabular figures in WPF are a typography feature on the run, not a property of the family: `<Run Typography.NumeralAlignment="Tabular">` or `Typography.NumeralAlignment="Tabular"` on the `TextBlock`. Without it the clock's digits change width as they tick and the whole line shifts, which is the defect this fixes.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows;
using System.Windows.Media;

namespace Plith.Tests;

public class TypographyTests
{
    private static ResourceDictionary LoadTheme() =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri("/Plith;component/Resources/Theme.xaml", UriKind.Relative));

    [StaFact]
    public void Typography_FontFamiliesNameAFallbackChain()
    {
        var theme = LoadTheme();

        foreach (var key in new[] { "OsdUiFont", "OsdMonoFont" })
        {
            var family = Assert.IsType<FontFamily>(theme[key]);

            // A packaged font that fails to load falls back silently, and the fallback must be a
            // face that exists on every Windows 11 machine rather than nothing.
            Assert.Contains(",", family.Source);
        }
    }

    [StaFact]
    public void Typography_MonoFontIsNotTheUiFont()
    {
        var theme = LoadTheme();
        var ui = (FontFamily)theme["OsdUiFont"];
        var mono = (FontFamily)theme["OsdMonoFont"];

        Assert.NotEqual(ui.Source, mono.Source);
    }
}
```

`[StaFact]` comes from `xunit.StaFact`; if the test project does not already reference it, add it in this step, because WPF types cannot be constructed on the MTA threads xunit uses by default.

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~TypographyTests`
Expected: FAIL, `OsdMonoFont` is not a key in the dictionary.

- [ ] **Step 3: Write minimal implementation**

In `src/Plith/Resources/Theme.xaml`, replace line 8 and add the mono family:

```xml
    <FontFamily x:Key="OsdUiFont">pack://application:,,,/Assets/Fonts/#Public Sans, Segoe UI Variable, Segoe UI</FontFamily>
    <FontFamily x:Key="OsdMonoFont">pack://application:,,,/Assets/Fonts/#Martian Mono, Cascadia Mono, Consolas</FontFamily>
```

In `src/Plith/Plith.csproj`:

```xml
  <ItemGroup>
    <Resource Include="Assets\Fonts\*.ttf" />
  </ItemGroup>
```

In `NOTICE.md`, append:

```markdown
## Fonts

- **Public Sans**, by the United States Web Design System. SIL Open Font License 1.1.
- **Martian Mono**, by Evil Martian. SIL Open Font License 1.1.

Both are redistributed with Plith under the OFL, which permits bundling. Neither is modified.
```

On every `TextBlock` that shows a number which changes in place, add `Typography.NumeralAlignment="Tabular"`: the clock and date in `AmbientCardView.xaml`, the elapsed and remaining times in `MediaCardView.xaml`, the percentage in `AudioCardView.xaml` and `BrightnessCardView.xaml`, and the temperature in the weather widget.

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~TypographyTests`
Expected: PASS.

- [ ] **Step 5: Look at it, twice**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-4`
Expected: the fonts actually loaded. A failed pack URI falls back to Segoe UI and renders perfectly, so compare `widget-clock.png` against Task 3's output and confirm the face changed. If it did not, the pack URI family name is wrong: the `#` name is the font's internal family name, not the file name.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Assets/Fonts src/Plith/Plith.csproj src/Plith/Resources/Theme.xaml src/Plith/Views NOTICE.md tests/Plith.Tests/TypographyTests.cs
git commit -m "feat(theme): ship Loft's fonts, tabular figures"
```

---

### Task 5: The ornament layer

**Files:**
- Create: `src/Plith/Resources/Ornaments.xaml`
- Modify: `src/Plith/Resources/OsdPalette.Dark.xaml`, `OsdPalette.Light.xaml`, `OsdPalette.HighContrast.xaml` (ornament token values, off in high contrast)
- Modify: `src/Plith/Views/OsdContent.xaml` (corner marks on the notch panel)
- Modify: `src/Plith/Views/AudioCardView.xaml`, `BrightnessCardView.xaml` (ticks under the bar)
- Modify: `src/Plith/Views/MediaCardView.xaml` (the output picker's selected row takes brackets)
- Test: `tests/Plith.Tests/OrnamentTests.cs`

**Interfaces:**
- Consumes: `OsdAccentMark` and `OsdAccentText` from Task 3.
- Produces: resource keys `OrnamentCornerLength` (`double`), `OrnamentOutlineThickness` (`double`), `OrnamentLabelCase` (`string`, `"Upper"` or `"None"`), `OrnamentTickBrush` (`Brush`); and a `Style` keyed `LoftFrame` that draws the four corner marks.

Corner marks are drawn inside the notch's corner radius, not on it: the radius stays 8 per the spec and a `+` on a curve reads as a mistake.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows;

namespace Plith.Tests;

public class OrnamentTests
{
    private static ResourceDictionary Load(string name) =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri($"/Plith;component/Resources/{name}", UriKind.Relative));

    [StaFact]
    public void Ornaments_ResolveToOffUnderHighContrast()
    {
        var hc = Load("OsdPalette.HighContrast.xaml");

        // Off, not absent: a missing key throws at resolve time on a surface a user with high
        // contrast turned on is already relying on.
        Assert.Equal(0.0, (double)hc["OrnamentCornerLength"]);
        Assert.Equal(0.0, (double)hc["OrnamentOutlineThickness"]);
        Assert.Equal("None", (string)hc["OrnamentLabelCase"]);
    }

    [StaFact]
    public void Ornaments_AreOnForBothLoftThemes()
    {
        foreach (var name in new[] { "OsdPalette.Dark.xaml", "OsdPalette.Light.xaml" })
        {
            var palette = Load(name);

            Assert.Equal(9.0, (double)palette["OrnamentCornerLength"]);
            Assert.Equal(1.0, (double)palette["OrnamentOutlineThickness"]);
            Assert.Equal("Upper", (string)palette["OrnamentLabelCase"]);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OrnamentTests`
Expected: FAIL, `OrnamentCornerLength` is not a key.

- [ ] **Step 3: Write minimal implementation**

Add to each palette. `OsdPalette.Dark.xaml` and `OsdPalette.Light.xaml`:

```xml
    <!-- Loft's ornament layer, as tokens so a theme turns it off without touching a view.
         9 DIP is praxvon-ui's `ornament.corner`. -->
    <sys:Double x:Key="OrnamentCornerLength">9</sys:Double>
    <sys:Double x:Key="OrnamentOutlineThickness">1</sys:Double>
    <sys:String x:Key="OrnamentLabelCase">Upper</sys:String>
```

`OsdPalette.HighContrast.xaml` gets the same keys at `0`, `0` and `None`. Both files need `xmlns:sys="clr-namespace:System;assembly=System.Runtime"`.

Create `src/Plith/Resources/Ornaments.xaml` with the `LoftFrame` style: four `Path` elements, one per corner, each a two-segment `+` arm of `OrnamentCornerLength`, stroked with `OsdAccentMark` at `OrnamentOutlineThickness`, inset from the panel edge by the notch radius so the mark sits inside the curve.

In `MediaCardView.xaml`, the output picker's selected row is marked by `[` and `]` in
`OsdAccentText` on either side of its label, not by a filled pill or a tone change. ADR 0014
closed this explicitly: in Loft the tone is no longer the indicator, because a light fill at
1.14:1 against its track is not an indicator at all. The brackets are decoration and must not
reach the accessibility tree, so they are `TextBlock` siblings with
`AutomationProperties.IsOffscreenBehavior` left alone and
`AutomationProperties.AccessibilityView="Raw"`: the row's existing selected state is what a
screen reader announces, and `scripts/check-a11y.ps1` in Step 5 is what proves the names
survived.

Per `scripts/check-shared-xaml.ps1`, `Ornaments.xaml` must not name an assembly in any `clr-namespace` it declares: it is loaded by both Plith and Plith.DropCatcher, and naming `Plith` there is what shipped the broken 0.1.6 installer.

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OrnamentTests`
Expected: PASS.

- [ ] **Step 5: Run the shared XAML check**

Run: `pwsh -File scripts/check-shared-xaml.ps1`
Expected: PASS. This check exists because its failure mode is invisible to the build and to both test suites.

- [ ] **Step 6: Look at it**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-5`
Expected: corner marks present and inside the radius, ticks under the volume and brightness bars, labels uppercase in the mono face.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Resources src/Plith/Views tests/Plith.Tests/OrnamentTests.cs
git commit -m "feat(notch): add Loft's ornament layer"
```

---

### Task 6: Values settle on marks

**Files:**
- Modify: `src/Plith/Views/AudioCardView.xaml`, `BrightnessCardView.xaml`, `MediaCardView.xaml` (bar and thumb animation)
- Test: `tests/Plith.Tests/SettleTests.cs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: resource key `LoftSettleDuration` (`Duration`, 400 ms) and `LoftSettleEase` (`IEasingFunction`).

Loft's rule is that a value settles without overshoot, like a batten pinned by a duck. praxvon-ui pairs Loft with the `calm` personality, whose `spring.smooth` is `visualDuration 0.55, bounce 0`. WPF has no spring, so the equivalent is a `CubicEase` with `EasingMode.EaseOut` over 400 ms: bounce 0 means no overshoot, and an ease-out cubic is the closest standard curve. Reduced motion jumps, with no animation at all.

- [ ] **Step 1: Write the failing test**

```csharp
using System.Windows;
using System.Windows.Media.Animation;

namespace Plith.Tests;

public class SettleTests
{
    private static ResourceDictionary LoadTheme() =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri("/Plith;component/Resources/Theme.xaml", UriKind.Relative));

    [StaFact]
    public void Settle_NeverOvershoots()
    {
        var theme = LoadTheme();
        var ease = theme["LoftSettleEase"];

        // Bounce zero is the rule. Any easing that returns above 1.0 before the end overshoots,
        // which is exactly the batten springing past its duck.
        Assert.IsNotType<BounceEase>(ease);
        Assert.IsNotType<ElasticEase>(ease);
        Assert.IsNotType<BackEase>(ease);

        var function = Assert.IsAssignableFrom<IEasingFunction>(ease);
        for (var t = 0.0; t <= 1.0; t += 0.01)
        {
            Assert.InRange(function.Ease(t), -0.0001, 1.0001);
        }
    }

    [StaFact]
    public void Settle_UsesTheCalmDuration()
    {
        var theme = LoadTheme();
        var duration = Assert.IsType<Duration>(theme["LoftSettleDuration"]);

        Assert.Equal(TimeSpan.FromMilliseconds(400), duration.TimeSpan);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettleTests`
Expected: FAIL, `LoftSettleEase` is not a key.

- [ ] **Step 3: Write minimal implementation**

In `src/Plith/Resources/Theme.xaml`:

```xml
    <!-- Loft: a value settles on its mark without overshoot. praxvon-ui pairs Loft with the
         `calm` personality, whose smooth spring is bounce 0. WPF has no spring, and an ease-out
         cubic is the closest standard curve that cannot pass its target. -->
    <Duration x:Key="LoftSettleDuration">0:0:0.4</Duration>
    <CubicEase x:Key="LoftSettleEase" EasingMode="EaseOut" />
```

Point the volume bar, the brightness bar and the seek thumb at both keys, replacing whatever duration and easing they carry now. Where a view checks `SystemParameters.ClientAreaAnimation` or an equivalent reduced-motion flag, keep that branch and let it jump.

- [ ] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettleTests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Resources/Theme.xaml src/Plith/Views tests/Plith.Tests/SettleTests.cs
git commit -m "feat(notch): settle values on marks without overshoot"
```

---

### Task 7: Verify on a running build, and say which half was measured

**Files:**
- Modify: `docs/PHASE6-VERIFICATION.md` (a new section for this work)
- Modify: `docs/ROADMAP.md` (status)

No code. This task is the repo's own standing lesson: a green build, green tests and green lints cannot see a wrong shape, a wrong layout, or a window that is not on screen.

- [ ] **Step 1: Run every instrument, and record the output, not a summary of it**

```bash
pwsh -File scripts/check-shared-xaml.ps1
pwsh -File scripts/check-a11y.ps1
pwsh -File scripts/check-contrast.ps1
pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark -Accent '#CAFF33'
pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark -Accent '#6D28D9'
pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -Accent '#C2401A'
dotnet test tests/Plith.Tests/Plith.Tests.csproj
```

- [ ] **Step 2: Drive the real notch**

```bash
pwsh -File scripts/drive-media-page.ps1
```
Expected: unchanged pass. A Debug build runs at medium integrity, so UI Automation reads the tree and `SendInput` drives it.

- [ ] **Step 3: Measure that nothing got slower**

```bash
pwsh -File scripts/measure-notch-open.ps1
pwsh -File scripts/measure-idle-cost.ps1
```
Expected: no regression against the figures in `docs/PERF-VERIFICATION.md`, and no new timer or wakeup. The ornament layer is static geometry; if idle cost moved, something is animating that should not be.

- [ ] **Step 4: Look at every render with your own eyes**

Open each PNG from Step 1. The panel must be the floor at every accent. The accent must appear only in lines, marks and text. Corner marks must sit inside the radius. Numbers must not shift as they change.

- [ ] **Step 5: Write the verification section**

Add to `docs/PHASE6-VERIFICATION.md`: what was measured, what was not, and which claims are inherited rather than observed. Name the accents and themes actually rendered, not "all of them". If an instrument was skipped, say which and why.

- [ ] **Step 6: Commit**

```bash
git add docs/PHASE6-VERIFICATION.md docs/ROADMAP.md
git commit -m "docs(notch): record the Loft verification"
```
