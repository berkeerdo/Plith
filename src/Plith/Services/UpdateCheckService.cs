using System.IO;
using System.Net.Http;
using System.Text.Json;
using System.Threading;

namespace Plith.Services;

/// <summary>Result of a single GitHub release check.</summary>
public sealed record UpdateInfo(
    bool IsAvailable,
    string CurrentVersion,
    string LatestVersion,
    string ReleaseName,
    string ReleasePageUrl,
    string? InstallerAssetUrl,
    long InstallerAssetSize);

/// <summary>
/// Checks GitHub's Releases API for a newer Plith build and can download the setup .exe
/// asset to a temp path. Deliberately narrow: no auto-install, no background polling — the
/// caller (SettingsWindow) drives every request. Startup can drive a throttled background
/// check separately by calling <see cref="CheckAsync"/> from a Task.
/// </summary>
public sealed class UpdateCheckService
{
    private const string GitHubReleasesUrl = "https://api.github.com/repos/berkeerdo/Plith/releases/latest";
    private const string InstallerAssetPrefix = "Plith-Setup-";

    private readonly DiagnosticLog? _log;

    public UpdateCheckService(DiagnosticLog? log = null) { _log = log; }

    /// <summary>Query the API. Returns null on network / parse failure (logged, never thrown to caller).</summary>
    public async Task<UpdateInfo?> CheckAsync(CancellationToken ct = default)
    {
        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(15) };
            // GitHub requires a User-Agent header on every API request.
            http.DefaultRequestHeaders.UserAgent.ParseAdd("Plith-UpdateCheck");
            http.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");

            var json = await http.GetStringAsync(GitHubReleasesUrl, ct).ConfigureAwait(false);
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;

            var tag = root.GetProperty("tag_name").GetString() ?? string.Empty;
            var name = TryGetString(root, "name") ?? tag;
            var pageUrl = TryGetString(root, "html_url") ?? string.Empty;

            string? assetUrl = null;
            long assetSize = 0;
            if (root.TryGetProperty("assets", out var assets) && assets.ValueKind == JsonValueKind.Array)
            {
                foreach (var asset in assets.EnumerateArray())
                {
                    var an = TryGetString(asset, "name") ?? string.Empty;
                    if (an.StartsWith(InstallerAssetPrefix, StringComparison.OrdinalIgnoreCase)
                        && an.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
                    {
                        assetUrl = TryGetString(asset, "browser_download_url");
                        assetSize = TryGetInt64(asset, "size");
                        break;
                    }
                }
            }

            var latestVersion = ParseVersion(tag);
            var currentVersion = typeof(UpdateCheckService).Assembly.GetName().Version ?? new Version(0, 0, 0);
            var currentNormalised = new Version(currentVersion.Major, currentVersion.Minor, Math.Max(0, currentVersion.Build));
            bool newer = latestVersion is not null && latestVersion.CompareTo(currentNormalised) > 0;

            var info = new UpdateInfo(
                IsAvailable: newer,
                CurrentVersion: currentNormalised.ToString(3),
                LatestVersion: latestVersion?.ToString(3) ?? tag,
                ReleaseName: name,
                ReleasePageUrl: pageUrl,
                InstallerAssetUrl: assetUrl,
                InstallerAssetSize: assetSize);

            _log?.Info("UpdateCheck",
                $"Latest={info.LatestVersion} Current={info.CurrentVersion} Newer={newer} Asset={(assetUrl is null ? "none" : "yes")}");
            return info;
        }
        catch (Exception ex)
        {
            _log?.Warn("UpdateCheck", $"check failed: {ex.GetType().Name}: {ex.Message}");
            return null;
        }
    }

    /// <summary>Downloads the installer asset to a fresh temp file. Reports byte progress
    /// through <paramref name="progress"/>. Returns the local path on success; null on failure.</summary>
    public async Task<string?> DownloadInstallerAsync(string url, long expectedSize,
        IProgress<double>? progress = null, CancellationToken ct = default)
    {
        try
        {
            var tempDir = Path.Combine(Path.GetTempPath(), "Plith-Update");
            Directory.CreateDirectory(tempDir);
            var fileName = Path.GetFileName(new Uri(url).LocalPath);
            if (string.IsNullOrWhiteSpace(fileName)) fileName = "Plith-Setup.exe";
            var target = Path.Combine(tempDir, fileName);

            using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(10) };
            http.DefaultRequestHeaders.UserAgent.ParseAdd("Plith-UpdateCheck");

            using var resp = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false);
            resp.EnsureSuccessStatusCode();

            var total = resp.Content.Headers.ContentLength ?? expectedSize;
            using var source = await resp.Content.ReadAsStreamAsync(ct).ConfigureAwait(false);
            using var dest = new FileStream(target, FileMode.Create, FileAccess.Write, FileShare.None);

            var buffer = new byte[81920];
            long received = 0;
            int read;
            while ((read = await source.ReadAsync(buffer.AsMemory(0, buffer.Length), ct).ConfigureAwait(false)) > 0)
            {
                await dest.WriteAsync(buffer.AsMemory(0, read), ct).ConfigureAwait(false);
                received += read;
                if (total > 0) progress?.Report((double)received / total);
            }

            _log?.Info("UpdateCheck", $"Downloaded {received} bytes to {target}");
            return target;
        }
        catch (Exception ex)
        {
            _log?.Warn("UpdateCheck", $"download failed: {ex.GetType().Name}: {ex.Message}");
            return null;
        }
    }

    // Accepts 'v0.1.2', '0.1.2', 'v0.1.2-rc1'. Ignores prerelease suffix — Plith releases don't
    // ship prereleases as separate versions on GitHub yet, so any parse failure just returns null.
    private static Version? ParseVersion(string tag)
    {
        if (string.IsNullOrWhiteSpace(tag)) return null;
        var trimmed = tag.TrimStart('v', 'V');
        var dash = trimmed.IndexOf('-');
        if (dash > 0) trimmed = trimmed.Substring(0, dash);
        return Version.TryParse(trimmed, out var v)
            ? new Version(v.Major, v.Minor, Math.Max(0, v.Build))
            : null;
    }

    private static string? TryGetString(JsonElement el, string prop)
        => el.TryGetProperty(prop, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static long TryGetInt64(JsonElement el, string prop)
        => el.TryGetProperty(prop, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetInt64() : 0;
}
