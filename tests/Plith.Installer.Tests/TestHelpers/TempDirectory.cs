using System.IO;

namespace Plith.Installer.Tests.TestHelpers;

/// <summary>Disposable temp directory for tests. Deletes itself on Dispose even if
/// the test left files inside. Use with <c>using var dir = new TempDirectory();</c>.</summary>
public sealed class TempDirectory : IDisposable
{
    public string Path { get; }

    public TempDirectory()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(),
            "Plith.Installer.Tests." + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); } catch { /* best-effort */ }
    }
}
