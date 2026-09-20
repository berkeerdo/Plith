namespace Plith.Services;

/// <summary>
/// Opens Windows' own sound settings.
///
/// This is what the media page's output control does, and the compromise is deliberate: changing
/// the default render endpoint has no documented API at all, only the undocumented IPolicyConfig
/// COM interface. An in-notch device list is its own slice, and until it exists the honest thing
/// is to hand the person the surface Windows does provide.
///
/// ms-settings:sound rather than the quick settings output picker, which has no documented way
/// in. Returns false rather than throwing, like
/// <see cref="MediaSessionClient.TryOpenSourceApp"/>: a press on a notch control is not worth
/// taking the OSD down for.
/// </summary>
public static class SystemSoundPanel
{
    public static bool TryOpen()
    {
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
            {
                FileName = "ms-settings:sound",
                UseShellExecute = true,
            });
            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }
}
