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
