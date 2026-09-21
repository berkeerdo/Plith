using Plith.Services.Shelf;

namespace Plith.Tests;

public class ShelfLabelTests
{
    [Fact]
    public void ShortDropsTheExtensionBecauseThePictureAlreadySaysIt()
    {
        Assert.Equal("report", ShelfLabel.Short("report.xlsx", 20));
    }

    [Fact]
    public void ShortKeepsAShortNameWhole()
    {
        Assert.Equal("notes", ShelfLabel.Short("notes.md", 10));
    }

    [Fact]
    public void ShortTrimsTheMIDDLESoBothENDSSurvive()
    {
        // The rule this type exists for. Files of one type on one shelf are usually one export
        // series, and what separates them is the tail: trimmed from the right, every file in the
        // series reads the same.
        var label = ShelfLabel.Short("NM_Mukellef_Veri_Dosyasi_2026-09-21.xlsx", 11);

        Assert.Equal(11, label.Length);
        Assert.StartsWith("NM_", label);
        Assert.EndsWith("9-21", label);
        Assert.Contains("…", label);
    }

    [Fact]
    public void TwoFilesInOneSeriesGetDIFFERENTCaptions()
    {
        // The report, expressed as an assertion: with the same type and the same prefix, hovering
        // was the only way to tell two files apart.
        var first = ShelfLabel.Short("NM_Mukellef_Veri_Dosyasi_2026-09-21.xlsx", 11);
        var second = ShelfLabel.Short("NM_Mukellef_Veri_Dosyasi_2026-09-22.xlsx", 11);

        Assert.NotEqual(first, second);
    }

    [Fact]
    public void ShortGivesTheODDCharacterToTheHead()
    {
        // A name is read from the left, so when the budget cannot be split evenly the start gets
        // the extra character.
        var label = ShelfLabel.Short("abcdefghijklmnop.txt", 6);

        Assert.Equal(6, label.Length);
        Assert.Equal("abc…op", label);
    }

    [Fact]
    public void ANameThatIsNOTHINGBUTAnExtensionKeepsIt()
    {
        // Path.GetFileNameWithoutExtension returns empty here, and a caption of nothing is worse
        // than a caption of the extension.
        Assert.Equal(".gitignore", ShelfLabel.Short(".gitignore", 20));
    }

    [Fact]
    public void ATrailingDotIsNotAnExtension()
    {
        Assert.Equal("report.", ShelfLabel.Short("report.", 20));
    }

    [Fact]
    public void AFolderNameWithADotInItKeepsWhatMatters()
    {
        // A folder called "v2.1 assets" has no extension, but the rule cannot know that from the
        // string alone. Documented rather than special-cased: the caller passes a name, and a
        // dotted folder loses its tail. The full name is in the tooltip and in the hover line.
        Assert.Equal("v2", ShelfLabel.Short("v2.1 assets", 20));
    }

    [Fact]
    public void EmptyAndZeroBudgetAreEmpty()
    {
        Assert.Equal(string.Empty, ShelfLabel.Short("", 10));
        Assert.Equal(string.Empty, ShelfLabel.Short("report.xlsx", 0));
    }
}
