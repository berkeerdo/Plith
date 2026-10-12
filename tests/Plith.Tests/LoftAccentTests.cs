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
