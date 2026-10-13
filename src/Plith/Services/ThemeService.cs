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
internal enum PaletteKind
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
    // Spelled with the assembly (/Plith;component/...) rather than against the application's
    // resource assembly: identical in the running exe, and it also resolves where the entry
    // assembly is a test host, which is what lets the live swap be tested.
    private static readonly Uri SettingsDarkUri = new(
        "pack://application:,,,/Plith;component/Resources/Palette.Dark.xaml", UriKind.Absolute);
    private static readonly Uri SettingsLightUri = new(
        "pack://application:,,,/Plith;component/Resources/Palette.Light.xaml", UriKind.Absolute);
    private static readonly Uri OsdDarkUri = new(
        "pack://application:,,,/Plith;component/Resources/OsdPalette.Dark.xaml", UriKind.Absolute);
    private static readonly Uri OsdLightUri = new(
        "pack://application:,,,/Plith;component/Resources/OsdPalette.Light.xaml", UriKind.Absolute);
    private static readonly Uri SettingsHighContrastUri = new(
        "pack://application:,,,/Plith;component/Resources/Palette.HighContrast.xaml", UriKind.Absolute);
    private static readonly Uri OsdHighContrastUri = new(
        "pack://application:,,,/Plith;component/Resources/OsdPalette.HighContrast.xaml", UriKind.Absolute);

    // Brush keys the accent override replaces. Kept as constants so the surface is
    // greppable and easy to expand if new accent-sensitive brushes get added later.
    private const string KeyAccent = "Accent";
    private const string KeyAccentHover = "AccentHover";
    private const string KeyAccentPressed = "AccentPressed";
    private const string KeyAccentGlow = "AccentGlow";
    private const string KeyOsdAccent = "OsdAccent";
    // OSD surface keys: the fixed Loft floor, no longer tinted by the accent.
    private const string KeyOsdSurface = "OsdSurfaceBrush";
    private const string KeyOsdBorder = "OsdBorder";
    private const string KeyOsdTrackBg = "OsdTrackBg";
    private const string KeyOsdDivider = "OsdDivider";

    private const string KeyOsdAccentMark = "OsdAccentMark";
    private const string KeyOsdAccentText = "OsdAccentText";
    private const string KeyOnAccent = "OnAccent";
    // Settings path only (the Updates card's download bar). Still accent-derived: a general
    // neutral measured 1.6:1 against a deep accent there.
    private const string KeyAccentTrack = "AccentTrack";

    // Alpha channels match the values the base OsdPalette.*.xaml files already use. The floor
    // stops keep the OSD's long-standing slight translucency (the spec fixes the floor's colour,
    // not its opacity); the light theme is more opaque so dark glyphs still read over any app.
    private const byte OsdSurfaceAlphaDark = 0xF0;
    private const byte OsdSurfaceAlphaLight = 0xFA;
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
        if (IsHighContrast)
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
        return BuildAccentOverride(baseColor, IsEffectiveDark);
    }

    /// <summary>
    /// Everything the notch resolves that depends on the theme, as ONE dictionary for
    /// <see cref="Plith.Views.OsdHost"/> to mirror: the active OSD palette's keys, then the accent
    /// override on top (last wins, the same order Application.Resources uses).
    ///
    /// Why the palette travels too: the notch ink (NotchInk, NotchInkMuted, NotchTrack) and the
    /// Ornament* tokens live in OsdPalette.*.xaml and differ per theme, while OsdSurfaceBrush
    /// arrives through the override mirror. If only the override were mirrored, a Dark to Light
    /// switch could update the floor and leave the ink at its dark value (about 1.05:1 on the
    /// light floor). The keys are COPIED from the palette rather than listed here, so a token
    /// added to OsdPalette.*.xaml reaches the notch without anyone remembering to list it.
    /// </summary>
    public ResourceDictionary BuildOsdMirror() =>
        BuildOsdMirror(_activeOsdPalette, BuildAccentOverride());

    /// <summary>Composition half of <see cref="BuildOsdMirror()"/>, static so a test can check it
    /// against the real palette files.</summary>
    public static ResourceDictionary BuildOsdMirror(ResourceDictionary? palette, ResourceDictionary accentOverride)
    {
        var mirror = new ResourceDictionary();
        if (palette is not null)
        {
            foreach (var key in palette.Keys) mirror[key] = palette[key];
        }
        foreach (var key in accentOverride.Keys) mirror[key] = accentOverride[key];
        return mirror;
    }

    /// <summary>
    /// The same dictionary, from an explicit accent and theme rather than from settings.
    ///
    /// Static and public so the offscreen render harness can build exactly what the running app
    /// builds. That is not a convenience: the harness spent this branch painting widgets on a
    /// flat ground while the app painted them on an accent-tinted gradient, so every colour
    /// judgement made from its output was made against the wrong background — including the one
    /// that shipped a page nobody could read. A harness that recomputes the colours itself would
    /// drift from this method the first time either changed; calling it cannot.
    /// </summary>
    public static ResourceDictionary BuildAccentOverride(Color baseColor, bool isDark)
    {
        var derived = AccentTheme.Derive(baseColor, isDark);
        var roles = AccentTheme.DeriveLoftRoles(baseColor, isDark);
        var floor = LoftPalette.For(isDark);

        return new ResourceDictionary
        {
            [KeyAccent]        = FrozenBrush(derived.Accent),
            [KeyAccentHover]   = FrozenBrush(derived.Hover),
            [KeyAccentPressed] = FrozenBrush(derived.Pressed),
            [KeyAccentGlow]    = FrozenBrush(derived.Glow),

            // The OSD's accent marks (bar fill, rail pip) take the Mark, which DeriveLoftRoles
            // guarantees at 3:1 against both floor surfaces. roles.Fill makes no such claim and
            // measured 1.05 to 2.36:1 on the light floor for five of six presets.
            [KeyOsdAccent]     = FrozenBrush(roles.Mark),

            // Text on AccentBrush, the Settings primary button. It is measured against the
            // Settings accent (derived.Accent, darkened on the light theme), NOT against
            // roles.Fill, which can be a different colour: using roles.OnFill here measured
            // 3.1:1 to 3.4:1 on three accents in check-contrast.ps1.
            [KeyOnAccent]      = FrozenBrush(ContrastInk.On(derived.Accent)),

            // Lines, borders and markers take the mark; accent-coloured text takes the text role.
            // ADR 0014: a surface is never filled with the accent, so there is no third option.
            [KeyOsdAccentMark] = FrozenBrush(roles.Mark),
            [KeyOsdAccentText] = FrozenBrush(roles.Text),

            // The floor. Two stops so the panel keeps the vertical modelling the tinted gradient
            // had, and both stops are fixed, which is the point of the change.
            [KeyOsdSurface]    = BuildFloorBrush(floor, isDark),
            [KeyOsdBorder]     = FrozenBrush(floor.Border),
            [KeyOsdTrackBg]    = FrozenBrush(WithAlpha(floor.Border, OsdTrackAlpha)),
            [KeyOsdDivider]    = FrozenBrush(WithAlpha(floor.Border, OsdDividerAlpha)),

            // Settings window, out of scope for the notch work: keeps its accent-derived groove.
            [KeyAccentTrack]   = FrozenBrush(ContrastInk.TrackOn(derived.Accent)),
        };
    }

    private static LinearGradientBrush BuildFloorBrush(LoftFloor floor, bool isDark)
    {
        var brush = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = new Point(0, 1),
        };
        var alpha = isDark ? OsdSurfaceAlphaDark : OsdSurfaceAlphaLight;
        brush.GradientStops.Add(new GradientStop(WithAlpha(floor.Background, alpha), 0));
        brush.GradientStops.Add(new GradientStop(WithAlpha(floor.Surface, alpha), 1));
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
        // "Is this dark" and "what ink goes on it" are the same question, so they are answered in
        // the same place. This was a separate Rec.601 heuristic with its own 0.5 cut-off; it
        // agreed with ContrastInk most of the time, which is the worst property a duplicate rule
        // can have - it disagrees only near the crossover, where the answer matters most.
        //
        // Windows high-contrast themes can be either polarity ("High Contrast Black" vs "High
        // Contrast White"), which is why this cannot simply follow the palette kind.
        // Light ink was chosen for it, so the surface is dark.
        return ContrastInk.RelativeLuminance(ContrastInk.On(c)) > 0.5;
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
