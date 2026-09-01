using System;
using Plith.Installer.Services;
using Xunit;

namespace Plith.Installer.Tests;

public class InstallLockedFileExceptionTests
{
    [Fact]
    public void Message_NamesTheLockedFile()
    {
        var ex = new InstallLockedFileException(@"C:\Program Files\Plith\Plith.exe", new IOException("test"));
        Assert.Contains(@"C:\Program Files\Plith\Plith.exe", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Message_SurfacesTrayExitHint()
    {
        // Users always land here because they left Plith running. Make sure the
        // "Exit from the tray" hint stays in the message — that's the fix path we
        // point them at in support conversations and in the CHANGELOG.
        var ex = new InstallLockedFileException("x", new IOException("test"));
        Assert.Contains("tray", ex.Message, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("Exit", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Message_MentionsAntivirusExclusion()
    {
        // Second-line fix when Exit-from-tray doesn't clear it (Norton scanning the
        // fresh binary). The message must name antivirus AND the install path so the
        // user can copy-paste the exclusion.
        var ex = new InstallLockedFileException("x", new IOException("test"));
        Assert.Contains("antivirus", ex.Message, StringComparison.OrdinalIgnoreCase);
        Assert.Contains(@"%ProgramFiles%\Plith", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void InnerException_IsPreserved()
    {
        var inner = new UnauthorizedAccessException("access denied");
        var ex = new InstallLockedFileException("x", inner);
        Assert.Same(inner, ex.InnerException);
    }

    [Fact]
    public void LockedFilePath_IsExposedForCallers()
    {
        var ex = new InstallLockedFileException(@"D:\foo\bar.dll", new IOException("test"));
        Assert.Equal(@"D:\foo\bar.dll", ex.LockedFilePath);
    }
}
