using System.Windows.Controls;
using System.Windows.Media;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Tests;

/// <summary>
/// Guards the exact defect that crashed Plith on 2026-09-07, the first time a user hovered
/// the notch on a build carrying the ambient row:
///
///   System.ArgumentException: 'Segoe MDL2 Assets' is not a valid value for property 'FontFamily'
///     at Plith.Views.OsdHost.Reposition()   -- UpdateLayout, loading the card template
///     at Plith.Views.OsdHost.OnNotchHoverChanged(inside)
///
/// The view bound `FontFamily="{x:Static services:WeatherCodeMap.GlyphFontFamilyName}"`, and
/// that constant is a string. A literal `FontFamily="Segoe MDL2 Assets"` attribute works
/// because the XAML parser runs a type converter; {x:Static} hands the property a String
/// OBJECT and DependencyObject.SetValue performs no conversion.
///
/// Nothing caught it. The build compiles {x:Static} happily — the mismatch is a run-time
/// SetValue. check-a11y.ps1 reads the XAML as text and never loads it. The headless suite
/// cannot construct a UserControl, so no template was ever realised. And no agent may launch
/// the app, so the only path to the defect was a human hovering the notch.
///
/// These tests cover what IS reachable from here: that the member the view binds is a real
/// FontFamily, and that it names the same family the glyph-existence test verified against.
/// Both fail if GlyphFont goes back to being a string — the second would not even compile.
///
/// Be precise about the limit: they guard the MEMBER, not the binding. If someone re-points
/// the XAML at WeatherCodeMap.GlyphFontFamilyName — which is the string constant, still there,
/// still needed by the font-existence test — the crash comes straight back and nothing here
/// notices. That is the honest shape of this coverage.
///
/// What they do NOT cover, stated plainly rather than implied:
///
/// 1. The SetValue path itself. Asserting that `new TextBlock().SetValue(FontFamilyProperty,
///    GlyphFont)` succeeds would be the closest possible test, and it cannot run here:
///    constructing any FrameworkElement throws "The calling thread must be STA" on this
///    suite's threads. That was tried and removed rather than left as a skipped stub.
/// 2. Any OTHER {x:Static} type mismatch, in this view or another. That whole class needs the
///    markup to be loaded, which this suite cannot do at all.
///
/// An STA XAML-load harness was attempted for both and abandoned: it could not resolve the
/// app's merged resource dictionaries from a PowerShell runspace, and shipping a gate that
/// reports resource-resolution noise instead of defects would be worse than not having one.
/// The gap is recorded in docs/PHASE6-VERIFICATION.md instead of being papered over.
/// </summary>
public class AmbientGlyphFontTests
{
    [Fact]
    public void GlyphFont_IsAFontFamily_NotItsName()
    {
        // The regression in one line: binding the NAME is what threw.
        Assert.IsType<FontFamily>(AmbientCardViewModel.GlyphFont);
    }

    [Fact]
    public void GlyphFont_NamesTheSameFamilyTheGlyphExistenceTestChecksAgainst()
    {
        // The reason the view binds a shared member at all: the font the glyphs are DRAWN in
        // must be the font WeatherCodeMapTests verified those code points EXIST in. Two
        // literals in two files would drift and nothing would notice.
        Assert.Contains(
            WeatherCodeMap.GlyphFontFamilyName,
            AmbientCardViewModel.GlyphFont.FamilyNames.Values);
    }
}
