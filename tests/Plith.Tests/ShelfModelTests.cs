using Plith.DropCatcher.Shelf;

namespace Plith.Tests;

public sealed class ShelfModelTests
{
    /// <summary>Stacks arrive one message at a time, so the model has to know when it has them
    /// all. Painting a half-delivered shelf would show a person a shelf that is missing rows.
    /// </summary>
    [Fact]
    public void SetStack_IsNotCompleteUntilEveryStackHasArrived()
    {
        var model = new ShelfModel();

        model.SetStack(0, 2, ["C:\\a.txt"]);
        Assert.False(model.IsComplete);

        model.SetStack(1, 2, ["C:\\b.txt"]);
        Assert.True(model.IsComplete);
        Assert.Equal(2, model.Stacks.Count);
    }

    [Fact]
    public void SetStack_WithZeroStacksIsAnEmptyShelfAndIsComplete()
    {
        var model = new ShelfModel();

        model.SetStack(0, 0, []);

        Assert.True(model.IsComplete);
        Assert.Empty(model.Stacks);
    }

    /// <summary>A second delivery replaces the first rather than adding to it. The shelf is
    /// re-sent whole on every change, so an appending model would double every row.</summary>
    [Fact]
    public void SetStack_StartingOverReplacesWhatWasThere()
    {
        var model = new ShelfModel();
        model.SetStack(0, 1, ["C:\\a.txt"]);

        model.SetStack(0, 1, ["C:\\b.txt"]);

        Assert.Equal("C:\\b.txt", Assert.Single(Assert.Single(model.Stacks)).Path);
    }

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

    /// <summary>
    /// Index 1 arriving before index 0 is exactly the reordering DropChannelServer's
    /// fire-and-forget sends can produce (see SetStack's own comment). The model has no way yet
    /// to know the set's shape when this happens, so it must not guess a position for it; it
    /// waits rather than placing the message somewhere it might not belong.
    /// </summary>
    [Fact]
    public void SetStack_ArrivingOutOfOrderDoesNotProduceAMisplacedStack()
    {
        var model = new ShelfModel();

        model.SetStack(1, 2, ["C:\\b.txt"]);
        Assert.False(model.IsComplete);
        Assert.Empty(model.Stacks);

        model.SetStack(0, 2, ["C:\\a.txt"]);
        Assert.False(model.IsComplete);

        model.SetStack(1, 2, ["C:\\b.txt"]);
        Assert.True(model.IsComplete);
        Assert.Equal("C:\\a.txt", model.Stacks[0][0].Path);
        Assert.Equal("C:\\b.txt", model.Stacks[1][0].Path);
    }

    /// <summary>
    /// A message from a delivery that has already been superseded by a new one must not be
    /// folded into the new, smaller delivery just because it happens to arrive after that
    /// delivery's own index-0 message. Its stale `total` (2, from the old delivery) no longer
    /// matches what the model now expects (1, from the new one), which is what lets this be
    /// recognised as not belonging here rather than accepted as if it still applied.
    /// </summary>
    [Fact]
    public void SetStack_StaleMessageFromAPreviousSetIsIgnored()
    {
        var model = new ShelfModel();
        model.SetStack(0, 2, ["C:\\a.txt"]);
        model.SetStack(1, 2, ["C:\\b.txt"]);
        Assert.True(model.IsComplete);

        model.SetStack(0, 1, ["C:\\c.txt"]);
        Assert.True(model.IsComplete);

        model.SetStack(1, 2, ["C:\\b.txt"]);

        Assert.True(model.IsComplete);
        Assert.Equal("C:\\c.txt", Assert.Single(model.Stacks)[0].Path);
    }

    private static ShelfModel Loaded()
    {
        var model = new ShelfModel();
        model.SetStack(0, 1, ["C:\\a.txt", "C:\\b.txt"]);
        return model;
    }
}
