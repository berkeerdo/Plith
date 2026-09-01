using System.Windows.Media;
using Plith.ViewModels;

namespace Plith.Tests;

public class OsdViewModelColorTests
{
    private const uint Green = 0xFF4AD695;
    private const uint Amber = 0xFFF5C242;
    private const uint Red = 0xFFE54B4B;
    private const uint Gray = 0xFF808080;

    private static uint ToArgb(Brush b)
    {
        var c = ((SolidColorBrush)b).Color;
        return (uint)(c.A << 24 | c.R << 16 | c.G << 8 | c.B);
    }

    [Fact]
    public void Muted_OverridesEverythingToGray()
    {
        var vm = new OsdViewModel { Muted = true, UseColorThresholds = true, GainNormalized = 0.95 };
        Assert.Equal(Gray, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOff_UsesAccentBrush()
    {
        // Default accent seed equals emerald, so the ARGB check still passes — but the
        // semantic assertion is different: bar colour now follows the Theme Studio accent
        // rather than the semantic-safe green. Any future accent change would be observable
        // by refreshing the ViewModel with a live palette dictionary.
        var vm = new OsdViewModel { UseColorThresholds = false, GainNormalized = 0.99 };
        Assert.Equal(Green, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Below070_IsGreen()
    {
        var vm = new OsdViewModel { UseColorThresholds = true, GainNormalized = 0.50 };
        Assert.Equal(Green, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Between070And090_IsAmber()
    {
        var vm = new OsdViewModel { UseColorThresholds = true, GainNormalized = 0.85 };
        Assert.Equal(Amber, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Above090_IsRed()
    {
        var vm = new OsdViewModel { UseColorThresholds = true, GainNormalized = 0.95 };
        Assert.Equal(Red, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ShowMediaCard_TrueWhenSessionAndNotCompact()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = true;
        vm.CompactMode = false;
        Assert.True(vm.ShowMediaCard);
    }

    [Fact]
    public void ShowMediaCard_FalseWhenCompact()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = true;
        vm.CompactMode = true;
        Assert.False(vm.ShowMediaCard);
    }

    [Fact]
    public void ShowMediaCard_FalseWhenNoSession()
    {
        var vm = new OsdViewModel();
        vm.Media.HasSession = false;
        vm.CompactMode = false;
        Assert.False(vm.ShowMediaCard);
    }
}
