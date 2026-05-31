using System.IO;
using System.IO.Compression;
using System.Reflection;

namespace Plith.Installer.Services;

/// <summary>
/// Extracts the embedded PlithBundle.zip resource (the Plith Release publish output) into
/// a target directory. Validates that the extracted output contains Plith.exe so a
/// corrupt or wrong-content bundle fails fast instead of producing a broken install.
/// </summary>
public sealed class EmbeddedExtractor
{
    public const string BundleResourceName = "PlithBundle.zip";

    private readonly Stream _bundleStream;

    private EmbeddedExtractor(Stream bundleStream)
    {
        _bundleStream = bundleStream;
    }

    /// <summary>Factory for production use — pulls the zip from the installer assembly's
    /// embedded resources.</summary>
    public static EmbeddedExtractor FromEmbeddedResource()
    {
        var assembly = Assembly.GetExecutingAssembly();
        var stream = assembly.GetManifestResourceStream(BundleResourceName)
            ?? throw new InvalidOperationException(
                $"Embedded resource '{BundleResourceName}' not found. " +
                "The pre-build target failed to populate Resources/Embedded/.");
        return new EmbeddedExtractor(stream);
    }

    /// <summary>Factory for tests — pass any stream containing a valid bundle.</summary>
    public static EmbeddedExtractor FromStream(Stream stream) => new(stream);

    public void ExtractTo(string targetDir)
    {
        Directory.CreateDirectory(targetDir);
        using (var archive = new ZipArchive(_bundleStream, ZipArchiveMode.Read))
        {
            archive.ExtractToDirectory(targetDir, overwriteFiles: true);
        }

        var plithExe = Path.Combine(targetDir, "Plith.exe");
        if (!File.Exists(plithExe))
            throw new InvalidDataException(
                $"Bundle is missing Plith.exe (expected at '{plithExe}'). " +
                "The pre-build embedding pipeline did not include the main executable.");
    }
}
