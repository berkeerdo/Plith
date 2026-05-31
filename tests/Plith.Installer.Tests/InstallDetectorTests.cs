using System.IO;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class InstallDetectorTests
{
    [Fact]
    public void GetInstalledVersion_returns_null_when_install_dir_missing()
    {
        using var dir = new TempDirectory();
        var fakeExe = Path.Combine(dir.Path, "MissingPlith.exe");
        var detector = new InstallDetector(fakeExe);

        var version = detector.GetInstalledVersion();

        Assert.Null(version);
    }

    [Fact]
    public void GetInstalledVersion_returns_version_when_exe_present_with_version_info()
    {
        // We can use the currently-running test host as a stand-in — it has FileVersionInfo
        // and exists on disk. Asserting the parsed version is just "not null + non-empty"
        // because we don't control the test host's version.
        var testHostExe = System.Diagnostics.Process.GetCurrentProcess().MainModule!.FileName;
        var detector = new InstallDetector(testHostExe);

        var version = detector.GetInstalledVersion();

        Assert.NotNull(version);
        Assert.False(string.IsNullOrEmpty(version));
    }
}
