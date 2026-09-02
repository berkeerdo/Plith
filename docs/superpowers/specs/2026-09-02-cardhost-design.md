# Phase 5 — CardHost, Suppression Gate, Accessibility

**Date:** 2026-09-02
**Status:** Approved design, ready for writing-plans
**Roadmap reference:** `docs/ROADMAP.md` §6 "Phase 5 — Consolidation + tech debt"

## Problem statement

Phases 1–4f shipped an audio-first OSD whose rendering path is hardcoded for exactly
two features. `OsdContent.xaml` is a `StackPanel` holding a media row, a divider, and a
volume row, all with fixed visibility bindings. `OsdViewModel` owns volume state, the
media sub-view-model, and the `ShowMediaCard` visibility decision at the same time.
`OsdOrchestrator` writes directly into `_osd.ViewModel` and calls `_osd.ShowOsd(...)`,
and five other call sites do the same.

Phase 6 adds a System Controls card (brightness, keyboard backlight, mic mute, lock keys,
airplane mode) and a Battery card. Neither fits this structure without either bloating
`OsdViewModel` further or bolting a second visibility mechanism onto the orchestrator.

Phase 5 puts a `CardHost` in place so today's OSD becomes a single-purpose set of cards
inside a generic host, with **no user-visible change**. Two adjacent items ride along
because they touch the same surfaces: an accessibility pass (the OSD and Settings window
currently carry zero `AutomationProperties`) and auto-hide during exclusive fullscreen
video, which is the mirror image of the show authority `CardHost` introduces.

## Goals

- `CardHost` owns which cards are visible and when the OSD pops, and is unit-testable
  without a WPF `Application` or an HWND.
- Today's OSD renders as an Audio card + a Media card inside that host, pixel-identical
  to 0.1.5.
- Adding a Phase 6 card means adding one card class, one view-model, one view, and one
  `DataTemplate` — with no edit to `CardHost`, `OsdHost`, or `OsdContent`.
- The OSD hides during fullscreen video (Netflix, VLC, YouTube fullscreen) and **never**
  hides during games.
- Screen readers announce OSD volume changes and can name every interactive Settings
  control; high-contrast mode renders a legible OSD.

## Non-goals

- **Notch geometry, preset modes, per-card Settings UI** — all Phase 6. Phase 5 ships no
  new user-facing surface beyond the fullscreen-video toggle.
- **`IEventSource` layer** (ROADMAP §5 event source table) — deferred to Phase 6, where
  the System Controls card becomes its first real consumer. Building it now would be an
  abstraction with no second implementation to validate it against.
- **Card priority / one-card-at-a-time cycling** — ROADMAP §9 flags this as an open
  question that Ambient and Full notch modes may answer differently. Phase 5's policy is
  the 0.1.5 policy: every visible card renders, stacked, in `Order`.
- **Plugin API / card catalog UI** — Phase 9.
- **Vendor OSD suppression** (Logitech, Corsair, Razer) — Phase 6+.

---

## §1 — CardHost and the card split

### New types (`src/Plith/Cards/`)

```csharp
namespace Plith.Cards;

public enum ShowReason
{
    AudioChange,
    MediaChange,
    MediaCommand,
    SummonHotkey,
    VolumeKey,
    EditModeExit,
}

public sealed record ShowRequest(
    ShowReason Reason,
    string? OriginCardId = null,
    TimeSpan? DurationOverride = null);

/// <summary>
/// A self-contained OSD feature. Owns its view-model, its visibility opinion, and its
/// own trigger conditions. Knows nothing about the window it renders in.
/// </summary>
public interface ICard
{
    string Id { get; }
    int Order { get; }
    bool IsVisible { get; }
    object ViewModel { get; }

    event Action? VisibilityChanged;
    event Action<ShowRequest>? ShowRequested;

    void Activate();
    void Deactivate();

    /// <summary>
    /// The active palette or accent changed; re-resolve any cached brushes. Default-empty
    /// so cards that hold no brush cache ignore it.
    /// </summary>
    void OnThemeChanged() { }
}

/// <summary>Consulted by CardHost before honouring any show request.</summary>
public interface IShowSuppressor
{
    bool IsSuppressed { get; }
    event Action<bool>? SuppressionChanged;
}
```

