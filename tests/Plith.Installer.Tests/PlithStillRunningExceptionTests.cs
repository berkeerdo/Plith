using Plith.Installer.Services;
using Xunit;

namespace Plith.Installer.Tests;

public class PlithStillRunningExceptionTests
{
    [Fact]
    public void Message_ExposesTheProcessCount()
    {
        var ex = new PlithStillRunningException(3);
        Assert.Contains("3", ex.Message, System.StringComparison.Ordinal);
        Assert.Equal(3, ex.SurvivingProcessCount);
    }

    [Fact]
    public void Message_HandsTheUserAConcreteFix()
    {
        // Every branch of the "why couldn't we kill it?" tree — hidden tray icon,
        // multiple instances, whatever — resolves to the same user action: kill it
        // from the tray or Task Manager. Test that both hints stay in the message.
        var ex = new PlithStillRunningException(1);
        Assert.Contains("tray", ex.Message, System.StringComparison.OrdinalIgnoreCase);
        Assert.Contains("Task Manager", ex.Message, System.StringComparison.Ordinal);
    }
}
