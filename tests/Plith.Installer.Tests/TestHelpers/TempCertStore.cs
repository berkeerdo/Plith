using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Tests.TestHelpers;

/// <summary>Removes any leftover Plith test certs from the user's CurrentUser\My store
/// on Dispose. Tests can run safely without polluting the real cert store.</summary>
public sealed class TempCertStore : IDisposable
{
    public const string TestSubject = "CN=Plith Test " + nameof(TempCertStore);

    public void Dispose() => RemoveAll();

    public static void RemoveAll()
    {
        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadWrite);
        var toRemove = store.Certificates.Find(X509FindType.FindBySubjectName,
            "Plith Test", validOnly: false);
        if (toRemove.Count > 0) store.RemoveRange(toRemove);
        store.Close();
    }
}
