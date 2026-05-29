using Plith.Services;

namespace Plith.Tests;

public class HotkeyServiceTests
{
    [Fact]
    public void FormatCombo_Empty_WhenEitherZero()
    {
        Assert.Equal("", HotkeyService.FormatCombo(0, 0));
        Assert.Equal("", HotkeyService.FormatCombo(0x02, 0));      // mods only
        Assert.Equal("", HotkeyService.FormatCombo(0, 0x56));      // vk only
    }

    [Fact]
    public void FormatCombo_OrdersModifiers_CtrlAltShiftWin()
    {
        // Win bit is 0x08, ordering inside FormatCombo: Ctrl, Alt, Shift, Win.
        var s = HotkeyService.FormatCombo(0x02 | 0x01 | 0x04 | 0x08, 0x56);
        Assert.StartsWith("Ctrl+Alt+Shift+Win+", s, StringComparison.Ordinal);
        Assert.Contains("V", s, StringComparison.Ordinal);
    }

    [Fact]
    public void FormatCombo_CtrlAltV_FormatsAsExpected()
    {
        Assert.Equal("Ctrl+Alt+V", HotkeyService.FormatCombo(0x02 | 0x01, 0x56));
    }

    [Fact]
    public void MigrateLegacy_KnownStrings_ReturnExpectedPairs()
    {
        var (m1, k1) = HotkeyService.MigrateLegacy("CtrlAltV");
        Assert.Equal((uint)(0x02 | 0x01), m1);
        Assert.Equal(0x56, k1);

        var (m2, k2) = HotkeyService.MigrateLegacy("CtrlAltM");
        Assert.Equal((uint)(0x02 | 0x01), m2);
        Assert.Equal(0x4D, k2);          // M
    }

    [Fact]
    public void MigrateLegacy_UnknownOrNull_ReturnsZeroes()
    {
        Assert.Equal(((uint)0, 0), HotkeyService.MigrateLegacy(null));
        Assert.Equal(((uint)0, 0), HotkeyService.MigrateLegacy(""));
        Assert.Equal(((uint)0, 0), HotkeyService.MigrateLegacy("NonsenseValue"));
    }

    [Fact]
    public void IsBound_StartsFalse_AndActiveFieldsZero()
    {
        // Ctor must not register anything; nothing is bound until Apply succeeds.
        using var svc = new HotkeyService();
        Assert.False(svc.IsBound);
        Assert.Equal((uint)0, svc.ActiveMods);
        Assert.Equal(0, svc.ActiveKey);
    }
}
