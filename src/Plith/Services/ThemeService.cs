using System.Linq;
using System.Windows;
using System.Windows.Threading;
using Microsoft.Win32;

namespace Plith.Services;

/// <summary>
/// Owns the Settings window theme. Swaps the active palette ResourceDictionary
/// in <see cref="Application.Resources"/> between <c>Palette.Dark.xaml</c> and
/// <c>Palette.Light.xaml</c> in response to <see cref="SettingsModel.Theme"/> or
/// the Windows apps-use-light-theme preference (for <see cref="ThemeMode.Auto"/>).
/// Every Settings XAML brush is bound via <c>DynamicResource</c>, so the swap
/// propagates without re-creating any windows. The OSD overlay theme is independent
/// and intentionally stays dark.
/// </summary>
public sealed class ThemeService : IDisposable
{
    private static readonly Uri SettingsDarkUri = new(
        "pack://application:,,,/Resources/Palette.Dark.xaml", UriKind.Absolute);
    private static readonly Uri SettingsLightUri = new(
        "pack://application:,,,/Resources/Palette.Light.xaml", UriKind.Absolute);
    private static readonly Uri OsdDarkUri = new(
        "pack://application:,,,/Resources/OsdPalette.Dark.xaml", UriKind.Absolute);
    private static readonly Uri OsdLightUri = new(
        "pack://application:,,,/Resources/OsdPalette.Light.xaml", UriKind.Absolute);

    private readonly Application _app;
    private readonly SettingsService _settings;

    private ResourceDictionary? _activeSettingsPalette;
    private ResourceDictionary? _activeOsdPalette;
    private bool _isEffectiveDark = true;
    private bool _started;
    private bool _disposed;

    /// <summary>Raised after a successful theme swap. Windows can subscribe to update
    /// per-window DWM attributes (immersive dark mode) that resources can't reach.</summary>
    public event Action? ThemeApplied;

    /// <summary>True when the currently rendered Settings palette is the dark one.
    /// Used by SettingsWindow to align the DWM dark-mode/Mica tint with the palette.</summary>
    public bool IsEffectiveDark => _isEffectiveDark;

    public ThemeService(Application app, SettingsService settings)
    {
        _app = app;
        _settings = settings;
    }

    public void Start()
    {
        if (_started) return;
        _started = true;
        Apply(_settings.Current.Theme);
        _settings.Changed += OnSettingsChanged;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
    }

    public void Apply(ThemeMode mode)
    {
        if (_disposed) return;

        bool wantsDark = mode switch
        {
            ThemeMode.Dark => true,
            ThemeMode.Light => false,
            ThemeMode.Auto => IsSystemDark(),
            _ => true,
        };

        // On the first Apply, adopt whichever palettes were loaded by App.xaml so we don't
        // pointlessly replace them with identical instances.
        if (_activeSettingsPalette is null)
        {
            _activeSettingsPalette = FindLoadedSettingsPalette();
            if (_activeSettingsPalette?.Source is { } sUri)
                _isEffectiveDark = IsSettingsPaletteDark(sUri);
        }
        _activeOsdPalette ??= FindLoadedOsdPalette();

        if (wantsDark == _isEffectiveDark && _activeSettingsPalette is not null && _activeOsdPalette is not null)
            return;

        var merged = _app.Resources.MergedDictionaries;
        _activeSettingsPalette = SwapPalette(merged, _activeSettingsPalette,
            wantsDark ? SettingsDarkUri : SettingsLightUri);
        _activeOsdPalette = SwapPalette(merged, _activeOsdPalette,
            wantsDark ? OsdDarkUri : OsdLightUri);

        _isEffectiveDark = wantsDark;
        ThemeApplied?.Invoke();
    }

    private static ResourceDictionary SwapPalette(
        System.Collections.ObjectModel.Collection<ResourceDictionary> merged,
        ResourceDictionary? current,
        Uri target)
    {
        var next = new ResourceDictionary { Source = target };
        if (current is not null)
        {
            int idx = merged.IndexOf(current);
            if (idx >= 0) merged[idx] = next;
            else merged.Add(next);
        }
        else
        {
            merged.Add(next);
        }
        return next;
    }

    // The leading slash disambiguates "Palette.Dark.xaml" (Settings) from
    // "OsdPalette.Dark.xaml" (OSD) — the OSD URI ends with the Settings suffix too.
    private ResourceDictionary? FindLoadedSettingsPalette() =>
        _app.Resources.MergedDictionaries.FirstOrDefault(d =>
            d.Source is not null &&
            (EndsWith(d.Source, "/Palette.Dark.xaml") || EndsWith(d.Source, "/Palette.Light.xaml")));

    private ResourceDictionary? FindLoadedOsdPalette() =>
        _app.Resources.MergedDictionaries.FirstOrDefault(d =>
            d.Source is not null &&
            (EndsWith(d.Source, "/OsdPalette.Dark.xaml") || EndsWith(d.Source, "/OsdPalette.Light.xaml")));

    // Only invoked for the Settings palette slot (see Apply). Kept narrow on purpose —
    // calling it for an OSD URI would silently treat the slot as a Settings one.
    private static bool IsSettingsPaletteDark(Uri uri) =>
        EndsWith(uri, "/Palette.Dark.xaml");

    private static bool EndsWith(Uri uri, string suffix) =>
        uri.OriginalString.EndsWith(suffix, StringComparison.OrdinalIgnoreCase);

    private void OnSettingsChanged(SettingsModel m)
    {
        // SettingsService raises Changed on the thread that called Save — typically the
        // UI dispatcher, but defend against future callers anyway.
        if (!_app.Dispatcher.CheckAccess())
        {
            _app.Dispatcher.BeginInvoke(new Action(() => Apply(m.Theme)));
            return;
        }
        Apply(m.Theme);
    }

    private void OnUserPreferenceChanged(object? sender, UserPreferenceChangedEventArgs e)
    {
        // The "General" category covers the apps-use-light-theme registry value among
        // others. Re-resolve only when the user has explicitly opted into Auto.
        if (e.Category != UserPreferenceCategory.General) return;
        if (_settings.Current.Theme != ThemeMode.Auto) return;

        if (!_app.Dispatcher.CheckAccess())
        {
            _app.Dispatcher.BeginInvoke(new Action(() => Apply(ThemeMode.Auto)));
            return;
        }
        Apply(ThemeMode.Auto);
    }

    private static bool IsSystemDark()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(
                @"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            var v = key?.GetValue("AppsUseLightTheme");
            // 0 = dark (apps don't use light), 1 = light. Default to dark if absent.
            if (v is int i) return i == 0;
            return true;
        }
        catch
        {
            return true;
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_started)
        {
            SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
            _settings.Changed -= OnSettingsChanged;
        }
    }
}
