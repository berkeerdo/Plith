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
    public void Message_NamesSingleHolder_WhenRestartManagerFoundOne()
    {
        // Restart Manager reported one process holding the file — the message should
        // quote it verbatim so the user sees who to close instead of guessing.
        var ex = new InstallLockedFileException(
            @"C:\Program Files\Plith\Plith.exe",
            new[] { "Norton Security" },
            new IOException("test"));
        Assert.Contains("'Norton Security' has it open", ex.Message, StringComparison.Ordinal);
        Assert.Equal(new[] { "Norton Security" }, ex.Holders);
    }

    [Fact]
    public void Message_ListsMultipleHolders()
    {
        var ex = new InstallLockedFileException(
            @"C:\Program Files\Plith\Plith.exe",
            new[] { "Norton Security", "Windows Explorer" },
            new IOException("test"));
        Assert.Contains("Norton Security, Windows Explorer", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Message_FallsBackToGenericHolderClause_WhenListIsEmpty()
    {
        // RM couldn't reach the OS or nothing was reported. The message should still
        // hand the user their fix path — no naked "unknown" wording.
        var ex = new InstallLockedFileException(@"x", Array.Empty<string>(), new IOException("test"));
        Assert.Contains("Nobody appears to be holding it", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Message_SwapsToPermissionsAdvice_WhenAccessDeniedWithNoHolders()
    {
        // The 0.1.4 install-fail case: nothing held the file per Restart Manager, but
        // File.Copy still hit UnauthorizedAccessException — that shape means a
        // filesystem-level protection (AV quarantine ACL, ReadOnly attribute). The
        // message must switch away from "close it in tray" (which won't help) to
        // the takeown / icacls recovery path.
        var ex = new InstallLockedFileException(
            @"C:\Program Files\Plith\Plith.deps.json",
            Array.Empty<string>(),
            "ERROR_ACCESS_DENIED (permissions / ACL / read-only)",
            new IOException("test"));
        Assert.Contains("takeown", ex.Message, StringComparison.Ordinal);
        Assert.Contains("icacls", ex.Message, StringComparison.Ordinal);
        Assert.Contains("ERROR_ACCESS_DENIED", ex.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void Win32ErrorDescription_IsExposed()
    {
        var ex = new InstallLockedFileException("x", Array.Empty<string>(), "ERROR_SHARING_VIOLATION", new IOException("test"));
        Assert.Equal("ERROR_SHARING_VIOLATION", ex.Win32ErrorDescription);
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
