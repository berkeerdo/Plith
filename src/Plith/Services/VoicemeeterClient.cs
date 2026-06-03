using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32;

namespace Plith.Services;

public enum VoicemeeterRail { Strip, Bus }

public sealed record VoicemeeterParameterSnapshot(
    VoicemeeterRail Rail,
    int Index,
    string Label,
    float GainDb,
    bool Muted);

/// <summary>
/// Thin P/Invoke wrapper around <c>VoicemeeterRemote64.dll</c>.
/// The DLL ships with the user's Voicemeeter installation; resolved via the
/// HKLM uninstall registry key (matches VB-Audio's recommended discovery path).
/// </summary>
public sealed class VoicemeeterClient : IDisposable
{
    private const string DllName = "VoicemeeterRemote64.dll";

    private bool _loggedIn;
    private bool _disposed;

    // Cold-boot tolerance: VBVMR_IsParametersDirty can return -1 for several seconds while
    // the VM engine finishes its own initialization. We use two guards:
    //   1) Grace period — for the first 3 s after a successful Login, ignore -1 returns
    //      entirely. Don't increment the counter, don't flip _loggedIn.
    //   2) Counter — after the grace period, require 5 consecutive negative returns
    //      (~150 ms at 30 ms poll cadence) before declaring the engine dead.
    private static readonly TimeSpan PostLoginGracePeriod = TimeSpan.FromSeconds(3);
    private const int TransientErrorThreshold = 5;
    private int _consecutiveDirtyErrors;
    private DateTime _graceEndsUtc;

    public bool IsLoggedIn => _loggedIn;

    public bool TryLogin()
    {
        if (_loggedIn) return true;
        EnsureDllResolverRegistered();

        // Defensive logout — if a previous death-detection cycle flipped our cache to false
        // without telling VBVMR (or if some prior state lingers), the next Login can return
        // -2 "unexpected login" forever. Try Logout first so VBVMR's internal state matches.
        try { _ = VBVMR_Logout(); } catch { /* not previously logged in — fine */ }

        var rc = VBVMR_Login();
        _loggedIn = rc == 0 || rc == 1; // 1 = engine already launched in app mode
        if (_loggedIn)
        {
            _consecutiveDirtyErrors = 0;
            _graceEndsUtc = DateTime.UtcNow + PostLoginGracePeriod;
        }
        return _loggedIn;
    }

    public void Logout()
    {
        if (!_loggedIn) return;
        try { _ = VBVMR_Logout(); } catch { /* swallow on shutdown */ }
        _loggedIn = false;
    }

    /// <summary>Non-blocking dirty check; returns true exactly once per parameter mutation batch.
    /// A negative return from the API can mean the Voicemeeter engine went away (user closed the
    /// app), but on a cold boot it returns -1 transiently for ~100-300 ms while the engine finishes
    /// its own initialization. We require <see cref="TransientErrorThreshold"/> consecutive negative
    /// returns before declaring the engine dead and dropping the cached login state — the
    /// orchestrator's next reconcile pass will then fall back to the Windows endpoint.</summary>
    public bool ConsumeDirtyFlag()
    {
        if (!_loggedIn) return false;
        int rc = VBVMR_IsParametersDirty();
        if (rc < 0)
        {
            // Grace period: VM engine is still warming up. Don't penalise transient errors.
            if (DateTime.UtcNow < _graceEndsUtc) return false;

            _consecutiveDirtyErrors++;
            if (_consecutiveDirtyErrors >= TransientErrorThreshold)
            {
                // 5 consecutive negatives after grace = ~150 ms of failure. Engine is
                // genuinely gone. Call Logout to sync VBVMR's internal state with our
                // cache so a future TryLogin can succeed when the engine comes back.
                try { _ = VBVMR_Logout(); } catch { }
                _loggedIn = false;
            }
            return false;
        }
        // Any successful read clears the streak.
        _consecutiveDirtyErrors = 0;
        return rc > 0;
    }

    // VBVMR_GetParameterStringA requires a caller-allocated buffer of at least 512 bytes per
    // VB-Audio's header. Reused across calls — the polling cadence is single-threaded on the UI
    // dispatcher, so no lock is needed.
    private readonly byte[] _labelBuffer = new byte[512];

