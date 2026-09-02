using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Media;
using Microsoft.Win32;

namespace Plith.Services;

/// <summary>
/// Which palette family is currently applied. Distinct from the user's
/// <see cref="ThemeMode"/> setting: <see cref="HighContrast"/> can be reached from any
/// <see cref="ThemeMode"/> whenever Windows high contrast is active, and overrides it.
/// </summary>
public enum PaletteKind
{
    Dark,
    Light,
    HighContrast,
}

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
    private static readonly Uri SettingsHighContrastUri = new(
        "pack://application:,,,/Resources/Palette.HighContrast.xaml", UriKind.Absolute);
    private static readonly Uri OsdHighContrastUri = new(
        "pack://application:,,,/Resources/OsdPalette.HighContrast.xaml", UriKind.Absolute);

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
    private PaletteKind _paletteKind = PaletteKind.Dark;
    private bool _started;
    private bool _disposed;

    /// <summary>Raised after a successful theme swap. Windows can subscribe to update
    /// per-window DWM attributes (immersive dark mode) that resources can't reach.</summary>
    public event Action? ThemeApplied;

    /// <summary>True when Windows high contrast is currently active. High contrast
    /// overrides the user's <see cref="ThemeMode"/> setting entirely, including
    /// <see cref="ThemeMode.Auto"/>.</summary>
    // CA1822: kept as an instance member — callers (OsdHost, SettingsWindow) already hold
    // a ThemeService instance and query it for every other theme fact; a static member
    // would fragment that API for no benefit.
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Performance", "CA1822:Mark members as static",
        Justification = "Instance member kept for API consistency with the rest of ThemeService's instance-scoped surface.")]
    public bool IsHighContrast => SystemParameters.HighContrast;

    /// <summary>True when the currently rendered Settings palette reads as dark.
    /// Used by SettingsWindow to align the DWM dark-mode/Mica tint with the palette.
    /// For <see cref="PaletteKind.HighContrast"/> this is derived from the luminance of
    /// the system window colour, since a high-contrast theme can be either light or dark.</summary>
    public bool IsEffectiveDark => _paletteKind switch
    {
        PaletteKind.Dark => true,
        PaletteKind.Light => false,
        PaletteKind.HighContrast => IsColorDark(SystemColors.WindowColor),
        _ => true,
    };

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
        SystemParameters.StaticPropertyChanged += OnSystemParametersChanged;
    }

    public void Apply(ThemeMode mode)
    {
        if (_disposed) return;

        // High contrast overrides the user's theme setting entirely, including Auto.
        PaletteKind wantsKind;
        if (SystemParameters.HighContrast)
        {
            wantsKind = PaletteKind.HighContrast;
        }
        else
        {
            bool wantsDark = mode switch
            {
                ThemeMode.Dark => true,
                ThemeMode.Light => false,
                ThemeMode.Auto => IsSystemDark(),
                _ => true,
            };
            wantsKind = wantsDark ? PaletteKind.Dark : PaletteKind.Light;
        }

        // On the first Apply, adopt whichever palettes were loaded by App.xaml so we don't
        // pointlessly replace them with identical instances.
        if (_activeSettingsPalette is null)
        {
            _activeSettingsPalette = FindLoadedSettingsPalette();
            if (_activeSettingsPalette?.Source is { } sUri)
                _paletteKind = ClassifySettingsPaletteUri(sUri);
        }
        _activeOsdPalette ??= FindLoadedOsdPalette();

        // Palette swap. When the palette kind is already correct, both slots are
        // populated, and there is no accent override yet-to-be-applied, we can
        // short-circuit. We still fall through to ApplyAccentOverride when the accent
        // picker changes, because the palette kind is unchanged there but the accent
        // brushes need refreshing.
        if (wantsKind != _paletteKind || _activeSettingsPalette is null || _activeOsdPalette is null)
        {
            var merged = _app.Resources.MergedDictionaries;
            _activeSettingsPalette = SwapPalette(merged, _activeSettingsPalette, SettingsUriFor(wantsKind));
            _activeOsdPalette = SwapPalette(merged, _activeOsdPalette, OsdUriFor(wantsKind));
            _paletteKind = wantsKind;
        }

        // Rebuild + re-append the accent override so it sits after any palette dictionary
        // (last-added wins in MergedDictionaries lookup) even if the palette swap moved
        // things around. Always safe to run: cheap, idempotent, and the only source of truth
        // for the currently applied accent. Under high contrast this yields an empty
        // dictionary (see BuildAccentOverride), so the palette's own SystemColors-mapped
        // brushes win instead of an accent tint layered over them.
        ApplyAccentOverride();

        ThemeApplied?.Invoke();
    }

    private static Uri SettingsUriFor(PaletteKind kind) => kind switch
    {
        PaletteKind.Dark => SettingsDarkUri,
        PaletteKind.Light => SettingsLightUri,
        PaletteKind.HighContrast => SettingsHighContrastUri,
        _ => SettingsDarkUri,
    };

    private static Uri OsdUriFor(PaletteKind kind) => kind switch
    {
        PaletteKind.Dark => OsdDarkUri,
        PaletteKind.Light => OsdLightUri,
        PaletteKind.HighContrast => OsdHighContrastUri,
        _ => OsdDarkUri,
    };

    private void ApplyAccentOverride()
    {
        var dict = BuildAccentOverride();
        var merged = _app.Resources.MergedDictionaries;
        // Removing the previously applied override before adding the new one means a
        // high-contrast transition (where the new dict is empty) leaves nothing behind:
        // the app-level accent slot goes from "tinted" to "contributes nothing", and
        // DynamicResource lookups fall through to the high-contrast palette's own
        // SystemColors-mapped brushes instead.
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
    ///
    /// Returns an empty dictionary while <see cref="IsHighContrast"/> is true: an accent
    /// tint layered over the user's system colours would defeat the point of high
    /// contrast, and an empty dictionary needs no special-casing at either call site —
    /// OsdHost.RefreshAccentMirror just mirrors nothing.
    /// </summary>
    public ResourceDictionary BuildAccentOverride()
    {
        if (IsHighContrast) return new ResourceDictionary();

        var s = _settings.Current;
        var baseColor = AccentTheme.ResolveBase(s.AccentThemeId, s.CustomAccentColor);
        var derived = AccentTheme.Derive(baseColor, IsEffectiveDark);
        var osd = AccentTheme.DeriveOsdSurfaces(baseColor, IsEffectiveDark);

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
    // calling it for an OSD URI would silently treat the slot as a Settings one. App.xaml
    // only ever loads Dark or Light at startup (never HighContrast), but the HighContrast
    // branch is included so this stays correct if that ever changes.
    private static PaletteKind ClassifySettingsPaletteUri(Uri uri)
    {
        if (EndsWith(uri, "/Palette.Dark.xaml")) return PaletteKind.Dark;
        if (EndsWith(uri, "/Palette.HighContrast.xaml")) return PaletteKind.HighContrast;
        return PaletteKind.Light;
    }

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

    private void OnSystemParametersChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(SystemParameters.HighContrast)) return;

        if (!_app.Dispatcher.CheckAccess())
        {
            _app.Dispatcher.BeginInvoke(new Action(() => Apply(_settings.Current.Theme)));
            return;
        }
        Apply(_settings.Current.Theme);
    }

    private static bool IsColorDark(Color c)
    {
        // Perceived-luminance heuristic (Rec. 601 weights). Windows high-contrast themes
        // can be either polarity (e.g. "High Contrast Black" vs "High Contrast White"), so
        // this can't be assumed the way Dark/Light palette kind can.
        double luminance = ((0.299 * c.R) + (0.587 * c.G) + (0.114 * c.B)) / 255.0;
        return luminance < 0.5;
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
            SystemParameters.StaticPropertyChanged -= OnSystemParametersChanged;
            SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
            _settings.Changed -= OnSettingsChanged;
        }
    }
}
