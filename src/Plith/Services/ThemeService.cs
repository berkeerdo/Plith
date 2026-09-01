using System.Linq;
using System.Windows;
using System.Windows.Media;
using Microsoft.Win32;

namespace Plith.Services;

/// <summary>
/// Owns both the palette polarity (dark / light) and the accent overlay for the
/// Settings window and the OSD. Swaps the active palette ResourceDictionary in
/// <see cref="Application.Resources"/> between the dark and light variants in
/// response to <see cref="SettingsModel.Theme"/> or the Windows apps-use-light-
/// theme preference (for <see cref="ThemeMode.Auto"/>), and stacks an accent
/// override dictionary at the end so the Theme Studio's picked colour beats the
/// palette's baked-in accent without touching the raw XAML files. Every Settings
/// brush is bound via <c>DynamicResource</c>, so both swaps propagate live
/// without re-creating any windows.
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

    // Brush keys the accent override replaces. Kept as constants so the surface is
    // greppable and easy to expand if new accent-sensitive brushes get added later.
    private const string KeyAccent = "Accent";
    private const string KeyAccentHover = "AccentHover";
    private const string KeyAccentPressed = "AccentPressed";
    private const string KeyAccentGlow = "AccentGlow";
    private const string KeyOsdAccent = "OsdAccent";
    // OSD surface keys — overriding these tints the whole card, not just the bar.
    private const string KeyOsdSurface = "OsdSurfaceBrush";
    private const string KeyOsdBorder = "OsdBorder";
    private const string KeyOsdTrackBg = "OsdTrackBg";
    private const string KeyOsdDivider = "OsdDivider";

    // Alpha channels match the values the base OsdPalette.*.xaml files already use, so
    // swapping in a tinted brush keeps the same drop-shadow-over-game translucency the
    // OSD had before the Theme Studio landed.
    private const byte OsdSurfaceAlpha = 0xF0;
    private const byte OsdTrackAlpha = 0x40;
    private const byte OsdDividerAlpha = 0x40;

    private readonly Application _app;
    private readonly SettingsService _settings;

    private ResourceDictionary? _activeSettingsPalette;
    private ResourceDictionary? _activeOsdPalette;
    private ResourceDictionary? _accentOverride;
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

        // Palette swap. When polarity is already correct, both slots are populated,
        // and there is no accent override yet-to-be-applied, we can short-circuit. We
        // still fall through to ApplyAccentOverride when the accent picker changes,
        // because polarity is unchanged there but the accent brushes need refreshing.
        if (wantsDark != _isEffectiveDark || _activeSettingsPalette is null || _activeOsdPalette is null)
        {
            var merged = _app.Resources.MergedDictionaries;
            _activeSettingsPalette = SwapPalette(merged, _activeSettingsPalette,
                wantsDark ? SettingsDarkUri : SettingsLightUri);
            _activeOsdPalette = SwapPalette(merged, _activeOsdPalette,
                wantsDark ? OsdDarkUri : OsdLightUri);
            _isEffectiveDark = wantsDark;
        }

        // Rebuild + re-append the accent override so it sits after any palette dictionary
        // (last-added wins in MergedDictionaries lookup) even if the polarity swap moved
        // things around. Always safe to run: cheap, idempotent, and the only source of truth
        // for the currently applied accent.
        ApplyAccentOverride();

        ThemeApplied?.Invoke();
    }

    private void ApplyAccentOverride()
    {
        var dict = BuildAccentOverride();
        var merged = _app.Resources.MergedDictionaries;
        if (_accentOverride is not null) merged.Remove(_accentOverride);
        merged.Add(dict);
        _accentOverride = dict;
    }

    /// <summary>
    /// Returns a fresh <see cref="ResourceDictionary"/> containing the current derived
    /// accent + OSD-surface brushes. Called by <see cref="ApplyAccentOverride"/> for the
    /// Application-level slot; also called by <see cref="Plith.Views.OsdHost"/> to mirror
    /// the same brushes into its own <c>Resources.MergedDictionaries</c>, because a
    /// BandWindow (HwndSource with a custom RootVisual) does NOT reliably receive
    /// notifications when <see cref="Application.Resources"/> is mutated —
    /// DynamicResource references inside the OSD only see local-tree changes. Each
    /// caller adds a distinct copy so ResourceDictionary parent-ownership stays clean.
    /// </summary>
    public ResourceDictionary BuildAccentOverride()
    {
        var s = _settings.Current;
        var baseColor = AccentTheme.ResolveBase(s.AccentThemeId, s.CustomAccentColor);
        var derived = AccentTheme.Derive(baseColor, _isEffectiveDark);
        var osd = AccentTheme.DeriveOsdSurfaces(baseColor, _isEffectiveDark);

        return new ResourceDictionary
        {
            [KeyAccent]        = FrozenBrush(derived.Accent),
            [KeyAccentHover]   = FrozenBrush(derived.Hover),
            [KeyAccentPressed] = FrozenBrush(derived.Pressed),
            [KeyAccentGlow]    = FrozenBrush(derived.Glow),
            // OSD accent uses the raw derived accent (bright) so the volume bar
            // pops off the tinted card behind it.
            [KeyOsdAccent]     = FrozenBrush(derived.Accent),
            // Whole-card tint. The surface is a two-stop gradient so it keeps the
            // vertical modeling the original palette had, only hue-shifted.
            [KeyOsdSurface]    = BuildOsdSurfaceBrush(osd),
            [KeyOsdBorder]     = FrozenBrush(osd.Border),
            [KeyOsdTrackBg]    = FrozenBrush(WithAlpha(osd.TrackBg, OsdTrackAlpha)),
            [KeyOsdDivider]    = FrozenBrush(WithAlpha(osd.Divider, OsdDividerAlpha)),
        };
    }

    private static LinearGradientBrush BuildOsdSurfaceBrush(OsdSurfaceDerived s)
    {
        // Vertical top-to-bottom gradient, matching the original OsdSurfaceBrush's
        // StartPoint / EndPoint. Alpha F0 keeps the same subtle translucency over games.
        var brush = new LinearGradientBrush(
            WithAlpha(s.SurfaceStart, OsdSurfaceAlpha),
            WithAlpha(s.SurfaceEnd, OsdSurfaceAlpha),
            new Point(0, 0), new Point(0, 1));
        brush.Freeze();
        return brush;
    }

    private static Color WithAlpha(Color c, byte a) => Color.FromArgb(a, c.R, c.G, c.B);

    private static SolidColorBrush FrozenBrush(Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        return b;
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