```csharp
public sealed class CardHost : IDisposable
{
    public CardHost(SettingsService settings, IShowSuppressor? suppressor = null);

    public void Register(ICard card);
    public IReadOnlyList<ICard> Cards { get; }                  // registration order-sorted by Order
    public ObservableCollection<ICard> VisibleCards { get; }     // policy output, Order-sorted

    public event Action<TimeSpan>? ShowRequested;
    public event Action? HideRequested;

    public void RequestShow(ShowRequest request);
    public void NotifyThemeChanged();   // fan out ICard.OnThemeChanged()
    public void Start();     // Activate() every registered card
    public void Dispose();   // Deactivate() every registered card
}
```

**`CardHost` holds no reference to any WPF window.** Its only framework dependency is
`ObservableCollection<T>` (`System.Collections.ObjectModel`). This is the constraint that
makes the show policy testable headlessly, and it is the reason `ShowRequested` is an
event rather than a direct `OsdHost.ShowOsd(...)` call.

### Show policy (v1)

`RequestShow(request)` performs, in order:

1. If `_suppressor?.IsSuppressed == true`, return without raising anything.
2. Recompute `VisibleCards` from each registered card's `IsVisible`, preserving `Order`.
3. Compute duration: `request.DurationOverride ?? TimeSpan.FromMilliseconds(settings.Current.ShowDurationMs)`.
4. Raise `ShowRequested(duration)`.

`VisibleCards` is also recomputed (without raising `ShowRequested`) whenever any card
raises `VisibilityChanged`, so a card going away mid-display collapses the OSD in place —
matching how `ShowMediaCard` behaves in 0.1.5.

When `_suppressor.SuppressionChanged` fires with `true`, `CardHost` raises `HideRequested`
so an OSD already on screen disappears rather than waiting out its timer.

This policy is deliberately thin: it reproduces 0.1.5 behaviour exactly. Priority,
cycling, and always-on card sets are Phase 6 concerns and belong here when there is a
mode that needs them.

### View resolution — `ICard` never names a view

`src/Plith/Resources/CardTemplates.xaml`, merged into `App.xaml` resources:

```xml
<DataTemplate DataType="{x:Type vm:AudioCardViewModel}">
    <views:AudioCardView />
</DataTemplate>
<DataTemplate DataType="{x:Type vm:MediaViewModel}">
    <views:MediaCardView />
</DataTemplate>
```

Implicit `DataType` templates mean `ItemsControl` resolves each card's view from its
view-model type. `ICard` exposes `object ViewModel` and stays free of any view reference,
which keeps the card classes constructible in tests.

### `OsdContent.xaml`

The outer `Grid` margin, the `Border` (corner radius 14, padding 20,16, `OsdSurfaceBrush`,
`OsdBorder`, drop shadow) and the fixed `Width="440"` all stay exactly as they are — they
are shell chrome, not card content. The inner `StackPanel` is replaced by:

```xml
<ItemsControl ItemsSource="{Binding VisibleCards}" AlternationCount="64">
    <ItemsControl.ItemsPanel>
        <ItemsPanelTemplate><StackPanel Orientation="Vertical" /></ItemsPanelTemplate>
    </ItemsControl.ItemsPanel>
    <ItemsControl.ItemTemplate>
        <DataTemplate>
            <!-- The alternation index is read into Tag by an ELEMENT-level binding and the
                 trigger then reads Tag. Binding RelativeSource=AncestorType=ContentPresenter
                 directly inside DataTemplate.Triggers does NOT work: a template trigger's
                 binding evaluates against the container and FindAncestor starts at the
                 container's PARENT, so the search walks past the item's own ContentPresenter
                 and lands on an outer one where AlternationIndex is unset and defaults to 0 —
                 collapsing the divider on every card. Do not "simplify" this back. -->
            <StackPanel x:Name="ItemRoot"
                        Tag="{Binding RelativeSource={RelativeSource AncestorType=ContentPresenter},
                                      Path=(ItemsControl.AlternationIndex)}">
                <!-- Separator above every card except the first. -->
                <Border x:Name="Divider" Height="1" Margin="0,14,0,14"
                        Background="{DynamicResource OsdDivider}" />
                <ContentControl Content="{Binding ViewModel}" />
            </StackPanel>
            <DataTemplate.Triggers>
                <DataTrigger Value="0" Binding="{Binding ElementName=ItemRoot, Path=Tag}">
                    <Setter TargetName="Divider" Property="Visibility" Value="Collapsed" />
                </DataTrigger>
            </DataTemplate.Triggers>
        </DataTemplate>
    </ItemsControl.ItemTemplate>
</ItemsControl>
```

