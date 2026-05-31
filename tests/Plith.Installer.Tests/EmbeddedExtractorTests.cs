using System.IO;
using System.IO.Compression;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class EmbeddedExtractorTests
{
    [Fact]
    public void Extract_writes_zip_contents_to_target_dir()
    {
        using var bundleDir = new TempDirectory();
        using var targetDir = new TempDirectory();

        // Build a fake bundle.zip with a Plith.exe stub.
        var zipPath = Path.Combine(bundleDir.Path, "fake-bundle.zip");
        using (var zipStream = File.Create(zipPath))
        using (var zip = new ZipArchive(zipStream, ZipArchiveMode.Create))
        {
            var entry = zip.CreateEntry("Plith.exe");
            using var writer = new StreamWriter(entry.Open());
            writer.Write("stub");
        }

        var extractor = EmbeddedExtractor.FromStream(File.OpenRead(zipPath));
        extractor.ExtractTo(targetDir.Path);

        Assert.True(File.Exists(Path.Combine(targetDir.Path, "Plith.exe")));
    }

    [Fact]
    public void Extract_throws_when_bundle_lacks_Plith_exe()
    {
        using var bundleDir = new TempDirectory();
        using var targetDir = new TempDirectory();

        var zipPath = Path.Combine(bundleDir.Path, "bad-bundle.zip");
        using (var zipStream = File.Create(zipPath))
        using (var zip = new ZipArchive(zipStream, ZipArchiveMode.Create))
        {
            zip.CreateEntry("README.txt");
        }

        var extractor = EmbeddedExtractor.FromStream(File.OpenRead(zipPath));

        Assert.Throws<InvalidDataException>((Action)(() => extractor.ExtractTo(targetDir.Path)));
    }
}
