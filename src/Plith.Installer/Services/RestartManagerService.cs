using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Plith.Installer.Services;

/// <summary>
/// Thin wrapper around Windows Restart Manager (rstrtmgr.dll). This is the same API
/// Windows Installer, Chocolatey, and the OS Update pipeline use to figure out which
/// processes have a target file open and, optionally, to shut them down cleanly so
/// the install can proceed.
///
/// Two entry points:
/// <list type="bullet">
///   <item><see cref="CloseHolders"/> asks RM to send WM_CLOSE (then TerminateProcess if
///     needed) to every non-critical process holding the given files. RM won't touch
///     services or processes marked critical (Norton, Windows Defender, System) — those
///     stay running and their locks stay live.</item>
///   <item><see cref="EnumerateHolders"/> queries without shutting anything down; used to
///     enrich the failure message so the user sees "Norton" or "Explorer" instead of a
///     generic "locked" error.</item>
/// </list>
///
/// Both calls fail-open: any Win32 error returns an empty result / false so the caller
/// falls through to the existing copy path. RM is a best-effort optimisation, not a
/// hard dependency.
/// </summary>
internal static class RestartManagerService
{
    private const int CCH_RM_MAX_APP_NAME = 255;
    private const int CCH_RM_MAX_SVC_NAME = 63;
    private const uint RmForceShutdown = 0x1;

    [StructLayout(LayoutKind.Sequential)]
    private struct RM_UNIQUE_PROCESS
    {
        public int dwProcessId;
        public System.Runtime.InteropServices.ComTypes.FILETIME ProcessStartTime;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct RM_PROCESS_INFO
    {
        public RM_UNIQUE_PROCESS Process;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = CCH_RM_MAX_APP_NAME + 1)]
        public string strAppName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = CCH_RM_MAX_SVC_NAME + 1)]
        public string strServiceShortName;
        public uint ApplicationType;
        public uint AppStatus;
        public uint TSSessionId;
        [MarshalAs(UnmanagedType.Bool)]
        public bool bRestartable;
    }

    [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)]
    private static extern int RmStartSession(out uint pSessionHandle, int dwSessionFlags,
        [MarshalAs(UnmanagedType.LPWStr)] string strSessionKey);

    [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)]
    private static extern int RmRegisterResources(uint pSessionHandle, uint nFiles,
        [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[] rgsFileNames,
        uint nApplications, RM_UNIQUE_PROCESS[]? rgApplications,
        uint nServices,
        [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[]? rgsServiceNames);

    [DllImport("rstrtmgr.dll")]
    private static extern int RmGetList(uint dwSessionHandle, out uint pnProcInfoNeeded,
        ref uint pnProcInfo, [In, Out] RM_PROCESS_INFO[]? rgAffectedApps, ref uint lpdwRebootReasons);

    [DllImport("rstrtmgr.dll")]
    private static extern int RmEndSession(uint pSessionHandle);

    [DllImport("rstrtmgr.dll")]
    private static extern int RmShutdown(uint pSessionHandle, uint lActionFlags, IntPtr fnStatus);

    /// <summary>
    /// Returns the display names of processes currently holding the given files open.
    /// Empty list means either nothing is holding them or RM couldn't reach the OS
    /// service (fail-open). Use this to enrich failure messages so the user sees the
    /// holder by name rather than a raw "locked" error.
    /// </summary>
    public static IReadOnlyList<string> EnumerateHolders(IReadOnlyList<string> files)
    {
        if (files.Count == 0) return Array.Empty<string>();
        var result = new List<string>();

        var sessionKey = Guid.NewGuid().ToString();
        if (RmStartSession(out uint session, 0, sessionKey) != 0) return result;
        try
        {
            var fileArr = new string[files.Count];
            for (int i = 0; i < files.Count; i++) fileArr[i] = files[i];

            if (RmRegisterResources(session, (uint)fileArr.Length, fileArr, 0, null, 0, null) != 0)
                return result;

            uint needed = 0;
            uint count = 0;
            uint reboot = 0;
            _ = RmGetList(session, out needed, ref count, null, ref reboot);
            if (needed == 0) return result;

            count = needed;
            var infos = new RM_PROCESS_INFO[needed];
            if (RmGetList(session, out needed, ref count, infos, ref reboot) != 0) return result;

            for (int i = 0; i < count; i++)
            {
                var name = string.IsNullOrWhiteSpace(infos[i].strAppName)
                    ? $"pid {infos[i].Process.dwProcessId}"
                    : infos[i].strAppName;
                if (!result.Contains(name, StringComparer.OrdinalIgnoreCase))
                    result.Add(name);
            }
        }
        catch (DllNotFoundException) { /* rstrtmgr.dll missing — nothing to do */ }
        catch (EntryPointNotFoundException) { /* older Windows without the API */ }
        finally
        {
            _ = RmEndSession(session);
        }
        return result;
    }

    /// <summary>
    /// Asks Restart Manager to close every non-critical process holding the given
    /// files (WM_CLOSE first, TerminateProcess as fallback). Returns the list of
    /// holders it found (whether or not it could shut them down — services and
    /// processes marked critical stay running).
    /// </summary>
    public static IReadOnlyList<string> CloseHolders(IReadOnlyList<string> files)
    {
        if (files.Count == 0) return Array.Empty<string>();
        var holders = new List<string>();

        var sessionKey = Guid.NewGuid().ToString();
        if (RmStartSession(out uint session, 0, sessionKey) != 0) return holders;
        try
        {
            var fileArr = new string[files.Count];
            for (int i = 0; i < files.Count; i++) fileArr[i] = files[i];

            if (RmRegisterResources(session, (uint)fileArr.Length, fileArr, 0, null, 0, null) != 0)
                return holders;

            uint needed = 0;
            uint count = 0;
            uint reboot = 0;
            _ = RmGetList(session, out needed, ref count, null, ref reboot);
            if (needed > 0)
            {
                count = needed;
                var infos = new RM_PROCESS_INFO[needed];
                if (RmGetList(session, out needed, ref count, infos, ref reboot) == 0)
                {
                    for (int i = 0; i < count; i++)
                    {
                        var name = string.IsNullOrWhiteSpace(infos[i].strAppName)
                            ? $"pid {infos[i].Process.dwProcessId}"
                            : infos[i].strAppName;
                        if (!holders.Contains(name, StringComparer.OrdinalIgnoreCase))
                            holders.Add(name);
                    }
                }
            }

            // RmForceShutdown: escalates to TerminateProcess if the WM_CLOSE fallback
            // hangs. Safe here because RM already skips critical/service processes.
            _ = RmShutdown(session, RmForceShutdown, IntPtr.Zero);
        }
        catch (DllNotFoundException) { }
        catch (EntryPointNotFoundException) { }
        finally
        {
            _ = RmEndSession(session);
        }
        return holders;
    }
}