`AlternationCount="64"` makes `ItemsControl.AlternationIndex` report the true index for
the first 64 items, which is the standard WPF idiom for index-aware item templates. The
notch will never hold 64 cards; the constant gets a comment saying so.

The `Tag` indirection above is not stylistic. An earlier revision of this spec put the
`AncestorType=ContentPresenter` binding straight inside the `DataTrigger`, and that form
was measured — in an isolated WPF layout harness reproducing this exact tree — to collapse
the divider on *every* item, costing `14 + 1 + 14 = 29 px` of separation between the media
and volume rows and shortening the whole card. Element-level bindings resolve the ancestor
correctly; template-trigger bindings do not.

**Geometry note:** in 0.1.5 the divider carries `Margin="0,14,0,14"` and sits between the
media row and the volume row. Placing the divider *above* each non-first card reproduces
that spacing exactly for the two-card case, which is the case that must stay
pixel-identical.

`OsdHost.Reposition()` measures `_content.DesiredSize` and is unaffected — it already
tracks content size changes, which is why the media card appearing and disappearing
already repositions correctly today.

### Responsibility migration

| Today | Phase 5 |
|---|---|
| `OsdViewModel`: `Label`, `GainNormalized`, `GainText`, `Muted`, `GainColor`, `UseColorThresholds`, `RefreshThresholdBrushes()` | `AudioCardViewModel` — logic unchanged |
| `OsdViewModel.Media` + `ShowMediaCard` | `MediaCard.IsVisible => Media.HasSession && !settings.Current.CompactMode` |
| `OsdViewModel.CompactMode` | `MediaCard` reads `SettingsService` directly. `CompactMode` means "hide the media card" in 0.1.5 and nowhere else, so it is the media card's business. |
| `OsdOrchestrator.HandleValueChange` baseline suppression (`_lastNormalized`, `_lastMuted`, first-read guard) | `AudioCard.Apply(...)` + `AudioCard.ResetBaseline()` |
| `OsdOrchestrator` → `_osd.ShowOsd(...)` ×2 | `_cardHost.RequestShow(...)` |
| `App.xaml.cs` `_volumeKeyHook.VolumeKeyPressed` → `_osd.ShowOsd(...)` | `_cardHost.RequestShow(new(ShowReason.VolumeKey))` |
| `App.xaml.cs` `_hotkey.Pressed` → `_osd.ShowOsd(...)` | `_cardHost.RequestShow(new(ShowReason.SummonHotkey))` |
| `OsdHost.ExitPositionEditMode` → `ShowOsd(...)` | `_cardHost.RequestShow(new(ShowReason.EditModeExit, DurationOverride: max(ShowDurationMs, 1500)))` |
| `OsdHost.MediaCommandInvoked` bridging `OsdContent` → orchestrator | `MediaCard.CommandInvoked`; orchestrator subscribes to the card |
| `App.xaml.cs` `_theme.ThemeApplied += () => _osd?.ViewModel.RefreshThresholdBrushes()` | `_theme.ThemeApplied += () => _cardHost.NotifyThemeChanged()`; `AudioCard.OnThemeChanged()` forwards to `AudioCardViewModel.RefreshThresholdBrushes()` |
| `OsdViewModel.VoicemeeterMinDb` / `VoicemeeterMaxDb` consts, referenced from `OsdOrchestrator.HandleVoicemeeterChange` | move to `AudioCardViewModel` — a compile-breaking reference the migration must follow |
| `OsdHost.ViewModel` public property | `OsdHost.Shell` (`OsdShellViewModel`); `ViewModel` is removed rather than retyped, so every stale consumer surfaces as a compile error |

