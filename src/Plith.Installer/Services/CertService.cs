using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Services;

/// <summary>
/// Self-signed code-signing cert lifecycle. EnsureCert returns the thumbprint of a
/// usable cert — reusing an existing CN=Plith Self-Signed entry in CurrentUser\My
/// when present, or generating a new 5-year cert otherwise. Imports the public cert
/// into BOTH LocalMachine\TrustedPublisher AND LocalMachine\Root so the UIAccess
/// chain validates (Phase 4h lessons learned: TrustedPublisher alone is not enough).
/// </summary>
public sealed class CertService
{
    public const string DefaultSubject = "CN=Plith Self-Signed";

    private static readonly Oid CodeSigningOid = new("1.3.6.1.5.5.7.3.3");
    private readonly string _subject;

    public CertService(string? subjectName = null)
    {
        _subject = subjectName ?? DefaultSubject;
    }

    /// <summary>Find or create a usable cert; ensure it's also in LocalMachine\TrustedPublisher
    /// + LocalMachine\Root; return its SHA-1 thumbprint.</summary>
    public string EnsureCert()
    {
        var cert = FindExisting() ?? CreateAndPersist();
        EnsureInLocalMachineStore(cert, StoreName.TrustedPublisher);
        EnsureInLocalMachineStore(cert, StoreName.Root);
        return cert.Thumbprint!;
    }

    private X509Certificate2? FindExisting()
    {
        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadOnly);
        foreach (var existing in store.Certificates)
        {
            if (existing.Subject == _subject && existing.NotAfter > DateTime.UtcNow)
                return existing;
        }
        return null;
    }

    private X509Certificate2 CreateAndPersist()
    {
        using var rsa = RSA.Create(2048);
        var req = new CertificateRequest(_subject, rsa, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1);
        req.CertificateExtensions.Add(new X509KeyUsageExtension(
            X509KeyUsageFlags.DigitalSignature, critical: true));
        req.CertificateExtensions.Add(new X509EnhancedKeyUsageExtension(
            new OidCollection { CodeSigningOid }, critical: true));

        var notBefore = DateTimeOffset.UtcNow.AddMinutes(-5);   // tolerate slight clock skew
        var notAfter = notBefore.AddYears(5);
        var cert = req.CreateSelfSigned(notBefore, notAfter);
        cert.FriendlyName = "Plith Code Signing";

        // Re-load with persisted private key. CreateSelfSigned returns an ephemeral key by
        // default; signing tools and SignTool need the key to be in the user's key store.
        var pfx = cert.Export(X509ContentType.Pfx);
        var persisted = X509CertificateLoader.LoadPkcs12(pfx, password: null,
            X509KeyStorageFlags.PersistKeySet | X509KeyStorageFlags.UserKeySet | X509KeyStorageFlags.Exportable);

        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadWrite);
        store.Add(persisted);
        return persisted;
    }

    private static void EnsureInLocalMachineStore(X509Certificate2 cert, StoreName storeName)
    {
        using var store = new X509Store(storeName, StoreLocation.LocalMachine);
        store.Open(OpenFlags.ReadWrite);
        var existing = store.Certificates.Find(X509FindType.FindByThumbprint, cert.Thumbprint!, validOnly: false);
        if (existing.Count > 0) return;

        // Public-only copy — never put the private key in LocalMachine stores.
        var publicOnly = X509CertificateLoader.LoadCertificate(cert.Export(X509ContentType.Cert));
        store.Add(publicOnly);
    }
}
