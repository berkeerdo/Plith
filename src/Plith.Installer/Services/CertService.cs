using System.IO;
using System.Reflection;
using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Services;

/// <summary>
/// Registers the embedded developer code-signing certificate into LocalMachine trust
/// stores so the pre-signed Plith.exe validates on the end user's machine. This
/// replaces the previous per-user cert-generation + signtool pipeline: signing now
/// happens once at build time, and installs no longer need the Windows SDK.
///
/// The .cer is bundled as an EmbeddedResource (LogicalName 'plith-cert.cer') by
/// Plith.Installer.csproj's PublishPlithAndEmbed target. Adding it to both
/// LocalMachine\Root and LocalMachine\TrustedPublisher is required for the UIAccess
/// chain to validate (Phase 4h lessons learned: TrustedPublisher alone is not enough).
/// </summary>
public sealed class CertService
{
    public const string EmbeddedCertResourceName = "plith-cert.cer";

    private readonly Assembly _resourceAssembly;

    public CertService()
        : this(typeof(CertService).Assembly) { }

    /// <summary>Test seam - inject an alternate assembly whose resources contain a test cert.</summary>
    public CertService(Assembly resourceAssembly)
    {
        _resourceAssembly = resourceAssembly;
    }

    /// <summary>Load the embedded public cert, ensure it's present in LocalMachine\Root
    /// and LocalMachine\TrustedPublisher, and return its SHA-1 thumbprint for logging.
    /// Idempotent: repeated calls are cheap because the store lookups short-circuit.</summary>
    public string InstallTrust()
    {
        var cert = LoadEmbeddedCert();
        EnsureInLocalMachineStore(cert, StoreName.TrustedPublisher);
        EnsureInLocalMachineStore(cert, StoreName.Root);
        return cert.Thumbprint!;
    }

    private X509Certificate2 LoadEmbeddedCert()
    {
        using var stream = _resourceAssembly.GetManifestResourceStream(EmbeddedCertResourceName)
            ?? throw new InvalidOperationException(
                $"Embedded resource '{EmbeddedCertResourceName}' not found. " +
                "The build-time signing target (PublishPlithAndEmbed -> sign-plith.ps1) " +
                "did not produce Resources/Embedded/plith-cert.cer.");
        using var ms = new MemoryStream();
        stream.CopyTo(ms);
        return X509CertificateLoader.LoadCertificate(ms.ToArray());
    }

    private static void EnsureInLocalMachineStore(X509Certificate2 cert, StoreName storeName)
    {
        using var store = new X509Store(storeName, StoreLocation.LocalMachine);
        store.Open(OpenFlags.ReadWrite);
        var existing = store.Certificates.Find(X509FindType.FindByThumbprint, cert.Thumbprint!, validOnly: false);
        if (existing.Count > 0) return;
        store.Add(cert);
    }
}
