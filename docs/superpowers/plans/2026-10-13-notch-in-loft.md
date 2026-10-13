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

**The endpoint is `L = 1.0` or `L = 0.0` exactly, and the walk must try it.** Stepping by 0.01 and
breaking when the next step would leave the range stops at 0.99, so white and black are never
tested, and `Walk` can return a failing tone while a passing one existed. The final step is
clamped to the endpoint and tried. This is load-bearing for Task 2, whose
`DeriveLoftRoles_ExtremeSeedsStillPassTheirRatios` pins "never returns a failing one" for seeds
that need the very end of the range. Found by review of the first implementation of this task,
which followed the code below literally; the code below is corrected.

Why OKLCH rather than the HSL already in `AccentTheme`: HSL lightness is not perceptual, so a walk in it changes apparent chroma as it moves and two accents of the same HSL lightness read as different brightnesses. praxvon-ui settled this in `packages/cli/src/accent.ts`; this is the same math so the two can be compared.

- [x] **Step 1: Write the failing test**

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

    /// <summary>
    /// Pins an absolute value, not just a round trip. A matrix pair that is wrong in a mutually
    /// consistent way survives RoundTrip_PreservesTheColour and is caught here, and these are the
    /// numbers to compare against praxvon-ui's `packages/cli/src/accent.ts`, which is the stated
    /// reason this conversion exists rather than the HSL one.
    /// </summary>
    [Theory]
    [InlineData(0xFF, 0xFF, 0xFF, 1.0000, 0.0000)]
    [InlineData(0x00, 0x00, 0x00, 0.0000, 0.0000)]
    [InlineData(0xFF, 0x00, 0x00, 0.6280, 0.2577)]
    public void FromColor_MatchesThePublishedOklabValues(byte r, byte g, byte b, double l, double c)
    {
        var (gotL, gotC, _) = Oklch.FromColor(Color.FromRgb(r, g, b));

        Assert.Equal(l, gotL, precision: 3);
        Assert.Equal(c, gotC, precision: 3);
    }

    [Fact]
    public void Walk_LightensUntilThePredicatePasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0x1D, 0x4E, 0x89);  // prussian, too dark on the dark floor
        var (seedL, _, _) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);
        var (walkedL, _, _) = Oklch.FromColor(walked);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);

        // It must have gone UP, and not all the way to white: a stub returning white also clears
        // the ratio, and "the first tone that passes" is the contract, not "any tone that passes".
        Assert.True(walkedL > seedL, $"expected a lighter tone, seed L {seedL}, walked L {walkedL}");
        Assert.True(walkedL < 1.0, $"walked all the way to white, L {walkedL}");
    }

    [Fact]
    public void Walk_DarkensWhenAskedTo()
    {
        var floor = Color.FromRgb(0xF3, 0xF2, 0xED);  // the light floor's surface
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);   // lime, far too pale on paper
        var (seedL, _, _) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: -1);
        var (walkedL, _, _) = Oklch.FromColor(walked);

        Assert.True(ContrastInk.ContrastRatio(walked, floor) >= 3.0);
        Assert.True(walkedL < seedL, $"expected a darker tone, seed L {seedL}, walked L {walkedL}");
    }

    [Fact]
    public void Walk_ReturnsTheSeedWhenItAlreadyPasses()
    {
        var floor = Color.FromRgb(0x0B, 0x0B, 0x0A);
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);  // lime already clears 3:1 on the dark floor

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 3.0, direction: 1);

        Assert.Equal(seed, walked);
    }

    /// <summary>
    /// Lime is the seed most likely to leave the sRGB gamut on a walk, because the walk holds
    /// chroma while lightness rises and ToByte then clamps each channel independently. Hue drift
    /// from that clamp is accepted; a hue CHANGE is not.
    /// </summary>
    [Fact]
    public void Walk_KeepsHueOnTheSeedMostLikelyToClip()
    {
        var floor = Color.FromRgb(0xF3, 0xF2, 0xED);
        var seed = Color.FromRgb(0xCA, 0xFF, 0x33);
        var (_, _, seedHue) = Oklch.FromColor(seed);

        var walked = Oklch.Walk(seed, c => ContrastInk.ContrastRatio(c, floor) >= 4.5, direction: -1);
        var (_, _, walkedHue) = Oklch.FromColor(walked);

        // 5.197 degrees measured for this seed. The bound is 8.0 rather than hugging the
        // measurement, because what this test must catch is a hue CHANGE, tens or hundreds of
        // degrees from a transposed matrix row, not the few degrees the gamut clamp costs. A
        // bound sitting 0.3 above the measurement is a tripwire for any harmless change to the
        // step grid.
        Assert.InRange(HueDistance(walkedHue, seedHue), 0.0, 8.0);
    }

    /// <summary>
    /// Shortest way round the circle. A plain subtraction reports 358 degrees for 359 and 1, so a
    /// seed near the red end of the hue wheel would fail a hue test that is actually passing.
    /// </summary>
    private static double HueDistance(double a, double b)
    {
        var d = Math.Abs(a - b) % 360.0;
        return d > 180.0 ? 360.0 - d : d;
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
        Assert.InRange(HueDistance(walkedHue, seedHue), 0.0, 2.0);
    }

    /// <summary>
    /// The endpoint must be TRIED, and this is the regression test for the walk that stopped one
    /// 0.01 step short of it. It is also the guarantee Task 2 leans on: never return a failing
    /// tone when a passing one existed.
    ///
    /// Three earlier shapes of this test did not work, each for a reason worth keeping:
    ///
    /// 1. `Assert.NotEqual(default, walked)` cannot fail at all. `default(Color)` has alpha 0 and
    ///    `Color.FromRgb` always sets 255, so a stub returning the seed passes it.
    /// 2. Asserting a round-tripped lightness cannot work either, because the walk holds chroma
    ///    and `ToColor` clamps out-of-gamut channels, so lightness COMPRESSES near the top:
    ///    for the seed below, the last short step round-trips to L 0.9638 and the true endpoint to
    ///    L 0.9650. Any threshold has to live in that 0.0012 window. Measured, not reasoned.
    /// 3. A grey seed does not discriminate. Mid grey sits at L 0.5999, so its step grid lands on
    ///    0.9999, which already renders #FFFFFF: the broken walk and the fixed one return the
    ///    identical colour. The seed must carry chroma for the two to be different colours.
    ///
    /// So: a chromatic seed, and exact equality against the endpoint colour computed from that
    /// seed's own chroma and hue. No threshold and no tolerance, nothing to tune. With the short
    /// step the walk returns #CAFFFF and the assertion fails; with the endpoint tried it returns
    /// #CCFFFF and passes.
    /// </summary>
    [Theory]
    [InlineData(0x1D, 0x4E, 0x89, 1)]   // prussian lightening: #CAFFFF short, #CCFFFF at the end
    [InlineData(0xFF, 0x00, 0x00, -1)]  // red darkening: #130000 short, #100000 at the end
    public void Walk_TriesTheExactEndpointWhenNothingPasses(byte r, byte g, byte b, int direction)
    {
        var seed = Color.FromRgb(r, g, b);
        var (_, c, h) = Oklch.FromColor(seed);
        var endpoint = Oklch.ToColor(direction > 0 ? 1.0 : 0.0, c, h);

        var walked = Oklch.Walk(seed, _ => false, direction);

        Assert.Equal(endpoint, walked);
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OklchTests`
Expected: FAIL, `The type or namespace name 'Oklch' could not be found`.

- [x] **Step 3: Write minimal implementation**

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

        var endpoint = direction > 0 ? 1.0 : 0.0;
        var last = seed;
        for (var i = 1; i <= 100; i++)
        {
            var next = l + direction * Step * i;

            // Clamp the last step to the endpoint rather than breaking short of it. Breaking
            // stops at 0.99, so white and black are never tried, and the walk can then return a
            // tone that fails while a passing tone existed one step further on.
            var atEnd = direction > 0 ? next >= endpoint : next <= endpoint;
            if (atEnd) next = endpoint;

            last = ToColor(next, c, h);
            if (passes(last)) return last;
            if (atEnd) break;
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

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OklchTests`
Expected: PASS, 8 tests.

- [x] **Step 5: Commit**

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
- Produces: `LoftPalette.DarkFloor` and `LoftPalette.LightFloor`, each a `LoftFloor(Color Background, Color Surface, Color Foreground, Color TextSecondary, Color TextTertiary, Color Border, Color BorderStrong)`; `LoftPalette.For(bool isDark) -> LoftFloor`, which returns one of the two; and `AccentTheme.DeriveLoftRoles(Color seed, bool isDark) -> LoftAccentRoles(Color Fill, Color OnFill, Color Mark, Color Text)`.

`LoftPalette.For` is part of the produced surface, not an implementation detail: Task 3 consumes
it by name, and Task 3 is the only reason the two floors are reachable through one call.

- [x] **Step 1: Write the failing test**

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

            // BOTH surfaces, because the constraint is "every surface it is drawn on" and the two
            // differ in which is harder: on light, Background #E7E6E0 is darker than Surface
            // #F3F2ED, so a darkened tone has LESS contrast against the background. Checking only
            // Surface would miss exactly the mode where an extreme seed runs out of room, which
            // is the case this test exists for.
            foreach (var surface in new[] { floor.Surface, floor.Background })
            {
                Assert.True(
                    ContrastInk.ContrastRatio(roles.Mark, surface) >= 3.0,
                    $"Mark {roles.Mark} on {surface}, isDark {isDark}, ratio {ContrastInk.ContrastRatio(roles.Mark, surface)}");
                Assert.True(
                    ContrastInk.ContrastRatio(roles.Text, surface) >= 4.5,
                    $"Text {roles.Text} on {surface}, isDark {isDark}, ratio {ContrastInk.ContrastRatio(roles.Text, surface)}");
            }

            // The third ratio from ADR 0014, on the one surface that is not the floor.
            Assert.True(
                ContrastInk.ContrastRatio(roles.OnFill, roles.Fill) >= 4.5,
                $"OnFill {roles.OnFill} on Fill {roles.Fill}, isDark {isDark}, ratio {ContrastInk.ContrastRatio(roles.OnFill, roles.Fill)}");
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

    /// <summary>
    /// All FOURTEEN values, not just the two surfaces per mode. This test is what
    /// `scripts/build-loft-tokens.ps1` names as the guard against someone hand-editing the
    /// generated constants instead of regenerating them, and a guard that covers four of fourteen
    /// makes that claim false for the other ten. Nothing else pins `Foreground`,
    /// `TextSecondary`, `TextTertiary`, `Border` or `BorderStrong`.
    /// </summary>
    [Fact]
    public void BothFloors_MatchTheSpec()
    {
        Assert.Equal(Color.FromRgb(0x0B, 0x0B, 0x0A), LoftPalette.DarkFloor.Background);
        Assert.Equal(Color.FromRgb(0x15, 0x14, 0x12), LoftPalette.DarkFloor.Surface);
        Assert.Equal(Color.FromRgb(0xEC, 0xE9, 0xE2), LoftPalette.DarkFloor.Foreground);
        Assert.Equal(Color.FromRgb(0xA3, 0x9E, 0x94), LoftPalette.DarkFloor.TextSecondary);
        Assert.Equal(Color.FromRgb(0x7A, 0x75, 0x6C), LoftPalette.DarkFloor.TextTertiary);
        Assert.Equal(Color.FromRgb(0x2B, 0x28, 0x24), LoftPalette.DarkFloor.Border);
        Assert.Equal(Color.FromRgb(0x7A, 0x75, 0x6C), LoftPalette.DarkFloor.BorderStrong);

        Assert.Equal(Color.FromRgb(0xE7, 0xE6, 0xE0), LoftPalette.LightFloor.Background);
        Assert.Equal(Color.FromRgb(0xF3, 0xF2, 0xED), LoftPalette.LightFloor.Surface);
        Assert.Equal(Color.FromRgb(0x16, 0x16, 0x16), LoftPalette.LightFloor.Foreground);
        Assert.Equal(Color.FromRgb(0x55, 0x54, 0x4F), LoftPalette.LightFloor.TextSecondary);
        Assert.Equal(Color.FromRgb(0x77, 0x75, 0x6D), LoftPalette.LightFloor.TextTertiary);
        Assert.Equal(Color.FromRgb(0xD0, 0xCE, 0xC6), LoftPalette.LightFloor.Border);
        Assert.Equal(Color.FromRgb(0x77, 0x75, 0x6D), LoftPalette.LightFloor.BorderStrong);
    }

    [Fact]
    public void For_ReturnsTheFloorForTheMode()
    {
        Assert.Equal(LoftPalette.DarkFloor, LoftPalette.For(isDark: true));
        Assert.Equal(LoftPalette.LightFloor, LoftPalette.For(isDark: false));
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests`
Expected: FAIL, `The type or namespace name 'LoftPalette' could not be found`.

- [x] **Step 3: Write minimal implementation**

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

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests`
Expected: PASS. If an extreme seed fails, the walk ran out of range rather than the math being wrong: fix by walking the other direction when the first endpoint fails, not by lowering a ratio.

- [x] **Step 5: Make the floors generated rather than copied**

The spec asks for this and the reason is specific: a hand copy of fourteen colours drifts the first
time either side is tuned, and the drift is silent because both sides still compile. The floors
stay checked in, so a build never needs the praxvon-ui checkout; the script is what proves they
still match, and `BothFloors_MatchTheSpec` is what fails when they do not.

Create `scripts/build-loft-tokens.ps1`:

```powershell
# Regenerates the floor constants in src/Plith/Services/LoftPalette.cs from praxvon-ui's
# theme tokens, the sibling of that repo's own `build-tokens`.
#
# The generated values are checked in on purpose: a Plith build must not need the praxvon-ui
# checkout. This script is run by a person when praxvon-ui's theme moves, and its output is
# reviewed like any other change. LoftAccentTests.BothFloors_MatchTheSpec is the guard that
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

- [x] **Step 6: Commit**

```bash
git add src/Plith/Services/LoftPalette.cs src/Plith/Services/AccentTheme.cs scripts/build-loft-tokens.ps1 tests/Plith.Tests/LoftAccentTests.cs
git commit -m "feat(theme): derive the four Loft accent roles"
```

---

### Task 3: Put the floor under the notch, and delete what it replaces

**Files:**
- Modify: `src/Plith/Services/ThemeService.cs:228-300` (`BuildAccentOverride`)
- Modify: `src/Plith/Services/AccentTheme.cs` (remove `DeriveOsdSurfaces` and the `OsdSurfaceDerived` record; retarget `Derive`'s own call at `:121`)
- Modify: `src/Plith/Resources/OsdPalette.Dark.xaml`, `OsdPalette.Light.xaml` (surface brushes become the Loft floors; the three `Notch*` keys re-valued to floor constants)
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs` (`DerivePalette`, the cross-process shelf wire)
- Modify: `scripts/check-contrast.ps1`, `scripts/render-widgets.ps1`, `scripts/render-accent-variants.ps1` (they reach the deleted method by reflection)
- Modify: `tests/Plith.Tests/AccentThemeTests.cs` (remove the six tests covering the deleted method)
- Test: `tests/Plith.Tests/ThemeServiceLoftTests.cs`

**Interfaces:**
- Consumes: `AccentTheme.DeriveLoftRoles`, `LoftPalette.For`.
- Produces: the override dictionary's key set, which the render harness and `OsdHost.RefreshAccentMirror` both mirror. Keys after this task: `AccentBrush`, `AccentHover`, `AccentPressed`, `AccentGlow`, `OsdAccent`, `OsdAccentMark`, `OsdAccentText`, `OsdSurfaceBrush`, `OsdBorder`, `OsdTrackBg`, `OsdDivider`, `OnAccent`, `AccentTrack`.

Removed from the override, with the reason in one line each: `NotchInk`, `NotchInkMuted` and
`NotchTrack` were computed from `osd.SurfaceEnd` because the surface moved; the floor's
`Foreground`, `TextSecondary` and a fixed track now say the same thing without a computation.

**Three corrections to this task, ruled before execution. The ledger at
`.superpowers/sdd/2026-10-13-notch-in-loft/progress.md` holds the full reasoning and the cost of
each being wrong.**

1. **`AccentTrack` keeps its derivation** and stays in the override, so three keys leave it rather
   than four. Spec section 4 lists it among five keys that lose their reason to exist, but spec
   section 5 puts the Settings window out of scope, and this key lives entirely on the Settings
   path: defined in `Palette.Dark/Light/HighContrast.xaml`, consumed only by
   `SettingsTheme.xaml:531` (`ModernProgressBarStyle`, the Updates card's download bar). Its own
   comment there records why it is accent-derived rather than a general neutral: a deep accent
   measured 1.6:1 against `ButtonGhost`, "a bar with no visible progress". Dropping the override
   falls back to `Palette.Light.xaml:27` `#2A323C`, a dark slab under a light card, which
   re-introduces that failure on the theme where it is worst. Section 4's own rationale is limited
   to "the notch path", and this key is not on it.
2. **The three `Notch*` keys keep their definitions** in `OsdPalette.Dark.xaml` and
   `OsdPalette.Light.xaml`, re-valued to the floor's `Foreground`, `TextSecondary` and `Border`.
   Only their `BuildAccentOverride` entries go. They have about forty `DynamicResource` consumers
   across `AudioWidget.xaml`, `ClockWidget.xaml`/`.cs` and `MediaWidget.xaml`/`.cs`, and an
   unresolved `DynamicResource` fails silently in WPF, with no build error and no test failure. No
   consuming view changes. `OsdPalette.HighContrast.xaml` keeps its own `SystemColors`-mapped
   versions, untouched.
3. **Deleting `DeriveOsdSurfaces` reaches five call sites beyond `BuildAccentOverride`**, and the
   three scripts among them reach it by reflection, so they break at run time rather than build
   time. Two of those scripts are this task's own Steps 5 and 6, so leaving them would make the
   task unable to verify itself.
   - `AccentTheme.cs:121`, inside `Derive` itself, the Settings accent path's light branch:
     retarget to `LoftPalette.LightFloor.Surface`. It was already measuring against a derived
     light panel, so the floor makes that a constant without redesigning Settings.
   - `ShelfSession.cs:387`, `DerivePalette`: rewrite onto the floor plus `DeriveLoftRoles`.
     Required, not optional. Spec section 5 says the shelf "follows automatically once the wire
     carries Loft values", and that wire's design premise is that the answer crosses it rather
     than the inputs, so a second derivation here would drift.
   - `check-contrast.ps1:258`, `render-widgets.ps1:116`, `render-accent-variants.ps1:52`: move to
     the floor.
   - `AccentThemeTests.cs` lines 159, 185, 216, 233, 249 and 264: six tests on the deleted
     method, which go with it.

- [x] **Step 1: Write the failing test**

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

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~ThemeServiceLoftTests`
Expected: FAIL. `OsdSurfaceBrush` still carries the accent-derived gradient and `OsdAccentMark` does not exist.

- [x] **Step 3: Write minimal implementation**

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

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS, the whole suite. Existing tests that asserted the old tinted surface will fail and must be rewritten to assert the floor, not deleted: each one is a recorded decision and the rewrite says which way it went.

- [x] **Step 5: Look at it**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-3 -Theme Dark -Accent '#6D28D9'`
Then: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-3-light -Theme Light -Accent '#6D28D9'`
Expected: the panel is the floor in both, violet appears only in lines and text. Open `frame-page1.png` and `widget-media.png` and confirm with your eyes. A passing test here means the numbers are right; only the render shows whether the thing is right.

- [x] **Step 6: Run the contrast harness**

Run: `pwsh -File scripts/check-contrast.ps1`
Expected: PASS on both themes and every accent.

- [x] **Step 7: Commit**

```bash
git add src/Plith/Services/ThemeService.cs src/Plith/Services/AccentTheme.cs src/Plith/Resources/OsdPalette.Dark.xaml src/Plith/Resources/OsdPalette.Light.xaml tests/Plith.Tests/
git commit -m "feat(theme): put the Loft floor under the notch"
```

---

### Task 4: Typography

**Files:**
- Create: `src/Plith/Assets/Fonts/PublicSans-Regular.ttf`, `PublicSans-SemiBold.ttf`, `MartianMono-StdRg.ttf`, plus `OFL-PublicSans.txt` and `OFL-MartianMono.txt`
- Modify: `src/Plith/Plith.csproj` (embed the fonts as `Resource`)
- Modify: `src/Plith/Resources/Theme.xaml:8` (`OsdUiFont`, plus a new `OsdMonoFont`)
- Modify: `src/Plith/Views/AmbientCardView.xaml`, `MediaCardView.xaml`, `AudioCardView.xaml`, `BrightnessCardView.xaml` (tabular figures on every changing number)
- Modify: `NOTICE.md`
- Test: `tests/Plith.Tests/TypographyTests.cs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: resource keys `OsdUiFont` and `OsdMonoFont`, both `FontFamily`.

Tabular figures in WPF are a typography feature on the run, not a property of the family: `<Run Typography.NumeralAlignment="Tabular">` or `Typography.NumeralAlignment="Tabular"` on the `TextBlock`. Without it the clock's digits change width as they tick and the whole line shifts, which is the defect this fixes.

- [x] **Step 1: Write the failing test**

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

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~TypographyTests`
Expected: FAIL, `OsdMonoFont` is not a key in the dictionary.

- [x] **Step 3: Write minimal implementation**

In `src/Plith/Resources/Theme.xaml`, replace line 8 and add the mono family:

```xml
    <FontFamily x:Key="OsdUiFont">pack://application:,,,/Assets/Fonts/#Public Sans, Segoe UI Variable, Segoe UI</FontFamily>
    <FontFamily x:Key="OsdMonoFont">pack://application:,,,/Assets/Fonts/#Martian Mono Std Rg, Cascadia Mono, Consolas</FontFamily>
```

**The mono family name is measured, not guessed, and it is not what this plan first said.** The
file's internal family name is `Martian Mono Std Rg`, not `Martian Mono`. Taken by asking WPF
itself, which is the only authority that matters here:

```powershell
Add-Type -AssemblyName PresentationCore
[System.Windows.Media.Fonts]::GetFontFamilies("<dir>\")   # trailing slash is required
# -> #Martian Mono Std Rg   (Normal and Bold, each with an Oblique)
# -> #Public Sans           (Normal and SemiBold, each with an Oblique)
```

`#Martian Mono` resolves to nothing and falls back to Cascadia Mono, which renders perfectly and
is exactly the trap Step 5 below warns about. Public Sans is `Public Sans` as originally written,
with Regular and SemiBold in one family, so `FontWeight="SemiBold"` reaches the second file.

**Where the files come from.** Upstream for the mono moved: `evilmartians/martian-mono` is a 404
and the project is now `evilmartians/mono`. Its regular cut ships as `MartianMono-StdRg.ttf`
(`Std` is the width, `Rg` the weight; the release also carries Condensed and SemiCondensed
widths). Upstream filenames are kept verbatim, because the cut is information worth keeping and
the `Assets\Fonts\*.ttf` glob makes the filename irrelevant to the build.

- Public Sans: `uswds/public-sans` release v2.001, `fonts/ttf/` inside the zip.
- Martian Mono: `evilmartians/mono` release v1.1.0, `martian-mono-1.1.0-ttf.zip`.
- Both OFL texts ship beside the fonts. The mono's copyright line reads "Copyright 2021 The
  Martian Mono Project Authors (https://github.com/evilmartians/mono)".

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

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~TypographyTests`
Expected: PASS.

- [x] **Step 5: Look at it, twice**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-4`
Expected: the fonts actually loaded. A failed pack URI falls back to Segoe UI and renders perfectly, so compare `widget-clock.png` against Task 3's output and confirm the face changed. If it did not, the pack URI family name is wrong: the `#` name is the font's internal family name, not the file name.

- [x] **Step 6: Commit**

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

- [x] **Step 1: Write the failing test**

```csharp
using System.Windows;

namespace Plith.Tests;

public class OrnamentTests
{
    private static ResourceDictionary Load(string name) =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri($"/Plith;component/Resources/{name}", UriKind.Relative));

    /// <summary>
    /// Off by FALLTHROUGH, not by being written into the high contrast palette. That file is left
    /// alone (spec section 7), so this asserts what a lookup RESOLVES to with the high contrast
    /// palette on top of the always-merged defaults, which is the thing the running app does. A
    /// direct key read against `OsdPalette.HighContrast.xaml` would be asserting the opposite
    /// design.
    /// </summary>
    [StaFact]
    public void Ornaments_ResolveToOffUnderHighContrast()
    {
        var stack = new ResourceDictionary();
        stack.MergedDictionaries.Add(Load("Ornaments.xaml"));
        stack.MergedDictionaries.Add(Load("OsdPalette.HighContrast.xaml"));

        // Off, not absent: a missing key throws at resolve time on a surface a user with high
        // contrast turned on is already relying on.
        Assert.Equal(0.0, (double)stack["OrnamentCornerLength"]);
        Assert.Equal(0.0, (double)stack["OrnamentOutlineThickness"]);
        Assert.Equal("None", (string)stack["OrnamentLabelCase"]);
    }

    /// <summary>
    /// The constraint this pins is that the high contrast palette does not define the ornament
    /// tokens at all. If a later change writes them in there, the test above would still pass
    /// while the spec was being broken.
    /// </summary>
    [StaFact]
    public void Ornaments_AreNotDefinedInTheHighContrastPalette()
    {
        var hc = Load("OsdPalette.HighContrast.xaml");

        foreach (var key in new[]
                 { "OrnamentCornerLength", "OrnamentOutlineThickness", "OrnamentLabelCase" })
        {
            Assert.False(
                hc.Contains(key),
                $"{key} is defined in OsdPalette.HighContrast.xaml; spec section 7 leaves that file alone");
        }
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

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OrnamentTests`
Expected: FAIL, `OrnamentCornerLength` is not a key.

- [x] **Step 3: Write minimal implementation**

Add to each palette. `OsdPalette.Dark.xaml` and `OsdPalette.Light.xaml`:

```xml
    <!-- Loft's ornament layer, as tokens so a theme turns it off without touching a view.
         9 DIP is praxvon-ui's `ornament.corner`. -->
    <sys:Double x:Key="OrnamentCornerLength">9</sys:Double>
    <sys:Double x:Key="OrnamentOutlineThickness">1</sys:Double>
    <sys:String x:Key="OrnamentLabelCase">Upper</sys:String>
```

**`OsdPalette.HighContrast.xaml` is NOT edited.** The off values are the defaults, and they live
in `Ornaments.xaml`, which is always merged:

```xml
    <!-- The off values, as defaults. Any palette that wants ornaments overrides them; a palette
         that says nothing gets no ornaments. High contrast says nothing, deliberately: spec
         section 7 leaves that file alone, because Loft's ornaments would be a second opinion
         about colours Windows has already decided for the one group of users who cannot work
         around a wrong answer. -->
    <sys:Double x:Key="OrnamentCornerLength">0</sys:Double>
    <sys:Double x:Key="OrnamentOutlineThickness">0</sys:Double>
    <sys:String x:Key="OrnamentLabelCase">None</sys:String>
```

`Ornaments.xaml` must therefore be merged BEFORE the active palette in `App.xaml`, or the palette
cannot override it. Both files need `xmlns:sys="clr-namespace:System;assembly=System.Runtime"`.

This corrects an earlier draft of this task, which wrote the off values into
`OsdPalette.HighContrast.xaml` and so contradicted this plan's own Global Constraints. The spec
sentence that settles it needs both halves to hold at once: the file "is left alone" AND "the
ornament tokens resolve to their off values there". Fallthrough is the only arrangement that does
both. Ruling recorded in `.superpowers/sdd/2026-10-13-notch-in-loft/progress.md`.

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

`Ornaments.xaml` must not use an absolute `pack://application:,,,/Plith;component/` URI, which is
what shipped the broken 0.1.6 installer. Note that the `xmlns:sys` declaration above is a
`clr-namespace` naming a framework assembly, which is required and is not what that check guards:
`scripts/check-shared-xaml.ps1` reads the INSTALLER's csproj and flags absolute same-solution pack
URIs, nothing else.

An earlier draft of this task said `Ornaments.xaml` "is loaded by both Plith and
Plith.DropCatcher". That is measured false: the catcher links five `.cs` files and no XAML, and
owns `App.xaml`, `CatcherWindow.xaml`, `Shelf/ShelfSurface.xaml` and `Shelf/ShelfWindow.xaml`
itself. The installer links only `Palette.Dark.xaml`, `SettingsTheme.xaml` and `PlithIcons.xaml`.
So this dictionary is Plith-only, Step 5's check passes trivially and proves nothing about it, and
**the catcher's shelf page gets no corner marks in this work.** If that one-page inconsistency
reads wrong on screen, the fix is to link the dictionary into the catcher, as its own change.

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~OrnamentTests`
Expected: PASS.

- [x] **Step 5: Run the shared XAML check**

Run: `pwsh -File scripts/check-shared-xaml.ps1`
Expected: PASS. This check exists because its failure mode is invisible to the build and to both test suites.

- [x] **Step 6: Look at it**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -OutDir $env:TEMP\loft-5`
Expected: corner marks present and inside the radius, ticks under the volume and brightness bars, labels uppercase in the mono face.

- [x] **Step 7: Commit**

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

- [x] **Step 1: Write the failing test**

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

- [x] **Step 2: Run test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettleTests`
Expected: FAIL, `LoftSettleEase` is not a key.

- [x] **Step 3: Write minimal implementation**

In `src/Plith/Resources/Theme.xaml`:

```xml
    <!-- Loft: a value settles on its mark without overshoot. praxvon-ui pairs Loft with the
         `calm` personality, whose smooth spring is bounce 0. WPF has no spring, and an ease-out
         cubic is the closest standard curve that cannot pass its target. -->
    <Duration x:Key="LoftSettleDuration">0:0:0.4</Duration>
    <CubicEase x:Key="LoftSettleEase" EasingMode="EaseOut" />
```

Point the volume bar, the brightness bar and the seek thumb at both keys, replacing whatever duration and easing they carry now. Where a view checks `SystemParameters.ClientAreaAnimation` or an equivalent reduced-motion flag, keep that branch and let it jump.

- [x] **Step 4: Run test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettleTests`
Expected: PASS.

- [x] **Step 5: Commit**

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
