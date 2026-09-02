using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Tests.TestHelpers;

/// <summary>
/// Removes leftover Plith test certificates on Dispose.
///
/// Scope matters here. A cert-creating test exercises CertService, and CertService
/// writes to three places: CurrentUser\My plus LocalMachine\Root and
/// LocalMachine\TrustedPublisher. An earlier version of this helper cleaned only
/// CurrentUser\My, so every test run left two certificates behind in the two
/// machine trust stores permanently - 47 of them had piled up by 2026-08-31.
/// Cleanup must cover every store the code under test can write to.
///
/// The LocalMachine stores need elevation. When the tests run unelevated the
/// removal is skipped rather than failing the run, which is safe because an
/// unelevated test could not have written there in the first place.
/// </summary>
public sealed class TempCertStore : IDisposable
{
    public const string TestSubject = "CN=Plith Test " + nameof(TempCertStore);

    private const string SubjectFilter = "Plith Test";

    public void Dispose() => RemoveAll();

    public static void RemoveAll()
    {
        RemoveFrom(StoreName.My, StoreLocation.CurrentUser);
        RemoveFrom(StoreName.Root, StoreLocation.LocalMachine);
        RemoveFrom(StoreName.TrustedPublisher, StoreLocation.LocalMachine);
    }

    private static void RemoveFrom(StoreName storeName, StoreLocation location)
    {
        try
        {
            using var store = new X509Store(storeName, location);
            store.Open(OpenFlags.ReadWrite);
            var toRemove = store.Certificates.Find(
                X509FindType.FindBySubjectName, SubjectFilter, validOnly: false);
            if (toRemove.Count > 0) store.RemoveRange(toRemove);
            store.Close();
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or CryptographicException)
        {
            // Unelevated run: the machine stores are read-only, so nothing was written
            // to them either. Nothing to clean, nothing to report.
        }
    }
}