    public bool TryGetSnapshot(VoicemeeterRail rail, int index, out VoicemeeterParameterSnapshot snapshot)
    {
        snapshot = default!;
        if (!_loggedIn) return false;

        string prefix = rail == VoicemeeterRail.Bus ? $"Bus[{index}]" : $"Strip[{index}]";

        if (VBVMR_GetParameterFloat($"{prefix}.Gain", out float gain) != 0) return false;
        _ = VBVMR_GetParameterFloat($"{prefix}.Mute", out float mute);

        Array.Clear(_labelBuffer);
        _ = VBVMR_GetParameterStringA($"{prefix}.Label", _labelBuffer);
        int nullIdx = Array.IndexOf(_labelBuffer, (byte)0);
        var label = Encoding.Latin1.GetString(_labelBuffer, 0, nullIdx >= 0 ? nullIdx : _labelBuffer.Length);
        if (string.IsNullOrWhiteSpace(label))
            label = rail == VoicemeeterRail.Bus ? $"Bus {BusFriendlyName(index)}" : $"Strip {index + 1}";

        snapshot = new VoicemeeterParameterSnapshot(rail, index, label, gain, mute >= 0.5f);
        return true;
    }

    private static string BusFriendlyName(int index) => index switch
    {
        0 => "A1", 1 => "A2", 2 => "A3", 3 => "A4",
        4 => "A5", 5 => "B1", 6 => "B2", 7 => "B3",
        _ => index.ToString(System.Globalization.CultureInfo.InvariantCulture),
    };

    #region DllImports

    [DllImport(DllName, CallingConvention = CallingConvention.StdCall)]
    private static extern int VBVMR_Login();

    [DllImport(DllName, CallingConvention = CallingConvention.StdCall)]
    private static extern int VBVMR_Logout();

    [DllImport(DllName, CallingConvention = CallingConvention.StdCall)]
    private static extern int VBVMR_IsParametersDirty();

    [DllImport(DllName, CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Ansi, BestFitMapping = false, ThrowOnUnmappableChar = true)]
    private static extern int VBVMR_GetParameterFloat([MarshalAs(UnmanagedType.LPStr)] string paramName, out float value);

    [DllImport(DllName, CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Ansi, BestFitMapping = false, ThrowOnUnmappableChar = true)]
    private static extern int VBVMR_GetParameterStringA(
        [MarshalAs(UnmanagedType.LPStr)] string paramName,
        [In, Out] byte[] szString);

    #endregion

    #region DLL discovery

    private static bool _resolverRegistered;
    private static readonly object _resolverLock = new();

    private static void EnsureDllResolverRegistered()
    {
        if (_resolverRegistered) return;
        lock (_resolverLock)
        {
            if (_resolverRegistered) return;
            NativeLibrary.SetDllImportResolver(typeof(VoicemeeterClient).Assembly, ResolveVoicemeeterDll);
            _resolverRegistered = true;
        }
    }

    private static nint ResolveVoicemeeterDll(string libraryName, System.Reflection.Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (!libraryName.Equals(DllName, StringComparison.OrdinalIgnoreCase))
            return 0;

        var installPath = TryGetVoicemeeterInstallPath();
        if (installPath is not null)
        {
            var candidate = Path.Combine(installPath, DllName);
            if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out var handle))
                return handle;
        }

        // Fall back to default search; the DLL is also in PATH on most installs.
        return NativeLibrary.TryLoad(libraryName, assembly, searchPath, out var fallback) ? fallback : 0;
    }

    private static string? TryGetVoicemeeterInstallPath()
    {
        const string key = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\VB:Voicemeeter {17359A74-1236-5467}";
        using var hklm = Registry.LocalMachine.OpenSubKey(key)
                       ?? Registry.LocalMachine.OpenSubKey(@"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\VB:Voicemeeter {17359A74-1236-5467}");
        if (hklm?.GetValue("UninstallString") is string uninstall)
        {
            var dir = Path.GetDirectoryName(uninstall.Trim('"'));
            if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir)) return dir;
        }

        // Hard-coded fallback for default x86 install location.
        const string defaultPath = @"C:\Program Files (x86)\VB\Voicemeeter";
        return Directory.Exists(defaultPath) ? defaultPath : null;
    }

    #endregion

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Logout();
    }
}