`OsdOrchestrator` keeps the Voicemeeter poll loop, the reconnect timer, source
reconciliation, the Windows-audio watchdog, and the `WindowsVolumeEvent` hook that feeds
`NativeFlyoutSuppressor`. It loses every display decision, dropping from ~338 lines to
roughly 250.

`OsdHost` keeps `ShowOsd(TimeSpan)` as a public method — position edit mode and the fade
machinery need it — but `App` wires `cardHost.ShowRequested += osd.ShowOsd` and
`cardHost.HideRequested += osd.HideOsd`, and nothing outside `OsdHost` calls `ShowOsd`
directly any more. `HideOsd()` is new: it cancels the hide timer and runs the existing
fade-out path immediately.

### Card implementations

**`AudioCard`** (`Order = 20`, `Id = "audio"`, `IsVisible => true`)

Absorbs the baseline-suppression logic currently in `OsdOrchestrator.HandleValueChange`:

```csharp
public void Apply(string label, double normalized, string text, bool muted)
{
    bool isFirstRead = _lastNormalized is null;
    bool changed = isFirstRead
        || Math.Abs(_lastNormalized!.Value - normalized) > 0.0005
        || _lastMuted != muted;

    _lastNormalized = (float)normalized;
    _lastMuted = muted;

    ViewModel.Apply(label, normalized, text, muted);
    if (changed && !isFirstRead)
        ShowRequested?.Invoke(new ShowRequest(ShowReason.AudioChange, Id));
}

public void ResetBaseline() { _lastNormalized = null; _lastMuted = null; }
```

`OsdOrchestrator` calls `ResetBaseline()` where it currently nulls those two fields:
source transitions in `ReconcileActiveSource`, successful Voicemeeter login, and
`OnSettingsChanged`.

**`MediaCard`** (`Order = 10`, `Id = "media"`)

```csharp
public bool IsVisible => _vm.HasSession && !_settings.Current.CompactMode;

public void Apply(MediaSnapshot snapshot)
{
    _vm.Apply(snapshot);
    if (_settings.Current.AutoShowOnMedia && snapshot.HasSession)
        ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaChange, Id));
}
```

`VisibilityChanged` is raised from `MediaViewModel.HasSessionChanged` and from
`SettingsService.Changed` when `CompactMode` flips.

`MediaCard` also re-exposes `event EventHandler<MediaCommand>? CommandInvoked`, forwarded
from `MediaCardView`. The orchestrator's existing `OnMediaCommandInvoked` handler moves to
subscribe here and issues `RequestShow(new(ShowReason.MediaCommand, "media"))` after
dispatching the transport command.

`Order` values are spaced by 10 so Phase 6 cards can slot between existing ones without
renumbering.

### `OsdShellViewModel`

`OsdContent`'s `DataContext`. Exposes `ObservableCollection<ICard> VisibleCards`
(delegated straight from `CardHost`). Nothing else in Phase 5 — shell-level state like
notch height and preset mode arrives in Phase 6.

### Files

**New**
- `src/Plith/Cards/ICard.cs`
- `src/Plith/Cards/ShowRequest.cs` (`ShowReason` + `ShowRequest`)
- `src/Plith/Cards/IShowSuppressor.cs`
- `src/Plith/Cards/CardHost.cs`
- `src/Plith/Cards/AudioCard.cs`
- `src/Plith/Cards/MediaCard.cs`
- `src/Plith/ViewModels/AudioCardViewModel.cs`
- `src/Plith/ViewModels/OsdShellViewModel.cs`
- `src/Plith/Views/AudioCardView.xaml` (+ `.cs`) — the volume row lifted out of `OsdContent`
- `src/Plith/Resources/CardTemplates.xaml`

**Renamed**
- `src/Plith/Views/MediaCard.xaml` → `src/Plith/Views/MediaCardView.xaml` (+ `.cs`).
  Required: `Plith.Cards.MediaCard` and `Plith.Views.MediaCard` would otherwise differ only
  by namespace, which is legal but a readability trap in XAML where both namespaces are
  imported.

**Modified**
- `src/Plith/Views/OsdContent.xaml` (+ `.cs`) — `ItemsControl`
- `src/Plith/Views/OsdHost.cs` — `DataContext` becomes `OsdShellViewModel`; add `HideOsd()`;
  drop the `MediaCommandInvoked` bridge; edit-mode exit routes through `CardHost`
