using System.Runtime.InteropServices;

namespace Plith.Services;

/// <summary>
/// Changes Windows' default render endpoint.
///
/// There is NO documented API for this. Windows' own Sound control panel uses an undocumented
/// COM interface, IPolicyConfig, and so does every application that offers the feature. The
/// GUIDs below are the ones docs/ROADMAP.md recorded when this was first measured here, during
/// the System Controls work, where SetDefaultEndpoint returned S_OK. That code was never
/// committed: searching the whole history for IPolicyConfig, PolicyConfig, 870af99c and
/// SetDefaultEndpoint finds only prose, so this is a rewrite of something that had only ever
/// been proved by a probe. It is proved again by scripts/probe-output-switch.ps1.
///
/// The risk is bounded rather than absent: an interface with no contract behind it can break on
/// any Windows release, and when it does <see cref="TrySetDefault"/> returns false, the picker
/// says so on screen, and the door to Windows' own sound settings still works.
/// </summary>
public static class OutputDeviceSwitcher
{
    /// <summary>
    /// Make <paramref name="endpointId"/> the default output.
    ///
    /// Console and Multimedia only. That pair is what Windows' own "Set as Default Device"
    /// writes, while "Set as Default Communication Device" is a separate action there. Moving the
    /// communications role would move Discord and Teams audio, which nobody asked this for.
    /// </summary>
    public static bool TrySetDefault(string endpointId, DiagnosticLog? log = null)
    {
        if (string.IsNullOrWhiteSpace(endpointId)) return false;

        object? instance = null;
        try
        {
            var type = Type.GetTypeFromCLSID(PolicyConfigClsid, throwOnError: false);
            if (type is null)
            {
                log?.Warn("OutputDeviceSwitcher", "PolicyConfig is not registered on this system.");
                return false;
            }

            instance = Activator.CreateInstance(type);
            if (instance is not IPolicyConfig config)
            {
                log?.Warn("OutputDeviceSwitcher",
                          "PolicyConfig does not implement the expected interface.");
                return false;
            }

            // eConsole = 0, eMultimedia = 1. eCommunications = 2 is deliberately not written.
            var console = config.SetDefaultEndpoint(endpointId, 0);
            var multimedia = config.SetDefaultEndpoint(endpointId, 1);

            if (console != 0 || multimedia != 0)
            {
                log?.Warn("OutputDeviceSwitcher",
                          $"SetDefaultEndpoint failed: console=0x{console:X8}, multimedia=0x{multimedia:X8}");
                return false;
            }

            log?.Info("OutputDeviceSwitcher", "Default output set, console and multimedia.");
            return true;
        }
        catch (Exception ex)
        {
            log?.Warn("OutputDeviceSwitcher", $"SetDefaultEndpoint threw: {ExceptionText.Describe(ex)}");
            return false;
        }
        finally
        {
            if (instance is not null && Marshal.IsComObject(instance))
                Marshal.ReleaseComObject(instance);
        }
    }

    private static readonly Guid PolicyConfigClsid = new("870af99c-171d-4f9e-af0d-e63df40c2bc9");

    /// <summary>
    /// The undocumented interface, declared only as far as the one method this needs.
    ///
    /// The TEN reserved slots are LOAD-BEARING, and the count is not a guess: it was measured.
    /// A COM interface is a vtable, so every method ahead of SetDefaultEndpoint has to be
    /// declared for the call to land on the right function pointer. Declaring nine compiled
    /// perfectly and returned 0x800706F4, RPC_X_NULL_REF_POINTER, on the first probe, because
    /// the call landed one slot early on SetPropertyValue, whose second parameter is a
    /// PROPERTYKEY reference and received a role number instead.
    ///
    /// The order, from the reverse-engineered header this interface is known by: GetMixFormat,
    /// GetDeviceFormat, ResetDeviceFormat, SetDeviceFormat, GetProcessingPeriod,
    /// SetProcessingPeriod, GetShareMode, SetShareMode, GetPropertyValue, SetPropertyValue,
    /// then SetDefaultEndpoint, then SetEndpointVisibility.
    ///
    /// They return int and take nothing, so no marshalling is attempted for signatures that are
    /// deliberately incomplete: none of them is ever called, and only their COUNT matters.
    /// </summary>
    [ComImport]
    [Guid("f8679f50-850a-41cf-9c72-430f290290c8")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IPolicyConfig
    {
        [PreserveSig] int GetMixFormat_Reserved();
        [PreserveSig] int GetDeviceFormat_Reserved();
        [PreserveSig] int ResetDeviceFormat_Reserved();
        [PreserveSig] int SetDeviceFormat_Reserved();
        [PreserveSig] int GetProcessingPeriod_Reserved();
        [PreserveSig] int SetProcessingPeriod_Reserved();
        [PreserveSig] int GetShareMode_Reserved();
        [PreserveSig] int SetShareMode_Reserved();
        [PreserveSig] int GetPropertyValue_Reserved();
        [PreserveSig] int SetPropertyValue_Reserved();

        [PreserveSig]
        int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string deviceId, uint role);
    }
}
