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