- `src/Plith/Services/OsdOrchestrator.cs` — pure source driver
- `src/Plith/App.xaml.cs` — construct and wire `CardHost`; merge `CardTemplates.xaml`
- `src/Plith/App.xaml` — merge `CardTemplates.xaml`
- `src/Plith/Views/SettingsPreview.xaml` (+ `.cs`) — binds `OsdViewModel` and
  `ShowMediaCard` today; migrates to `AudioCardViewModel` + `MediaViewModel`.
  **The preview stays a hand-built mock, not a `CardHost` instance** — it exists to
  animate live theme and toggle changes inside the Settings window and must not acquire a
  show pipeline. Its `ShowMediaCard` binding is replaced by a local `bool` on the preview's
  own code-behind, driven by the same settings it already watches.

**Deleted**
- `src/Plith/ViewModels/OsdViewModel.cs` (split into the two above)

### Tests (`tests/Plith.Tests/`, all headless)

`CardHostTests`
- Registered cards are exposed sorted by `Order`.
- A card whose `IsVisible` is false is absent from `VisibleCards`.
- `VisibilityChanged` on a registered card updates `VisibleCards` without raising `ShowRequested`.
- A card's `ShowRequested` is re-emitted with the duration from `SettingsService.Current.ShowDurationMs`.
- `DurationOverride` wins over the settings value.
- With a suppressor reporting `IsSuppressed == true`, `RequestShow` raises nothing.
- `SuppressionChanged(true)` raises `HideRequested`.
- `Dispose()` calls `Deactivate()` on every registered card.
- `NotifyThemeChanged()` reaches every registered card, including invisible ones.

`AudioCardTests`
- First `Apply` after construction raises no `ShowRequested` (baseline read).
- A subsequent `Apply` with a different value raises `ShowRequested`.
- An `Apply` with the same value (within the 0.0005 epsilon) raises nothing.
- A mute flip at an unchanged gain raises `ShowRequested`.
- After `ResetBaseline()`, the next `Apply` is silent again.

`MediaCardTests`
- `AutoShowOnMedia == false` → `Apply` raises no `ShowRequested`.
- `HasSession == false` → `IsVisible` is false.
- `CompactMode == true` → `IsVisible` is false even with an active session.
- `HasSession` flipping raises `VisibilityChanged`.

`AudioCardViewModelTests`
- The existing `OsdViewModelTests` and `OsdViewModelColorTests` bodies move here
  unchanged; only the type name differs. `OsdViewModelTests.cs` and
  `OsdViewModelColorTests.cs` are deleted.

---

## §2 — Suppression gate: auto-hide during fullscreen video

### Detection

The OSD must keep drawing over games — that is what the entire BandWindow / UIAccess
effort in Phase 4h bought. So detection has to separate fullscreen *video* from
fullscreen *games*, and must fail toward "do not hide".

```
IsSuppressed =
       settings.Current.HideDuringFullscreenVideo
    && ForegroundWindowCoversMonitor()
    && SHQueryUserNotificationState() != QUNS_RUNNING_D3D_FULL_SCREEN
    && ( ForegroundOwnsPlayingSmtcSession() || HideList.Contains(foregroundProcessName) )
```

- `ForegroundWindowCoversMonitor()` — `GetForegroundWindow` + `GetWindowRect` compared
  against `Screen.FromHandle(...).Bounds` (full bounds, not working area) with a small
  tolerance. Excludes the shell (`Progman`, `WorkerW`) by class name.
- `SHQueryUserNotificationState() != QUNS_RUNNING_D3D_FULL_SCREEN` — a hard veto for
  D3D exclusive fullscreen. This is the single most important term: an exclusive-fullscreen
  game can never be suppressed regardless of what else matches.
- `ForegroundOwnsPlayingSmtcSession()` — the positive signal, and one Plith already has.
  Netflix and YouTube in a browser, VLC, and Spotify all publish SMTC sessions.
  Borderless-windowed games do not, which is what keeps them visible.
