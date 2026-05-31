using System.Security.Cryptography.X509Certificates;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class CertServiceTests : IDisposable
{
    private readonly TempCertStore _certCleanup = new();

    public void Dispose()
    {
        _certCleanup.Dispose();
        GC.SuppressFinalize(this);
    }

    [Fact]
    public void EnsureCert_creates_cert_when_none_exists_and_returns_thumbprint()
    {
        TempCertStore.RemoveAll();   // ensure clean slate
        var svc = new CertService(subjectName: TempCertStore.TestSubject);

        var thumbprint = svc.EnsureCert();

        Assert.NotNull(thumbprint);
        Assert.Equal(40, thumbprint.Length);   // SHA-1 thumbprint hex = 40 chars

        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadOnly);
        var found = store.Certificates.Find(X509FindType.FindByThumbprint, thumbprint, validOnly: false);
        Assert.Single(found);
    }

    [Fact]
    public void EnsureCert_returns_existing_thumbprint_when_cert_already_present()
    {
        TempCertStore.RemoveAll();
        var svc = new CertService(subjectName: TempCertStore.TestSubject);
        var first = svc.EnsureCert();

        var second = svc.EnsureCert();

        Assert.Equal(first, second);
    }
}
