using Plith.Services;
using Plith.ViewModels;

namespace Plith.Tests;

public class OsdViewModelTests
{
    [Fact]
    public void ApplyVoicemeeterSnapshot_NormalizesDbCorrectly()
    {
        var vm = new OsdViewModel();
        // At 0 dB, normalized = (0 - (-60)) / (12 - (-60)) = 60 / 72 = 0.8333...
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 0f, Muted: false));
        Assert.InRange(vm.GainNormalized, 0.832, 0.834);
        Assert.Equal("0.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyVoicemeeterSnapshot_ClampsBelowMin()
    {
        var vm = new OsdViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", -120f, Muted: false));
        Assert.Equal(0, vm.GainNormalized);
        Assert.Equal("-120.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyVoicemeeterSnapshot_ClampsAboveMax()
    {
        var vm = new OsdViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 100f, Muted: false));
        Assert.Equal(1, vm.GainNormalized);
        Assert.Equal("+100.0 dB", vm.GainText);
    }

    [Fact]
    public void ApplyMuted_DisplaysMUTED()
    {
        var vm = new OsdViewModel();
        vm.Apply(new VoicemeeterParameterSnapshot(VoicemeeterRail.Bus, 0, "Bus A1", 0f, Muted: true));
        Assert.Equal("MUTED", vm.GainText);
        Assert.True(vm.Muted);
    }

    [Fact]
    public void ApplyGenericOverload_StoresValuesAsGiven()
    {
        var vm = new OsdViewModel();
        vm.Apply("Speakers (G733)", 0.75, "75%", muted: false);

        Assert.Equal("Speakers (G733)", vm.Label);
        Assert.Equal(0.75, vm.GainNormalized);
        Assert.Equal("75%", vm.GainText);
        Assert.False(vm.Muted);
    }

    [Fact]
    public void ApplyGenericOverload_MutedReplacesText()
    {
        var vm = new OsdViewModel();
        vm.Apply("Speakers", 0.5, "50%", muted: true);
        Assert.Equal("MUTED", vm.GainText);
    }

    [Fact]
    public void GainNormalized_ClampsAtTheSetter()
    {
        var vm = new OsdViewModel { GainNormalized = 1.5 };
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
