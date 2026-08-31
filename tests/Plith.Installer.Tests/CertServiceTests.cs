using System.Reflection;
using Plith.Installer.Services;
using Xunit;

namespace Plith.Installer.Tests;

// Trust-store side effects (Root + TrustedPublisher install) require elevation and are
// verified end-to-end by running the installer itself, not here. Unit tests cover only
// the resource-loading contract.
public class CertServiceTests
{
    [Fact]
    public void InstallTrust_throws_when_embedded_cert_resource_is_missing()
    {
        // xUnit's own assembly does not carry a 'plith-cert.cer' resource, so it makes a
        // safe stand-in for an assembly that lacks the embedded cert.
        var assemblyWithoutCert = typeof(FactAttribute).Assembly;
        var svc = new CertService(assemblyWithoutCert);

        var ex = Assert.Throws<InvalidOperationException>(() => svc.InstallTrust());
        Assert.Contains(CertService.EmbeddedCertResourceName, ex.Message, StringComparison.Ordinal);
    }
}
