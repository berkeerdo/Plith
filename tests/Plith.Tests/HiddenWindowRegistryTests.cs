using Plith.Services;

namespace Plith.Tests;

public sealed class HiddenWindowRegistryTests
{
    /// <summary>
    /// The whole reason this type exists: a window Plith hid must be put back when Plith stops,
    /// or the shell's volume flyout stays invisible for the rest of that shell's life on the
    /// builds where the shell creates it once and only moves it.
    /// </summary>
    [Fact]
    public void RestoreAll_ShowsEveryWindowThatWasHidden()
    {
        var registry = new HiddenWindowRegistry();
        var shown = new List<nint>();

        registry.Hide(1, _ => true);
        registry.Hide(2, _ => true);
        registry.RestoreAll(h => { shown.Add(h); return true; });

        Assert.Equal([1, 2], shown.Order());
    }

    /// <summary>
    /// A window that was ALREADY hidden was hidden by somebody else. Showing it would be this
    /// process revealing a window it never took, which is the same rudeness as the defect this
    /// type fixes. ShowWindow reports the previous visibility, and false means "was not visible".
    /// </summary>
    [Fact]
    public void Hide_DoesNotRecordAWindowThatWasAlreadyHidden()
    {
        var registry = new HiddenWindowRegistry();
        var shown = new List<nint>();

        registry.Hide(7, _ => false);

        Assert.Equal(0, registry.Count);
        registry.RestoreAll(h => { shown.Add(h); return true; });
        Assert.Empty(shown);
    }

    /// <summary>The same window hidden twice is owed one restore, not two.</summary>
    [Fact]
    public void Hide_IsIdempotentForTheSameWindow()
    {
        var registry = new HiddenWindowRegistry();

        registry.Hide(3, _ => true);
        registry.Hide(3, _ => true);

        Assert.Equal(1, registry.Count);
    }

    /// <summary>
    /// Cleared even when the show fails, because a handle that could not be restored is a window
    /// that has gone away, and keeping it would mean retrying forever.
    /// </summary>
    [Fact]
    public void RestoreAll_ForgetsEvenWhenTheWindowIsGone()
    {
        var registry = new HiddenWindowRegistry();
        registry.Hide(5, _ => true);

        registry.RestoreAll(_ => false);

        Assert.Equal(0, registry.Count);
    }

    /// <summary>A second stop must not show anything a second time.</summary>
    [Fact]
    public void RestoreAll_IsSafeToCallTwice()
    {
        var registry = new HiddenWindowRegistry();
        var calls = 0;
        registry.Hide(9, _ => true);

        registry.RestoreAll(_ => { calls++; return true; });
        registry.RestoreAll(_ => { calls++; return true; });

        Assert.Equal(1, calls);
    }
}
