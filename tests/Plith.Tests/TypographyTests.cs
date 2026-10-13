using System.Windows;
using System.Windows.Media;

namespace Plith.Tests;

public class TypographyTests
{
    private static ResourceDictionary LoadTheme() =>
        (ResourceDictionary)Application.LoadComponent(
            new Uri("/Plith;component/Resources/Theme.xaml", UriKind.Relative));

    [StaFact]
    public void Typography_FamilyNamesMatchTheShippedFonts()
    {
        var theme = LoadTheme();

        // The pack URI cannot be resolved from the test process (Application.ResourceAssembly is
        // the test host, not Plith), so this pins the other half: the family name written in
        // Theme.xaml must be one the shipped .ttf files really report. A wrong name falls back
        // silently at runtime. The trailing separator on the directory is required.
        var fontsDir = System.IO.Path.Combine(AppContext.BaseDirectory, "Fonts") + System.IO.Path.DirectorySeparatorChar;
        var shipped = Fonts.GetFontFamilies(fontsDir)
            .Select(f => f.Source[(f.Source.IndexOf('#') + 1)..])
            .ToList();
        Assert.NotEmpty(shipped);

        foreach (var (key, expected) in new[] { ("OsdUiFont", "Public Sans"), ("OsdMonoFont", "Martian Mono Std Rg") })
        {
            var family = Assert.IsType<FontFamily>(theme[key]);
            var chain = family.Source[(family.Source.IndexOf('#') + 1)..]
                .Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);

            Assert.True(chain.Length >= 2, $"{key} names no fallback chain: '{family.Source}'");
            Assert.True(shipped.Contains(chain[0]),
                $"{key} leads with '{chain[0]}', which no shipped font reports. Found: {string.Join(", ", shipped)}");
            Assert.Equal(expected, chain[0]);
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
