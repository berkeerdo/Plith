using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Media;
using Plith.Views.Presentation;

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

        // Off, not absent: a missing key is a silent miss at resolve time on a surface a user
        // with high contrast turned on is already relying on.
        Assert.Equal(0.0, (double)stack["OrnamentCornerLength"]);
        Assert.Equal(0.0, (double)stack["OrnamentOutlineThickness"]);
        Assert.Equal("None", (string)stack["OrnamentLabelCase"]);
        Assert.NotNull(stack["OrnamentTickBrush"]);

        // The label treatment is an ornament too, so high contrast keeps the OLD one: the UI
        // face, 11 with small caps for a rail label, 13 for a title.
        Assert.Contains("Public Sans", ((FontFamily)stack["OrnamentLabelFont"]).Source);
        Assert.Equal(11.0, (double)stack["OrnamentLabelSize"]);
        Assert.Equal(13.0, (double)stack["OrnamentTitleLabelSize"]);
        Assert.Equal(FontCapitals.AllSmallCaps, (FontCapitals)stack["OrnamentLabelCaps"]);
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
                 { "OrnamentCornerLength", "OrnamentOutlineThickness", "OrnamentLabelCase", "OrnamentTickBrush" })
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
            Assert.NotNull(palette["OrnamentTickBrush"]);
            Assert.Contains("Martian Mono Std Rg", ((FontFamily)palette["OrnamentLabelFont"]).Source);
            Assert.Equal(10.0, (double)palette["OrnamentLabelSize"]);
            Assert.Equal(11.0, (double)palette["OrnamentTitleLabelSize"]);
            Assert.Equal(FontCapitals.Normal, (FontCapitals)palette["OrnamentLabelCaps"]);
        }
    }

    /// <summary>
    /// The whole design rests on the defaults being merged BEFORE the OSD palette, because in WPF
    /// a later dictionary wins a duplicate key. Merged after, the off values would beat the on
    /// values and no ornament would ever draw, with no error anywhere.
    /// </summary>
    [Fact]
    public void AppXaml_MergesOrnamentsBeforeTheOsdPalette()
    {
        var dir = AppContext.BaseDirectory;
        string? appXaml = null;
        for (var d = new DirectoryInfo(dir); d is not null; d = d.Parent)
        {
            var candidate = Path.Combine(d.FullName, "src", "Plith", "App.xaml");
            if (File.Exists(candidate)) { appXaml = candidate; break; }
        }

        Assert.NotNull(appXaml);
        var text = File.ReadAllText(appXaml!);
        var ornaments = text.IndexOf("Resources/Ornaments.xaml", StringComparison.Ordinal);
        var palette = text.IndexOf("Resources/OsdPalette.Dark.xaml", StringComparison.Ordinal);

        Assert.True(ornaments >= 0, "App.xaml does not merge Ornaments.xaml");
        Assert.True(ornaments < palette, "Ornaments.xaml must be merged before the OSD palette");
    }

    [StaFact]
    public void LoftFrame_DrawsFourCornerMarks()
    {
        var style = (Style)Load("Ornaments.xaml")["LoftFrame"];
        var control = new Control { Style = style, Width = 200, Height = 100 };
        control.ApplyTemplate();

        var grid = Assert.IsType<Grid>(VisualTreeHelper.GetChild(control, 0));
        Assert.Equal(4, grid.Children.OfType<System.Windows.Shapes.Path>().Count());
    }

    [StaFact]
    public void LoftLabel_UppercasesOnlyWhenTheCaseTokenSaysUpper()
    {
        var label = new LoftLabel { Text = "usb out" };
        Assert.Equal("usb out", label.Text);

        label.Case = "Upper";
        Assert.Equal("USB OUT", label.Text);

        label.Text = "line 2";
        Assert.Equal("LINE 2", label.Text);

        label.Case = "None";
        Assert.Equal("line 2", label.Text);
    }

    [StaFact]
    public void DecorativeText_CreatesNoAutomationPeer()
    {
        var mark = new DecorativeText { Text = "[" };

        Assert.Null(System.Windows.Automation.Peers.UIElementAutomationPeer.CreatePeerForElement(mark));
    }

    private static LoftLabel LabelOver(string palette, string style = "LoftLabelStyle")
    {
        var stack = new ResourceDictionary();
        stack.MergedDictionaries.Add(Load("Ornaments.xaml"));
        stack.MergedDictionaries.Add(Load(palette));
        var label = new LoftLabel { Resources = stack, Text = "usb out" };
        label.SetResourceReference(FrameworkElement.StyleProperty, style);
        return label;
    }

    /// <summary>
    /// The wiring, not the property: the style must read the TOKEN. Break the Case binding in
    /// LoftLabelStyle and setting Case by hand would still pass the test above.
    /// </summary>
    [StaFact]
    public void LoftLabelStyle_FollowsTheTokensOfTheActivePalette()
    {
        var on = LabelOver("OsdPalette.Dark.xaml");
        Assert.Equal("USB OUT", on.Text);
        Assert.Equal(10.0, on.FontSize);
        Assert.Equal(FontCapitals.Normal, on.GetValue(Typography.CapitalsProperty));
        Assert.Contains("Martian Mono", on.FontFamily.Source);

        var off = LabelOver("OsdPalette.HighContrast.xaml");
        Assert.Equal("usb out", off.Text);
        Assert.Equal(11.0, off.FontSize);
        Assert.Equal(FontCapitals.AllSmallCaps, off.GetValue(Typography.CapitalsProperty));
        Assert.Contains("Public Sans", off.FontFamily.Source);

        var title = LabelOver("OsdPalette.HighContrast.xaml", "LoftTitleLabelStyle");
        Assert.Equal(13.0, title.FontSize);
        Assert.Equal(FontCapitals.Normal, title.GetValue(Typography.CapitalsProperty));
    }

    /// <summary>
    /// A swap that changes the token reaches a label already on screen, which is how a theme
    /// change to high contrast arrives.
    /// </summary>
    [StaFact]
    public void LoftLabelStyle_ReactsWhenThePaletteIsSwapped()
    {
        var label = LabelOver("OsdPalette.Dark.xaml");
        Assert.Equal("USB OUT", label.Text);

        var stack = new ResourceDictionary();
        stack.MergedDictionaries.Add(Load("Ornaments.xaml"));
        stack.MergedDictionaries.Add(Load("OsdPalette.HighContrast.xaml"));
        label.Resources = stack;

        Assert.Equal("usb out", label.Text);
    }

    /// <summary>
    /// Brand names must not take Turkish casing: under tr-TR "Voicemeeter" would become
    /// "VOİCEMEETER". Runs under tr-TR explicitly so it means the same on every machine.
    /// </summary>
    [StaFact]
    public void LoftLabel_UppercasesBrandNamesWithoutTurkishCasing()
    {
        var saved = System.Globalization.CultureInfo.CurrentCulture;
        try
        {
            System.Globalization.CultureInfo.CurrentCulture = new System.Globalization.CultureInfo("tr-TR");
            var label = new LoftLabel { Case = "Upper", Text = "Voicemeeter" };

            Assert.Equal("VOICEMEETER", label.Text);
            Assert.NotEqual("VOİCEMEETER", label.Text);

            label.Text = "Logitech G733";
            Assert.Equal("LOGITECH G733", label.Text);
        }
        finally
        {
            System.Globalization.CultureInfo.CurrentCulture = saved;
        }
    }

    [StaFact]
    public void LoftLabel_TreatsNullTextAsEmpty()
    {
        var label = new LoftLabel { Case = "Upper", Text = "x" };

        label.Text = null!;

        Assert.Equal(string.Empty, label.Text);
    }
}