- `HideList` — escape hatch for players that render fullscreen video without publishing
  SMTC (mpv by default, some PotPlayer configurations).

Every other state, including "unknown", evaluates to false. A user who runs neither a
listed player nor a media-publishing app sees byte-identical 0.1.5 behaviour.

### AUMID → process matching is a heuristic

`ForegroundOwnsPlayingSmtcSession()` needs the SMTC session's owner. `MediaSnapshot` gains
a `SourceAppUserModelId` field, populated from
`GlobalSystemMediaTransportControlsSession.SourceAppUserModelId`.

For Win32 apps that AUMID is in practice the executable name (`chrome.exe`, `vlc.exe`,
`Spotify.exe`); for packaged apps it is a package family name, which will not match a
process name. Matching is therefore a case-insensitive comparison between the foreground
process name and the AUMID, accepting a substring hit in either direction.

**This is explicitly a heuristic and will miss cases.** It is acceptable because every
miss fails toward "do not hide" — the safe direction — and `HideList` is the user's
override. If Phase 6 needs something stronger, `GetProcessId`-based enumeration of SMTC
owners is the escalation path.

### `FullscreenVideoWatcher`

`src/Plith/Services/FullscreenVideoWatcher.cs`, implementing `IShowSuppressor, IDisposable`.

Reuses the `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT)` pattern
already proven in `ForegroundWatcher`, plus a 1-second `DispatcherTimer` poll — entering
fullscreen with F11 changes no foreground window, so the hook alone would miss it. Each
evaluation is one `GetForegroundWindow`, one `GetWindowRect`, one
`SHQueryUserNotificationState`, and a string compare; polling at 1 Hz is negligible.

Raises `SuppressionChanged(bool)` only on an actual state transition.

The decision itself lives in a pure static method so it can be tested without any Win32
call:

```csharp
internal static bool ShouldSuppress(
    bool enabled,
    bool foregroundCoversMonitor,
    uint notificationState,
    bool foregroundOwnsPlayingSmtc,
    string foregroundProcessName,
    IReadOnlyCollection<string> hideList);
```

### Settings

Two new fields on `SettingsModel`, persisted through `SettingsService` in the existing
INI format:

- `HideDuringFullscreenVideo` (bool, **default `true`**)
- `FullscreenVideoHideList` (comma-separated string, default `"mpv,PotPlayerMini64"`)

Settings UI: a toggle plus a text box under Behavior.

**Known tension:** Phase 5's success metric is "OSD behaviour is identical to 0.1.5 from
the user's side", and this feature is by definition a behaviour change. Defaulting to
`true` is the deliberate choice — the feature exists to be on, and the conservative
detection means it only fires when a fullscreen window is actively playing media. It
goes in the release notes as a behaviour change with a named opt-out.

### Wiring

`App` constructs `FullscreenVideoWatcher` and passes it to `CardHost`'s constructor.
`CardHost` consults `IsSuppressed` in `RequestShow` and subscribes to
`SuppressionChanged` to raise `HideRequested`.

### Tests

`FullscreenVideoDetectorTests` drives `ShouldSuppress` across the matrix:
- disabled → false regardless of every other input
- not fullscreen → false
- fullscreen + `QUNS_RUNNING_D3D_FULL_SCREEN` + playing SMTC → **false** (the game case)
- fullscreen + `QUNS_BUSY` + playing SMTC → true (the Netflix / VLC case)
- fullscreen + `QUNS_BUSY` + no SMTC + process in hide list → true (the mpv case)
- fullscreen + `QUNS_BUSY` + no SMTC + process not in list → false (the borderless-game case)
- hide-list matching is case-insensitive and tolerates a `.exe` suffix

---

## §3 — Accessibility pass

`AutomationProperties` appears in zero of the five XAML files today.

### 1. MediaCard transport buttons — a real defect

The previous / play-pause / next buttons contain only Segoe Fluent Icons glyphs from the
Unicode private use area. A screen reader currently announces the raw glyph codepoint.

Fix: `AutomationProperties.Name` of `"Previous track"`, `"Next track"`, and a
`"Play"` / `"Pause"` value bound to `MediaViewModel.IsPlaying` on the toggle — mirroring
the existing `PlayPauseGlyph` property, which gains a sibling `PlayPauseLabel`.

