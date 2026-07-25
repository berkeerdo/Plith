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
    // the VM engine finishes its own initialization. Two cases need different patience:
    //   - rc==0 from Login: engine was already running. 5 s of grace is plenty.
    //   - rc==1 from Login: VBVMR JUST LAUNCHED the engine in app mode for us. Empirically
    //     this can take 10-20 s to be ready on a cold boot. Repeatedly Logout/Login during
    //     that window resets the engine and prevents it from ever finishing init — so we
    //     need a long grace AND we don't call Logout when we declare death.
    private static readonly TimeSpan PostLoginGraceAlreadyRunning = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan PostLoginGraceJustLaunched = TimeSpan.FromSeconds(30);
    private const int TransientErrorThreshold = 5;
    private int _consecutiveDirtyErrors;
    private DateTime _graceEndsUtc;

    /// <summary>Last raw return code from VBVMR_Login, surfaced for diagnostics so the
    /// orchestrator log can distinguish "engine already running" (0) from "we launched it"
    /// (1) from error states (-1, -2). int.MinValue means Login was never attempted.</summary>
    public int LastLoginReturnCode { get; private set; } = int.MinValue;

    public bool IsLoggedIn => _loggedIn;

    /// <summary>Returns true when a Voicemeeter installation is present on this machine.
    /// Checked against the same registry + default-path fallback used by the DLL resolver,
    /// plus a probe that the DLL file itself exists so a half-removed install (registry key
    /// left behind but binaries gone) reports false. Cheap enough to call on every settings
    /// window open; result is not cached because the user can install/uninstall between calls.</summary>
    public static bool IsInstalled
    {
        get
        {
            var dir = TryGetVoicemeeterInstallPath();
            return dir is not null && File.Exists(Path.Combine(dir, DllName));
        }
    }

    public bool TryLogin()
    {
        if (_loggedIn) return true;
        EnsureDllResolverRegistered();

        // Defensive logout — if a previous death-detection cycle flipped our cache to false
        // without telling VBVMR (or if some prior state lingers), the next Login can return
        // -2 "unexpected login" forever. Try Logout first so VBVMR's internal state matches.
        try { _ = VBVMR_Logout(); } catch { /* not previously logged in — fine */ }

        var rc = VBVMR_Login();
        LastLoginReturnCode = rc;
        _loggedIn = rc == 0 || rc == 1;
        if (_loggedIn)
        {
            _consecutiveDirtyErrors = 0;
            // rc==1 means VBVMR is launching the engine for us — give it real time before
            // we start counting dirty-check failures against it.
            var grace = rc == 1 ? PostLoginGraceJustLaunched : PostLoginGraceAlreadyRunning;
            _graceEndsUtc = DateTime.UtcNow + grace;
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
    /// app), but on a cold boot it returns -1 transiently for many seconds while the engine
    /// finishes its own initialization. We require <see cref="TransientErrorThreshold"/>
    /// consecutive negative returns AFTER the post-Login grace expires before declaring the
    /// engine dead and dropping the cached login state — the orchestrator's next reconcile pass
    /// will then fall back to the Windows endpoint.</summary>
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
                // 5 consecutive negatives after grace = engine is genuinely gone.
                // We deliberately do NOT call VBVMR_Logout here: calling Logout/Login
                // repeatedly while the engine is mid-initialization (rc==1 case) was
                // observed to interrupt the engine's own init and trap it in a never-ready
                // state, producing a 3-second flap loop. Just flip our cache — the next
                // TryLogin starts with a defensive Logout, which is the only safe place
                // to call it (right before Login, so the new state is consistent).
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
