using System.Globalization;
using System.Management;
using System.Runtime.InteropServices;
using System.Text;
using Plith.Services.Brightness;

namespace Plith.Services;

/// <summary>
/// A snapshot of what Plith is running on, written into the diagnostic bundle.
///
/// Every line here exists because something was once diagnosed the slow way without it. The
/// session line in particular: inside a Remote Desktop session no physical display is
/// reachable, every DDC/CI call fails, and the symptom is indistinguishable from a monitor
/// that does not support brightness.
/// </summary>
public static class DiagnosticEnvironment
{
    private const int SM_REMOTESESSION = 0x1000;

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int index);

    [DllImport("kernel32.dll")]
    private static extern uint WTSGetActiveConsoleSessionId();

    public static string Describe(SettingsModel settings, DateTime nowUtc)
    {
        var sb = new StringBuilder();
        var inv = CultureInfo.InvariantCulture;

        sb.Append(inv, $"Plith diagnostics{Environment.NewLine}");
        sb.Append(inv, $"Collected: {nowUtc:yyyy-MM-ddTHH:mm:ss.fffZ}{Environment.NewLine}");
        sb.Append(inv, $"Version:   {typeof(DiagnosticEnvironment).Assembly.GetName().Version}{Environment.NewLine}");
        sb.Append(inv, $"OS:        {Environment.OSVersion.VersionString}{Environment.NewLine}");
        sb.Append(inv, $"64-bit OS: {Environment.Is64BitOperatingSystem}{Environment.NewLine}");
        sb.Append(Environment.NewLine);

        AppendSession(sb, inv);
        AppendDisplays(sb, inv);
        AppendInternalPanel(sb, inv);
        AppendSettings(sb, inv, settings);

        return sb.ToString();
    }

    /// <summary>
    /// Console or Remote Desktop, and it matters more than it looks.
    ///
    /// Reconnecting to a logged-in session over RDP moves that session away from the physical
    /// display without changing anything else, so a feature that reads the monitor works one
    /// minute and answers nothing the next.
    /// </summary>
    private static void AppendSession(StringBuilder sb, IFormatProvider inv)
    {
        // Deliberately NOT the SESSIONNAME environment variable. It is copied into a process
        // when the process starts and never updated, so after a session moves between the
        // console and Remote Desktop it keeps reporting where it used to be. Measured: it read
        // "RDP-Tcp#0" next to a live reading of "not remote", which is exactly the kind of
        // contradiction this file exists to prevent.
        var remote = GetSystemMetrics(SM_REMOTESESSION) != 0;

        uint consoleSession;
        try { consoleSession = WTSGetActiveConsoleSessionId(); }
        catch (EntryPointNotFoundException) { consoleSession = uint.MaxValue; }

        using var process = System.Diagnostics.Process.GetCurrentProcess();

        sb.Append(inv, $"Remote session:      {remote}{Environment.NewLine}");
        sb.Append(inv, $"This session id:     {process.SessionId}{Environment.NewLine}");
        sb.Append(inv, $"Console session id:  {(consoleSession == uint.MaxValue ? "unknown" : consoleSession.ToString(inv))}{Environment.NewLine}");

        if (remote)
        {
            sb.Append(inv, $"  NOTE: inside a Remote Desktop session no physical display is reachable.{Environment.NewLine}");
            sb.Append(inv, $"  Every DDC/CI call fails here, so brightness results from this run prove nothing.{Environment.NewLine}");
        }

        sb.Append(Environment.NewLine);
    }

    private static void AppendDisplays(StringBuilder sb, IFormatProvider inv)
    {
        sb.Append(inv, $"Displays answering a DDC/CI brightness read:{Environment.NewLine}");

        IReadOnlyList<IBrightnessDevice> devices;
        try
        {
            devices = BrightnessDiscovery.Discover();
        }
        catch (Exception ex)
        {
            sb.Append(inv, $"  discovery failed: {ex.GetType().Name}: {ex.Message}{Environment.NewLine}{Environment.NewLine}");
            return;
        }

        if (devices.Count == 0)
        {
            sb.Append(inv, $"  none{Environment.NewLine}");
        }

        foreach (var device in devices)
        {
            if (device.TryRead(out var reading))
                sb.Append(inv, $"  {device.Id}: min={reading.Min} current={reading.Current} max={reading.Max}{Environment.NewLine}");
            else
                sb.Append(inv, $"  {device.Id}: read failed{Environment.NewLine}");

            (device as IDisposable)?.Dispose();
        }

        sb.Append(Environment.NewLine);
    }

    /// <summary>
    /// Whether this machine has a built-in panel, which is the only kind
    /// WmiMonitorBrightnessEvent covers. On a desktop the sense half of brightness can never
    /// fire, and that is worth stating rather than leaving as an absence.
    /// </summary>
    private static void AppendInternalPanel(StringBuilder sb, IFormatProvider inv)
    {
        try
        {
            using var searcher = new ManagementObjectSearcher(
                new ManagementScope(@"\\.\root\wmi"),
                new ObjectQuery("SELECT * FROM WmiMonitorBrightness"));
            using var results = searcher.Get();

            var count = results.Count;
            sb.Append(inv, $"Internal panel (WmiMonitorBrightness): {count} instance(s){Environment.NewLine}");
            if (count == 0)
                sb.Append(inv, $"  So WmiMonitorBrightnessEvent cannot fire on this machine.{Environment.NewLine}");
        }
        catch (Exception ex)
        {
            sb.Append(inv, $"Internal panel (WmiMonitorBrightness): not available ({ex.GetType().Name}: {ex.Message}){Environment.NewLine}");
        }

        sb.Append(Environment.NewLine);
    }

    /// <summary>
    /// The settings that shape behaviour. No secrets live in this model, and a bundle that
    /// hides the configuration makes the log it ships with harder to read than it needs to be.
    /// </summary>
    private static void AppendSettings(StringBuilder sb, IFormatProvider inv, SettingsModel m)
    {
        sb.Append(inv, $"Settings:{Environment.NewLine}");
        sb.Append(inv, $"  Presentation:        {m.Presentation}{Environment.NewLine}");
        sb.Append(inv, $"  ShowDurationMs:      {m.ShowDurationMs}{Environment.NewLine}");
        sb.Append(inv, $"  AudioSource:         {m.AudioSource}{Environment.NewLine}");
        sb.Append(inv, $"  CompactMode:         {m.CompactMode}{Environment.NewLine}");
        sb.Append(inv, $"  AutoShowOnMedia:     {m.AutoShowOnMedia}{Environment.NewLine}");
        sb.Append(inv, $"  HideDuringFullscreenVideo: {m.HideDuringFullscreenVideo}{Environment.NewLine}");
        sb.Append(inv, $"  BrightnessEnabled:   {m.BrightnessEnabled}{Environment.NewLine}");
        sb.Append(inv, $"  BrightnessStep:      {m.BrightnessStepPercent}%{Environment.NewLine}");
        sb.Append(inv, $"  Brighter hotkey:     {Combo(m.BrightnessUpHotkeyMods, m.BrightnessUpHotkeyKey)}{Environment.NewLine}");
        sb.Append(inv, $"  Dimmer hotkey:       {Combo(m.BrightnessDownHotkeyMods, m.BrightnessDownHotkeyKey)}{Environment.NewLine}");
        sb.Append(inv, $"  Summon hotkey:       {Combo(m.SummonHotkeyMods, m.SummonHotkeyKey)}{Environment.NewLine}");
    }

    private static string Combo(uint mods, int vk)
    {
        var label = HotkeyService.FormatCombo(mods, vk);
        return string.IsNullOrEmpty(label) ? "(not set)" : label;
    }
}
