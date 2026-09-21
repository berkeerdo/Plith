using System.Windows.Input;
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

    // The three tests below are everything this suite can prove about HotkeyService.
    // Registration itself cannot be reached from here: RegisterHotKey needs the message-only
    // window, HwndSource refuses to be built off an STA thread, and this suite runs MTA.
    // Measured rather than assumed: a probe reported
    // "apartment=MTA | HwndSource: InvalidOperationException: The calling thread must be STA".
    // Whether two hotkeys actually coexist is a hardware check, listed in the plan's final task.

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

    [Fact]
    public void TwoServicesWithDifferentIdsCanBeBuiltAndDisposed()
    {
        // The summon hotkey is one binding; brightness needs two more. The id had to stop
        // being a constant for a second instance to mean anything.
        using var up = new HotkeyService(hotkeyId: 2);
        using var down = new HotkeyService(hotkeyId: 3);

        Assert.True(up.NoRepeat);
        Assert.False(up.IsBound);
        Assert.False(down.IsBound);
    }

    [Fact]
    public void AServiceThatCannotBuildItsWindowReportsUnboundRatherThanThrowing()
    {
        // This suite is the failure case: no STA thread, so the message-only window cannot
        // exist. A hotkey that cannot register must leave the product running.
        using var service = new HotkeyService(hotkeyId: 6);

        var mods = (uint)(HotkeyService.HotkeyMods.Control | HotkeyService.HotkeyMods.Alt);
        var applied = service.Apply(mods, KeyInterop.VirtualKeyFromKey(Key.F13));

        Assert.False(applied);
        Assert.False(service.IsBound);
        Assert.Equal(0u, service.ActiveMods);
    }
}
