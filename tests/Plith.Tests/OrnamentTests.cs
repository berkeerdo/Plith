using System.IO;
using System.Windows;
using System.Windows.Controls;
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
}
