using System.Windows.Media;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Tests;

public class AudioCardViewModelTests
{
    [Fact]
    public void ApplyVoicemeeterSnapshot_NormalizesDbCorrectly()
    {
        var vm = new AudioCardViewModel();
        // At 0 dB, normalized = (0 - (-60)) / (12 - (-60)) = 60 / 72 = 0.8333...
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 0f, Muted: false));
        Assert.InRange(vm.GainNormalized, 0.832, 0.834);
        Assert.Equal("0.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyVoicemeeterSnapshot_ClampsBelowMin()
    {
        var vm = new AudioCardViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", -120f, Muted: false));
        Assert.Equal(0, vm.GainNormalized);
        Assert.Equal("-120.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyVoicemeeterSnapshot_ClampsAboveMax()
    {
        var vm = new AudioCardViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 100f, Muted: false));
        Assert.Equal(1, vm.GainNormalized);
        Assert.Equal("+100.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyMuted_DisplaysMUTED()
    {
        var vm = new AudioCardViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 0f, Muted: true));
        Assert.Equal("MUTED", vm.GainText);
        Assert.True(vm.Muted);
    }

    [Fact]
    public void ApplyGenericOverload_StoresValuesAsGiven()
    {
        var vm = new AudioCardViewModel();
        vm.Apply("Speakers (G733)", 0.75, "75%", muted: false);

        Assert.Equal("Speakers (G733)", vm.Label);
        Assert.Equal(0.75, vm.GainNormalized);
        Assert.Equal("75%", vm.GainText);
        Assert.False(vm.Muted);
    }

    [Fact]
    public void ApplyGenericOverload_MutedReplacesText()
    {
        var vm = new AudioCardViewModel();
        vm.Apply("Speakers", 0.5, "50%", muted: true);
        Assert.Equal("MUTED", vm.GainText);
    }

    [Fact]
    public void GainNormalized_ClampsAtTheSetter()
    {
        var vm = new AudioCardViewModel { GainNormalized = 1.5 };
        Assert.Equal(1.0, vm.GainNormalized);
        vm.GainNormalized = -0.3;
        Assert.Equal(0.0, vm.GainNormalized);
    }
}

public class NormalizedToWidthConverterTests
{
    [Fact]
    public void Convert_HalfNormalized_ReturnsHalfWidth()
    {
        var c = new NormalizedToWidthConverter();
        var r = c.Convert(new object[] { 0.5, 200.0 }, typeof(double), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.Equal(100.0, r);
    }

    [Fact]
    public void Convert_OverOne_ClampsAtFullWidth()
    {
        var c = new NormalizedToWidthConverter();
        var r = c.Convert(new object[] { 2.0, 200.0 }, typeof(double), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.Equal(200.0, r);
    }

    [Fact]
    public void Convert_Negative_ClampsAtZero()
    {
        var c = new NormalizedToWidthConverter();
        var r = c.Convert(new object[] { -0.5, 200.0 }, typeof(double), null, System.Globalization.CultureInfo.InvariantCulture);
        Assert.Equal(0.0, r);
    }
}

public class AudioCardViewModelColorTests
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
        var vm = new AudioCardViewModel { Muted = true, UseColorThresholds = true, GainNormalized = 0.95 };
        Assert.Equal(Gray, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOff_UsesAccentBrush()
    {
        // Default accent seed equals emerald, so the ARGB check still passes — but the
        // semantic assertion is different: bar colour now follows the Theme Studio accent
        // rather than the semantic-safe green. Any future accent change would be observable
        // by refreshing the ViewModel with a live palette dictionary.
        var vm = new AudioCardViewModel { UseColorThresholds = false, GainNormalized = 0.99 };
        Assert.Equal(Green, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Below070_IsGreen()
    {
        var vm = new AudioCardViewModel { UseColorThresholds = true, GainNormalized = 0.50 };
        Assert.Equal(Green, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Between070And090_IsAmber()
    {
        var vm = new AudioCardViewModel { UseColorThresholds = true, GainNormalized = 0.85 };
        Assert.Equal(Amber, ToArgb(vm.GainColor));
    }

    [Fact]
    public void ThresholdsOn_Above090_IsRed()
    {
        var vm = new AudioCardViewModel { UseColorThresholds = true, GainNormalized = 0.95 };
        Assert.Equal(Red, ToArgb(vm.GainColor));
    }
}
