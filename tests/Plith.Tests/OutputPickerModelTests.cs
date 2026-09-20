using Plith.Services;

namespace Plith.Tests;

public class OutputPickerModelTests
{
    private static WindowsAudioEndpointInfo Ep(string id, string name) => new(id, name);

    private static readonly IReadOnlyList<WindowsAudioEndpointInfo> Five =
    [
        Ep("steam-speakers", "Hoparlor (Steam Streaming Speakers)"),
        Ep("realtek", "Hoparlor (Realtek(R) Audio)"),
        Ep("steam-mic", "Hoparlor (Steam Streaming Microphone)"),
        Ep("nvidia", "PG27AQDM (NVIDIA High)"),
        Ep("g733", "Hoparlor (Logitech G733)"),
    ];

    [Fact]
    public void TheCurrentOutputComesFirst()
    {
        // The one you are on is the anchor: it is what tells you the list is about the thing you
        // are already hearing.
        var cells = OutputPickerModel.Cells(Five, "g733", capacity: 6);

        Assert.Equal("g733", cells[0].Id);
        Assert.True(cells[0].IsCurrent);
    }

    [Fact]
    public void TheRestKeepEnumerationOrder()
    {
        var cells = OutputPickerModel.Cells(Five, "g733", capacity: 6);

        Assert.Equal(["g733", "steam-speakers", "realtek", "steam-mic", "nvidia"],
                     cells.Select(c => c.Id));
    }

    [Fact]
    public void NothingIsMarkedCurrentWhenTheDefaultIsNotInTheList()
    {
        // Reachable: the default id and the enumeration are two separate reads, and a device can
        // be unplugged between them.
        var cells = OutputPickerModel.Cells(Five, "a-device-that-left", capacity: 6);

        Assert.Equal(5, cells.Count);
        Assert.DoesNotContain(cells, c => c.IsCurrent);
        Assert.Equal("steam-speakers", cells[0].Id);
    }

    [Fact]
    public void ExactlyCapacityDrawsEveryDeviceAndNoOverflow()
    {
        var six = Five.Append(Ep("sixth", "Speakers (Sixth Device)")).ToList();

        var cells = OutputPickerModel.Cells(six, "g733", capacity: 6);

        Assert.Equal(6, cells.Count);
        Assert.DoesNotContain(cells, c => c.IsOverflow);
    }

    [Fact]
    public void MoreThanCapacityKeepsTheLastCellForTheDoor()
    {
        // Nothing is hidden behind a count, which is the shelf's most expensive lesson: a folded
        // tile is in no UIA tree at all, so it is invisible to a screen reader and reachable by
        // no key. The overflow is a NAMED cell instead, and five devices are drawn rather than
        // six.
        var seven = Five
            .Append(Ep("sixth", "Speakers (Sixth Device)"))
            .Append(Ep("seventh", "Speakers (Seventh Device)"))
            .ToList();

        var cells = OutputPickerModel.Cells(seven, "g733", capacity: 6);

        Assert.Equal(6, cells.Count);
        Assert.True(cells[^1].IsOverflow);
        Assert.Equal(5, cells.Count(c => !c.IsOverflow));
    }

    [Fact]
    public void TheOverflowCellIsNotADeviceAndCarriesNoId()
    {
        var seven = Five
            .Append(Ep("sixth", "Speakers (Sixth Device)"))
            .Append(Ep("seventh", "Speakers (Seventh Device)"))
            .ToList();

        var overflow = OutputPickerModel.Cells(seven, "g733", capacity: 6)[^1];

        Assert.Equal(string.Empty, overflow.Id);
        Assert.False(overflow.IsCurrent);
        Assert.NotEmpty(overflow.Label);
    }

    [Fact]
    public void TheCurrentOutputSurvivesAnOverflow()
    {
        // It is first, so it cannot be the one that falls off the end. Stated as a test because
        // the opposite would be the worst possible version of this: a picker that hides the
        // device you are listening to.
        var seven = Five
            .Append(Ep("sixth", "Speakers (Sixth Device)"))
            .Append(Ep("seventh", "Speakers (Seventh Device)"))
            .ToList();

        var cells = OutputPickerModel.Cells(seven, "seventh", capacity: 6);

        Assert.Equal("seventh", cells[0].Id);
        Assert.True(cells[0].IsCurrent);
    }

    [Fact]
    public void NoEndpointsIsAnEmptyList()
    {
        // Possible: a machine with every output disabled. The page says so rather than drawing
        // an empty grid.
        Assert.Empty(OutputPickerModel.Cells([], "g733", capacity: 6));
    }
}