### 2. OSD live announcements

The OSD never takes focus (`Activatable = false`, `Focusable = false`), so it cannot be
navigated to. The correct pattern for transient status surfaces is a named live region:

- `AutomationProperties.Name` on each card root, reflecting current content
  (`"Bus A1, 0.0 dB"` for the audio card; `"<title> by <artist>, playing"` for media).
- `AutomationProperties.LiveSetting="Polite"` on the card root so a value change is
  announced without stealing the reader's place.

**Verification caveat:** whether a `CreateWindowInBand` HWND participates in the UIA tree
the way a normal top-level window does is not something to assume. This gets confirmed
manually with Narrator during implementation. If the band window turns out to be
invisible to UIA, the fallback documented here is to leave the properties in place (they
cost nothing and are correct) and record the limitation in the plan rather than chase a
workaround inside Phase 5.

### 3. SettingsWindow

680 lines of XAML with no automation metadata. Every interactive control gets
`AutomationProperties.Name`; controls already labelled by an adjacent `TextBlock` get
`AutomationProperties.LabeledBy` instead, so the name is not duplicated. Controls whose
label alone is ambiguous get `AutomationProperties.HelpText`.

Highest-value targets, both textless today:
- the hotkey capture button, whose content is a dynamic combo string or a prompt
- the accent colour swatches, which carry no text at all and need per-swatch names

Also in scope: tab order across the window, and confirming focus visuals are visible on
every control style — the Phase 4f button restyling did not check keyboard focus.

### 4. High contrast

`Palette.Dark.xaml` and `Palette.Light.xaml` hardcode every colour, and `ThemeService`
chooses between them from the theme setting alone. Under Windows high contrast the OSD
therefore paints its own surface and accent, ignoring the user's system colours; the 1 px
`OsdBorder` and the card divider are at risk of vanishing entirely.

Add `Palette.HighContrast.xaml` mapping the OSD and Settings brush keys onto
`SystemColors.*` keys. `ThemeService` selects it when `SystemParameters.HighContrast` is
true, overriding the theme setting, and re-evaluates on
`SystemParameters.StaticPropertyChanged` for `HighContrast`.

Audit deliverable: a short list in the plan of which brush keys currently break, checked
by toggling high contrast on a running build.

### 5. Regression guard

`AutomationProperties` live in XAML, where unit tests are weak — asserting on them needs
an STA thread and a loaded visual tree, which is flaky for the value returned.

Instead: `scripts/check-a11y.ps1` parses each XAML file and fails (exit 1) on any
`Button`, `ComboBox`, `Slider`, `ToggleButton`, `CheckBox`, or `TextBox` element carrying
neither `AutomationProperties.Name` nor `AutomationProperties.LabeledBy`. Deterministic,
fast, and CI-attachable. Elements that legitimately need no name carry an explicit
`AutomationProperties.Name=""` with a comment, which the script treats as intentional.

---

## Verification

Phase 5 closes under `superpowers:verification-before-completion` — claims backed by
command output, not assertions:

- `dotnet build` clean, zero warnings (the project runs `EnableNETAnalyzers` at
  `Recommended`).
- `dotnet test` green across both test projects.
- **Screenshot comparison of the OSD between a 0.1.5 build and the Phase 5 build**, in
  both the audio-only and audio+media states. This is the only real evidence for the
  "zero user-visible change" success metric; a passing test suite is not.
- Manual Narrator pass over the OSD and the Settings window.
- Manual high-contrast pass over the OSD and the Settings window.
- Manual check that the OSD still draws over an exclusive-fullscreen game, and that it
  does hide during fullscreen Netflix or VLC playback.

`superpowers:requesting-code-review` runs before the branch merges.

## Open questions deferred to Phase 6

Recorded here so they are not rediscovered:

- Whether the notch shows all enabled cards side-by-side or cycles one at a time by
  priority (ROADMAP §9). `CardHost`'s policy method is the single place this changes.
- Which monitor the notch pins to (ROADMAP §9). `OsdHost.ResolveTargetScreen` already
  handles the per-event OSD case.
- Whether `CompactMode` should become a shell-level concept rather than a media-card
  property once more cards exist.
