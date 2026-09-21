using Plith.DropCatcher.Shelf;
using Plith.Views.Presentation;

namespace Plith.Tests;

public sealed class ShelfModelTests
{
    [Fact]
    public void Select_WithoutAdditiveReplacesTheSelection()
    {
        var model = Loaded();

        model.Select("C:\\a.txt", additive: false);
        model.Select("C:\\b.txt", additive: false);

        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    [Fact]
    public void Select_AdditiveTogglesAndAccumulates()
    {
        var model = Loaded();

        model.Select("C:\\a.txt", additive: true);
        model.Select("C:\\b.txt", additive: true);
        Assert.Equal(2, model.Selection.Count);

        model.Select("C:\\a.txt", additive: true);
        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    /// <summary>
    /// The rule file managers use, and the reason it is here rather than in the window: pressing
    /// a tile inside the selection drags the whole selection, and pressing one outside it drags
    /// that tile alone. Getting this backwards means a person drags four files when they meant
    /// one, which with Copy semantics is recoverable but with any other would not be.
    /// </summary>
    [Fact]
    public void DragPaths_PressInsideTheSelectionCarriesAllOfIt()
    {
        var model = Loaded();
        model.Select("C:\\a.txt", additive: true);
        model.Select("C:\\b.txt", additive: true);

        var paths = model.DragPaths("C:\\a.txt");

        Assert.Equal(2, paths.Count);
    }

    [Fact]
    public void DragPaths_PressOutsideTheSelectionCarriesOnlyThatOne()
    {
        var model = Loaded();
        model.Select("C:\\a.txt", additive: false);

        var paths = model.DragPaths("C:\\b.txt");

        Assert.Equal("C:\\b.txt", Assert.Single(paths));
        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    private static ShelfModel Loaded()
    {
        var model = new ShelfModel();
        model.SetItems(["C:\\a.txt", "C:\\b.txt"]);
        return model;
    }

    /// <summary>One message carries the whole shelf, so there is no assembly to get wrong.
    /// </summary>
    [Fact]
    public void SetItems_ReplacesTheListOutright()
    {
        var model = new ShelfModel();

        model.SetItems(["C:\\a.txt", "C:\\b.txt"]);
        model.SetItems([@"C:\c.txt"]);

        Assert.Equal([@"C:\c.txt"], model.Items.Select(e => e.Path));
    }

    /// <summary>
    /// The pipe's ACL is open to every process on the machine, so the path list is a claim. It is
    /// truncated to what the surface can draw rather than trusted: a message declaring thousands
    /// of paths must not become thousands of tiles.
    /// </summary>
    [Fact]
    public void SetItems_TruncatesToTheShelfCapacity()
    {
        var model = new ShelfModel();
        var many = Enumerable.Range(0, NotchGeometry.ShelfCapacity + 50)
                             .Select(i => $"C:\\f{i}.txt")
                             .ToList();

        model.SetItems(many);

        Assert.Equal(NotchGeometry.ShelfCapacity, model.Items.Count);
    }

    /// <summary>
    /// A path that has gone away since Plith sent it must not stay selected, or a drag would
    /// carry a file that is not on the shelf.
    /// </summary>
    [Fact]
    public void SetItems_DropsASelectionThatIsNoLongerPresent()
    {
        var model = new ShelfModel();
        model.SetItems(["C:\\a.txt", "C:\\b.txt"]);
        model.Select("C:\\a.txt", additive: false);

        model.SetItems(["C:\\b.txt"]);

        Assert.Empty(model.Selection);
    }
}
