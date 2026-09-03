# Phase 5 — CardHost, Suppression Gate, Accessibility — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Plith's hardcoded two-feature OSD into a generic `CardHost` that renders N cards, add auto-hide during fullscreen video, and give the app real screen-reader and high-contrast support — with the OSD looking and behaving exactly as it does in 0.1.5.

**Architecture:** A `CardHost` service owns which cards are visible and is the single authority for when the OSD pops. It holds no reference to any WPF window — it raises `ShowRequested(TimeSpan)` and `HideRequested`, and `OsdHost` subscribes. Today's OSD becomes an Audio card plus a Media card registered with that host; `OsdOrchestrator` degrades to a pure audio/media source driver. A `FullscreenVideoWatcher` plugs into the host as an `IShowSuppressor`.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`), xunit 2.9.3, NAudio, WpfScreenHelper, ini-parser-netstandard. No new NuGet packages.

**Spec:** `docs/superpowers/specs/2026-09-02-cardhost-design.md`

## Global Constraints

- **English only** in all code, comments, commit messages, and docs. No AI attribution of any kind anywhere (per `CLAUDE.md`).
- **Conventional Commits** format: `type(scope): subject`, subject ≤ 50 chars.
- **Build baseline is not zero warnings.** `dotnet build Plith.slnx -c Debug` currently emits **exactly 3 CA1861 warnings**, all in `tests/Plith.Installer.Tests/InstallLockedFileExceptionTests.cs` (lines 23, 26, 34). These are pre-existing and out of scope. The bar for every task is **no new warnings** — 3 warnings at the end of a task is success; 4 is a regression. Do not "fix" the existing three.
- `EnableNETAnalyzers` is on at `AnalysisMode=Recommended` for `src/Plith`. `NoWarn` already covers `CA1707;CA1838`.
- **InvariantCulture** for every persisted value and every number formatted for display. `config.ini` written on a tr-TR machine must read identically on en-US.
- Target framework `net10.0-windows10.0.22000.0`, platform `x64` only.
- Build command: `dotnet build Plith.slnx -c Debug`
- Test command: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
- **Every task must leave the solution building and all tests passing.** No task may land a half-migrated type.
- **No synthetic input, ever.** An automated worker must never inject mouse or keyboard
  events, activate or focus a window, or launch and drive the app interactively. This is
  not a style preference: during Task 3 a synthetic click intended for the OSD's Play/Pause
  button landed in a live Valorant match — the OSD had auto-hidden, and a click sent at
  screen coordinates goes wherever those coordinates now point. Automated work stops at
  code, tests, `dotnet build`, and reading logs.
- **Manual GUI verification is a human step.** Where a task calls for running the app,
  comparing screenshots, driving Narrator, or checking high contrast, the worker stops and
  reports `NEEDS_CONTEXT` with exact instructions: what to launch, what to do, what to look
  for, and what would count as a failure. A human runs it and reports back. A task is not
  complete until its manual steps have actually been performed by a person — "covered by
  unit tests instead" does not discharge a manual step.
- Card `Order` values are spaced by 10 (`media = 10`, `audio = 20`) so Phase 6 cards slot in without renumbering.

---

# Stage A — CardHost refactor (Tasks 1–6)

Ends with the OSD rendered by `CardHost` and pixel-identical to 0.1.5.

---

### Task 1: Card contracts and CardHost

Pure new code with no consumers. Nothing else in the app changes, so this task is safe to land on its own.

**Files:**
- Create: `src/Plith/Cards/ICard.cs`
- Create: `src/Plith/Cards/ShowRequest.cs`
- Create: `src/Plith/Cards/IShowSuppressor.cs`
- Create: `src/Plith/Cards/CardHost.cs`
- Test: `tests/Plith.Tests/CardHostTests.cs`

**Interfaces:**
- Consumes: `Plith.Services.SettingsService` (existing — `Current.ShowDurationMs`), `Plith.Services.SettingsModel`
- Produces: `Plith.Cards.ICard`, `Plith.Cards.ShowReason`, `Plith.Cards.ShowRequest`, `Plith.Cards.IShowSuppressor`, `Plith.Cards.CardHost`

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/CardHostTests.cs`:

```csharp
using System.Collections.Generic;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

/// <summary>Minimal ICard stand-in so CardHost can be tested without any real card.</summary>
internal sealed class FakeCard : ICard
{
    public FakeCard(string id, int order, bool visible = true)
    {
        Id = id;
        Order = order;
        _isVisible = visible;
    }

    public string Id { get; }
    public int Order { get; }
    public object ViewModel { get; } = new object();

    private bool _isVisible;
    public bool IsVisible
    {
        get => _isVisible;
        set { _isVisible = value; VisibilityChanged?.Invoke(); }
    }

    public int ActivateCount { get; private set; }
    public int DeactivateCount { get; private set; }
    public int ThemeChangedCount { get; private set; }

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    public void Activate() => ActivateCount++;
    public void Deactivate() => DeactivateCount++;
    public void OnThemeChanged() => ThemeChangedCount++;

    public void RaiseShow(ShowRequest request) => ShowRequested?.Invoke(request);
}

internal sealed class FakeSuppressor : IShowSuppressor
{
    private bool _suppressed;
    public bool IsSuppressed
    {
        get => _suppressed;
        set { _suppressed = value; SuppressionChanged?.Invoke(value); }
    }

    public event Action<bool>? SuppressionChanged;
}

public class CardHostTests
{
    private static SettingsService NewSettings(int showDurationMs = 2000)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        var m = svc.Current.Clone();
        m.ShowDurationMs = showDurationMs;
        svc.Save(m);
        return svc;
    }

    [Fact]
    public void Cards_AreExposedSortedByOrder()
    {
        var host = new CardHost(NewSettings());
        host.Register(new FakeCard("audio", 20));
        host.Register(new FakeCard("media", 10));

        Assert.Equal(new[] { "media", "audio" }, host.Cards.Select(c => c.Id));
    }

    [Fact]
    public void VisibleCards_ExcludesInvisibleCard()
    {
        var host = new CardHost(NewSettings());
        host.Register(new FakeCard("media", 10, visible: false));
        host.Register(new FakeCard("audio", 20));

        Assert.Equal(new[] { "audio" }, host.VisibleCards.Select(c => c.Id));
    }

    [Fact]
    public void VisibilityChanged_UpdatesVisibleCardsWithoutRaisingShow()
    {
        var host = new CardHost(NewSettings());
        var media = new FakeCard("media", 10, visible: false);
        host.Register(media);
        host.Register(new FakeCard("audio", 20));

        int shows = 0;
        host.ShowRequested += _ => shows++;

        media.IsVisible = true;

        Assert.Equal(new[] { "media", "audio" }, host.VisibleCards.Select(c => c.Id));
        Assert.Equal(0, shows);
    }

    [Fact]
    public void CardShowRequest_IsReEmittedWithSettingsDuration()
    {
        var host = new CardHost(NewSettings(showDurationMs: 3500));
        var audio = new FakeCard("audio", 20);
        host.Register(audio);

        TimeSpan? seen = null;
        host.ShowRequested += d => seen = d;

        audio.RaiseShow(new ShowRequest(ShowReason.AudioChange, "audio"));

        Assert.Equal(TimeSpan.FromMilliseconds(3500), seen);
    }

    [Fact]
    public void DurationOverride_WinsOverSettingsValue()
    {
        var host = new CardHost(NewSettings(showDurationMs: 2000));

        TimeSpan? seen = null;
        host.ShowRequested += d => seen = d;

        host.RequestShow(new ShowRequest(ShowReason.EditModeExit, null, TimeSpan.FromMilliseconds(1500)));

        Assert.Equal(TimeSpan.FromMilliseconds(1500), seen);
    }

    [Fact]
    public void SuppressedHost_SwallowsShowRequest()
    {
        var suppressor = new FakeSuppressor { IsSuppressed = true };
        var host = new CardHost(NewSettings(), suppressor);

        int shows = 0;
        host.ShowRequested += _ => shows++;

        host.RequestShow(new ShowRequest(ShowReason.SummonHotkey));

        Assert.Equal(0, shows);
    }

    [Fact]
    public void SuppressionTurningOn_RaisesHideRequested()
    {
        var suppressor = new FakeSuppressor();
        var host = new CardHost(NewSettings(), suppressor);

        int hides = 0;
        host.HideRequested += () => hides++;

        suppressor.IsSuppressed = true;

        Assert.Equal(1, hides);
    }

    [Fact]
    public void SuppressionTurningOff_DoesNotRaiseHideRequested()
    {
        var suppressor = new FakeSuppressor { IsSuppressed = true };
        var host = new CardHost(NewSettings(), suppressor);

        int hides = 0;
        host.HideRequested += () => hides++;

        suppressor.IsSuppressed = false;

        Assert.Equal(0, hides);
    }

    [Fact]
    public void Start_ActivatesEveryCard_DisposeDeactivatesEveryCard()
    {
        var host = new CardHost(NewSettings());
        var a = new FakeCard("audio", 20);
        var b = new FakeCard("media", 10);
        host.Register(a);
        host.Register(b);

        host.Start();
        Assert.Equal(1, a.ActivateCount);
        Assert.Equal(1, b.ActivateCount);

        host.Dispose();
        Assert.Equal(1, a.DeactivateCount);
        Assert.Equal(1, b.DeactivateCount);
    }

    [Fact]
    public void NotifyThemeChanged_ReachesInvisibleCardsToo()
    {
        var host = new CardHost(NewSettings());
        var hidden = new FakeCard("media", 10, visible: false);
        host.Register(hidden);

        host.NotifyThemeChanged();

        Assert.Equal(1, hidden.ThemeChangedCount);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~CardHostTests`
Expected: compile failure — `The type or namespace name 'Cards' does not exist in the namespace 'Plith'`.

- [ ] **Step 3: Write the contracts**

Create `src/Plith/Cards/ShowRequest.cs`:

```csharp
namespace Plith.Cards;

/// <summary>Why the OSD is being asked to appear. Phase 5 treats every reason identically;
/// Phase 6 notch modes use it to decide which cards a given trigger should surface.</summary>
public enum ShowReason
{
    AudioChange,
    MediaChange,
    MediaCommand,
    SummonHotkey,
    VolumeKey,
    EditModeExit,
}

/// <param name="Reason">What triggered the request.</param>
/// <param name="OriginCardId">The <see cref="ICard.Id"/> that raised it, or null for shell-level triggers.</param>
/// <param name="DurationOverride">Visible-for override; null means use SettingsModel.ShowDurationMs.</param>
public sealed record ShowRequest(
    ShowReason Reason,
    string? OriginCardId = null,
    TimeSpan? DurationOverride = null);
```

Create `src/Plith/Cards/ICard.cs`:

```csharp
namespace Plith.Cards;

/// <summary>
/// A self-contained OSD feature. Owns its view-model, its own visibility opinion, and its
/// own trigger conditions. Deliberately knows nothing about the window it renders in — the
/// view is resolved from <see cref="ViewModel"/>'s type via an implicit DataTemplate, which
/// is what keeps cards constructible in headless tests.
/// </summary>
public interface ICard
{
    /// <summary>Stable identifier, e.g. "audio" / "media". Used in ShowRequest.OriginCardId.</summary>
    string Id { get; }

    /// <summary>Render order inside the OSD stack; lower renders higher up. Spaced by 10
    /// so new cards can slot between existing ones without renumbering.</summary>
    int Order { get; }

    /// <summary>The card's own opinion on whether it has anything to show right now.</summary>
    bool IsVisible { get; }

    /// <summary>DataContext for the card's view.</summary>
    object ViewModel { get; }

    /// <summary>Raised when <see cref="IsVisible"/> flips.</summary>
    event Action? VisibilityChanged;

    /// <summary>Raised when the card wants the OSD on screen.</summary>
    event Action<ShowRequest>? ShowRequested;

    /// <summary>Subscribe to whatever sources this card reads.</summary>
    void Activate();

    /// <summary>Unsubscribe. Must be safe to call without a prior Activate.</summary>
    void Deactivate();

    /// <summary>
    /// The active palette or accent changed; re-resolve any cached brushes. Default-empty
    /// so cards holding no brush cache ignore it.
    /// </summary>
    void OnThemeChanged() { }
}
```

Create `src/Plith/Cards/IShowSuppressor.cs`:

```csharp
namespace Plith.Cards;

/// <summary>
/// Consulted by <see cref="CardHost"/> before honouring any show request. Implemented in
/// Phase 5 by FullscreenVideoWatcher; the nullable constructor parameter means CardHost
/// works without one.
/// </summary>
public interface IShowSuppressor
{
    bool IsSuppressed { get; }

    /// <summary>Raised only on an actual transition, with the new value.</summary>
    event Action<bool>? SuppressionChanged;
}
```

- [ ] **Step 4: Write CardHost**

Create `src/Plith/Cards/CardHost.cs`:

```csharp
using System.Collections.ObjectModel;
using Plith.Services;

namespace Plith.Cards;

/// <summary>
/// Owns the registered card set, decides which cards are visible, and is the single
/// authority for when the OSD appears.
///
/// Deliberately holds no reference to any WPF window: it raises <see cref="ShowRequested"/>
/// and <see cref="HideRequested"/> and lets OsdHost subscribe. That constraint is what makes
/// the show policy unit-testable with no Application and no HWND, and it is the reason this
/// type must never grow a Window, Dispatcher, or Visual dependency.
/// </summary>
public sealed class CardHost : IDisposable
{
    private readonly SettingsService _settings;
    private readonly IShowSuppressor? _suppressor;
    private readonly List<ICard> _cards = new();
    private bool _disposed;

    public CardHost(SettingsService settings, IShowSuppressor? suppressor = null)
    {
        _settings = settings;
        _suppressor = suppressor;
        if (_suppressor is not null)
            _suppressor.SuppressionChanged += OnSuppressionChanged;
    }

    /// <summary>Every registered card, sorted by <see cref="ICard.Order"/>.</summary>
    public IReadOnlyList<ICard> Cards => _cards;

    /// <summary>Policy output: the cards that should render right now, in Order.
    /// Bound directly by OsdShellViewModel.</summary>
    public ObservableCollection<ICard> VisibleCards { get; } = new();

    /// <summary>The OSD should appear for this long.</summary>
    public event Action<TimeSpan>? ShowRequested;

    /// <summary>The OSD should disappear now, regardless of its hide timer.</summary>
    public event Action? HideRequested;

    public void Register(ICard card)
    {
        // Keep _cards sorted on insert so Cards and VisibleCards share one ordering rule.
        int index = _cards.FindIndex(c => c.Order > card.Order);
        if (index < 0) _cards.Add(card); else _cards.Insert(index, card);

        card.VisibilityChanged += OnCardVisibilityChanged;
        card.ShowRequested += OnCardShowRequested;
        RecomputeVisibleCards();
    }

    public void Start()
    {
        foreach (var card in _cards) card.Activate();
        RecomputeVisibleCards();
    }

    /// <summary>Fan a theme/accent swap out to every card, visible or not — an invisible
    /// card must already hold correct brushes by the time it becomes visible.</summary>
    public void NotifyThemeChanged()
    {
        foreach (var card in _cards) card.OnThemeChanged();
    }

    public void RequestShow(ShowRequest request)
    {
        if (_disposed) return;
        if (_suppressor?.IsSuppressed == true) return;

        RecomputeVisibleCards();

        var duration = request.DurationOverride
            ?? TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs);
        ShowRequested?.Invoke(duration);
    }

    private void OnCardShowRequested(ShowRequest request) => RequestShow(request);

    // A card going away mid-display must collapse the OSD in place without re-popping it —
    // this is how ShowMediaCard behaved in 0.1.5 and the behaviour must not change.
    private void OnCardVisibilityChanged() => RecomputeVisibleCards();

    private void OnSuppressionChanged(bool suppressed)
    {
        if (_disposed) return;
        // Only the rising edge matters: suppression turning on must pull an on-screen OSD
        // down immediately rather than let it ride out its hide timer.
        if (suppressed) HideRequested?.Invoke();
    }

    private void RecomputeVisibleCards()
    {
        // _cards is already Order-sorted, so a positional in-place reconcile preserves order
        // without clearing the collection — clearing would make the ItemsControl rebuild every
        // card container and restart any animation the views own.
        int target = 0;
        foreach (var card in _cards)
        {
            if (!card.IsVisible) continue;

            if (target < VisibleCards.Count && ReferenceEquals(VisibleCards[target], card))
            {
                target++;
                continue;
            }

            int existing = VisibleCards.IndexOf(card);
            if (existing >= 0) VisibleCards.Move(existing, target);
            else VisibleCards.Insert(target, card);
            target++;
        }

        while (VisibleCards.Count > target) VisibleCards.RemoveAt(VisibleCards.Count - 1);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (_suppressor is not null)
            _suppressor.SuppressionChanged -= OnSuppressionChanged;

        foreach (var card in _cards)
        {
            card.VisibilityChanged -= OnCardVisibilityChanged;
            card.ShowRequested -= OnCardShowRequested;
            card.Deactivate();
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~CardHostTests`
Expected: PASS, 10 tests.

- [ ] **Step 6: Verify no new warnings**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.` with `3 Warning(s)` — the pre-existing CA1861 trio and nothing else.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Cards tests/Plith.Tests/CardHostTests.cs
git commit -m "feat(cards): add ICard contract and CardHost"
```

---

### Task 2: Extract AudioCardViewModel from OsdViewModel

Splits the volume half of `OsdViewModel` into its own type. `OsdViewModel` survives this task as a thin composite (`Audio` + `Media` + `ShowMediaCard`) so the render path stays untouched and the app keeps working. It is deleted in Task 6.

**Files:**
- Create: `src/Plith/ViewModels/AudioCardViewModel.cs`
- Modify: `src/Plith/ViewModels/OsdViewModel.cs`
- Modify: `src/Plith/Views/OsdContent.xaml` (volume-row bindings gain an `Audio.` prefix)
- Modify: `src/Plith/Views/SettingsPreview.xaml` (same) and `src/Plith/Views/SettingsPreview.xaml.cs`
- Modify: `src/Plith/Services/OsdOrchestrator.cs` (constant references + `Apply` target)
- Modify: `src/Plith/App.xaml.cs:45`
- Create: `tests/Plith.Tests/AudioCardViewModelTests.cs`
- Delete: `tests/Plith.Tests/OsdViewModelTests.cs`, `tests/Plith.Tests/OsdViewModelColorTests.cs`

**Interfaces:**
- Consumes: `Plith.Services.VoicemeeterParameterSnapshot` (existing)
- Produces: `Plith.ViewModels.AudioCardViewModel` with `const float VoicemeeterMinDb = -60f`, `const float VoicemeeterMaxDb = 12f`, properties `Label`, `GainNormalized`, `GainText`, `Muted`, `UseColorThresholds`, `Brush GainColor`, methods `void Apply(VoicemeeterParameterSnapshot)`, `void Apply(string label, double normalized, string text, bool muted)`, `void RefreshThresholdBrushes()`. `OsdViewModel` retains `AudioCardViewModel Audio { get; }`, `MediaViewModel Media { get; }`, `bool CompactMode`, `bool ShowMediaCard`.

- [ ] **Step 1: Create AudioCardViewModel by moving the volume half**

Create `src/Plith/ViewModels/AudioCardViewModel.cs` containing, moved verbatim from `OsdViewModel.cs`: the `VoicemeeterMinDb` / `VoicemeeterMaxDb` constants, `UseColorThresholds`, `Label`, `GainNormalized`, `GainText`, `Muted`, all five `_brush*` fields, `FreezeBrush`, `RefreshThresholdBrushes`, `ResolveBrush`, `GainColor`, both `Apply` overloads, and the `INotifyPropertyChanged` plumbing (`PropertyChanged`, `OnPropertyChanged`, `Set<T>`). Keep every comment — the brush-seed comment explaining why unit tests see the dark-theme defaults is load-bearing.

The constructor becomes:

```csharp
public AudioCardViewModel() => RefreshThresholdBrushes();
```

Class declaration and doc comment:

```csharp
/// <summary>
/// Source-agnostic view model for the Audio card. The orchestrator computes the normalized
/// bar fill and the display text from whichever source produced the change — Voicemeeter is
/// dB, Windows endpoint is percent — and hands the formatted result to <see cref="Apply"/>.
/// </summary>
public sealed class AudioCardViewModel : INotifyPropertyChanged
```

- [ ] **Step 2: Reduce OsdViewModel to a composite**

Replace the body of `src/Plith/ViewModels/OsdViewModel.cs` with:

```csharp
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.ViewModels;

/// <summary>
/// Transitional composite binding root for the OSD: the Audio card's view model, the Media
/// card's view model, and the media-visibility rule that used to live inline. Replaced by
/// OsdShellViewModel + CardHost in Task 6 of the Phase 5 plan.
/// </summary>
public sealed class OsdViewModel : INotifyPropertyChanged
{
    public AudioCardViewModel Audio { get; } = new();
    public MediaViewModel Media { get; }

    public OsdViewModel()
    {
        Media = new MediaViewModel();
        Media.HasSessionChanged += () => OnPropertyChanged(nameof(ShowMediaCard));
    }

    private bool _compactMode;
    public bool CompactMode
    {
        get => _compactMode;
        set
        {
            if (_compactMode == value) return;
            _compactMode = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(ShowMediaCard));
        }
    }

    /// <summary>The media card shows only when there's an active session AND the user hasn't
    /// asked for compact mode.</summary>
    public bool ShowMediaCard => Media.HasSession && !_compactMode;

    public event PropertyChangedEventHandler? PropertyChanged;
    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
```

- [ ] **Step 3: Repoint every consumer**

In `src/Plith/Views/OsdContent.xaml`, the volume row's four bindings gain an `Audio.` prefix — `{Binding GainText}` → `{Binding Audio.GainText}`, `{Binding Label}` → `{Binding Audio.Label}`, `{Binding GainColor}` → `{Binding Audio.GainColor}`, and inside the `MultiBinding`, `<Binding Path="GainNormalized" />` → `<Binding Path="Audio.GainNormalized" />`. `ShowMediaCard` bindings are unchanged.

In `src/Plith/Views/SettingsPreview.xaml`, apply the same `Audio.` prefix to the volume-row bindings. `ShowMediaCard` bindings are unchanged.

In `src/Plith/Views/SettingsPreview.xaml.cs`, the seed block becomes:

```csharp
PreviewViewModel = new OsdViewModel();
PreviewViewModel.Audio.Label = "Bus A1";
PreviewViewModel.Audio.GainText = "+3.0 dB";
PreviewViewModel.Audio.GainNormalized = 0.85;
PreviewViewModel.Media.Title = "Sample track";
PreviewViewModel.Media.Artist = "Sample artist";
PreviewViewModel.Media.HasSession = true;
```

and `UpdateColorThresholds` becomes `PreviewViewModel.Audio.UseColorThresholds = thresholds;`.

In `src/Plith/Services/OsdOrchestrator.cs`: `OsdViewModel.VoicemeeterMinDb` / `VoicemeeterMaxDb` (lines ~263–265) become `AudioCardViewModel.*`, and `_osd.ViewModel.Apply(...)` (line ~288) becomes `_osd.ViewModel.Audio.Apply(...)`.

In `src/Plith/Views/OsdHost.cs`, the settings-changed handler line `ViewModel.UseColorThresholds = m.UseColorThresholds;` becomes `ViewModel.Audio.UseColorThresholds = m.UseColorThresholds;` (two occurrences: inside the `_settings.Changed` lambda and in the constructor's seed block).

In `src/Plith/App.xaml.cs:45`: `_osd?.ViewModel.RefreshThresholdBrushes()` becomes `_osd?.ViewModel.Audio.RefreshThresholdBrushes()`.

- [ ] **Step 4: Migrate the tests**

Create `tests/Plith.Tests/AudioCardViewModelTests.cs` holding the combined contents of `OsdViewModelTests.cs` and `OsdViewModelColorTests.cs`, with the class renamed to `AudioCardViewModelTests` and every `new OsdViewModel()` replaced by `new AudioCardViewModel()`. **Do not change a single assertion** — identical assertions passing against the new type is the evidence that the extraction preserved behaviour.

Delete `tests/Plith.Tests/OsdViewModelTests.cs` and `tests/Plith.Tests/OsdViewModelColorTests.cs`.

- [ ] **Step 5: Run the tests**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS. The AudioCardViewModel test count must equal the combined count of the two deleted files — if it dropped, a test was lost in the move.

- [ ] **Step 6: Verify the build**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 7: Commit**

```bash
git add -A src/Plith tests/Plith.Tests
git commit -m "refactor(vm): extract AudioCardViewModel from OsdViewModel"
```

---

### Task 3: Rename MediaCard view and route commands through its view model

`OsdContent`'s code-behind reaches the media view by its XAML name (`MediaCardControl`). Once the OSD renders through an `ItemsControl` there is no named element, so transport commands must travel via the view model instead. This task does that rewiring while the StackPanel is still in place, and renames the view so `Plith.Views.MediaCard` stops colliding with the `Plith.Cards.MediaCard` added in Task 5.

**Files:**
- Rename: `src/Plith/Views/MediaCard.xaml` → `src/Plith/Views/MediaCardView.xaml`
- Rename: `src/Plith/Views/MediaCard.xaml.cs` → `src/Plith/Views/MediaCardView.xaml.cs`
- Modify: `src/Plith/ViewModels/MediaViewModel.cs`
- Modify: `src/Plith/Views/OsdContent.xaml`, `src/Plith/Views/OsdContent.xaml.cs`
- Test: `tests/Plith.Tests/MediaViewModelTests.cs` (new)

**Interfaces:**
- Consumes: `Plith.Views.MediaCommand` (existing enum, stays in `MediaCardView.xaml.cs`)
- Produces: `MediaViewModel.RequestCommand(MediaCommand)`, `event Action<MediaCommand>? CommandRequested`

- [ ] **Step 1: Write the failing test**

Create `tests/Plith.Tests/MediaViewModelTests.cs`:

```csharp
using Plith.ViewModels;
using Plith.Views;

namespace Plith.Tests;

public class MediaViewModelTests
{
    [Fact]
    public void RequestCommand_RaisesCommandRequestedWithTheSameCommand()
    {
        var vm = new MediaViewModel();
        MediaCommand? seen = null;
        vm.CommandRequested += c => seen = c;

        vm.RequestCommand(MediaCommand.SkipNext);

        Assert.Equal(MediaCommand.SkipNext, seen);
    }

    [Fact]
    public void RequestCommand_WithNoSubscriber_DoesNotThrow()
    {
        var vm = new MediaViewModel();
        vm.RequestCommand(MediaCommand.TogglePlayPause);
    }

    [Fact]
    public void PlayPauseLabel_TracksIsPlaying()
    {
        var vm = new MediaViewModel();

        vm.IsPlaying = false;
        Assert.Equal("Play", vm.PlayPauseLabel);

        vm.IsPlaying = true;
        Assert.Equal("Pause", vm.PlayPauseLabel);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~MediaViewModelTests`
Expected: compile failure — `'MediaViewModel' does not contain a definition for 'RequestCommand'`.

- [ ] **Step 3: Add the command channel and the accessible label to MediaViewModel**

In `src/Plith/ViewModels/MediaViewModel.cs`, extend the `IsPlaying` setter to also notify `PlayPauseLabel`:

```csharp
private bool _isPlaying;
public bool IsPlaying
{
    get => _isPlaying;
    set
    {
        if (Set(ref _isPlaying, value))
        {
            OnPropertyChanged(nameof(PlayPauseGlyph));
            OnPropertyChanged(nameof(PlayPauseLabel));
        }
    }
}
```

and add, next to the existing `PlayPauseGlyph`:

```csharp
/// <summary>Screen-reader label for the play/pause toggle. The glyph beside it is a Segoe
/// Fluent Icons private-use codepoint, which a screen reader would otherwise read aloud
/// verbatim — this is the only text a non-sighted user gets for that button.</summary>
public string PlayPauseLabel => _isPlaying ? "Pause" : "Play";

/// <summary>Raised when the user clicks a transport button. The view calls
/// <see cref="RequestCommand"/> on its DataContext rather than surfacing an event on the
/// UserControl, because under CardHost the view is created by a DataTemplate and no owner
/// holds a named reference to it.</summary>
public event Action<Plith.Views.MediaCommand>? CommandRequested;

public void RequestCommand(Plith.Views.MediaCommand command) => CommandRequested?.Invoke(command);
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~MediaViewModelTests`
Expected: PASS, 3 tests.

- [ ] **Step 5: Rename the view and repoint its clicks**

```bash
git mv src/Plith/Views/MediaCard.xaml src/Plith/Views/MediaCardView.xaml
git mv src/Plith/Views/MediaCard.xaml.cs src/Plith/Views/MediaCardView.xaml.cs
```

In `MediaCardView.xaml`, change `x:Class="Plith.Views.MediaCard"` to `x:Class="Plith.Views.MediaCardView"`.

Also **remove** `Visibility="{Binding HasSession, Converter={StaticResource BoolToVisibilityConverter}}"` from the root `Grid`. From Task 6 onward `CardHost` owns visibility; leaving this binding in place would create a second source of truth that silently disagrees with `MediaCard.IsVisible`. Between now and Task 6, `OsdContent`'s wrapper `Border` (bound to `ShowMediaCard`) already covers the same case, so this removal changes nothing visible.

Replace the body of `MediaCardView.xaml.cs`:

```csharp
using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class MediaCardView : UserControl
{
    public MediaCardView()
    {
        InitializeComponent();
        PrevButton.Click += (_, _) => Request(MediaCommand.SkipPrevious);
        PlayPauseButton.Click += (_, _) => Request(MediaCommand.TogglePlayPause);
        NextButton.Click += (_, _) => Request(MediaCommand.SkipNext);
    }

    // The DataContext is supplied by whoever hosts this view — an explicit assignment today,
    // an implicit DataTemplate from Task 6 on. Either way the view-model is the only channel
    // out, so a null DataContext (designer, or a not-yet-bound container) is a silent no-op.
    private void Request(MediaCommand command) => (DataContext as MediaViewModel)?.RequestCommand(command);
}

public enum MediaCommand { SkipPrevious, TogglePlayPause, SkipNext }
```

- [ ] **Step 6: Repoint OsdContent**

In `src/Plith/Views/OsdContent.xaml`, change `<views:MediaCard x:Name="MediaCardControl" ... />` to `<views:MediaCardView x:Name="MediaCardControl" ... />`.

Replace `src/Plith/Views/OsdContent.xaml.cs`:

```csharp
using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdContent()
    {
        InitializeComponent();
        DataContextChanged += (_, e) =>
        {
            if (e.OldValue is OsdViewModel old) old.Media.CommandRequested -= OnCommandRequested;
            if (e.NewValue is OsdViewModel now) now.Media.CommandRequested += OnCommandRequested;
        };
    }

    private void OnCommandRequested(MediaCommand command) => MediaCommandInvoked?.Invoke(this, command);
}
```

- [ ] **Step 7: Run all tests and the build**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS.

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 8: Manual smoke check**

Run: `dotnet run --project src/Plith/Plith.csproj`
Confirm: the OSD still pops on a volume change, the media card still appears with an active session, and the prev / play-pause / next buttons still control playback. This is the first task that touches a live interaction path, so it gets a manual check rather than a test-only gate.

- [ ] **Step 9: Commit**

```bash
git add -A src/Plith tests/Plith.Tests
git commit -m "refactor(media): route transport commands via view model"
```

---

### Task 4: Extract AudioCardView from OsdContent

Lifts the volume row into its own UserControl. `OsdContent` remains a StackPanel; it now stacks two UserControls instead of one UserControl and an inline Grid. Output must be pixel-identical.

**Files:**
- Create: `src/Plith/Views/AudioCardView.xaml`, `src/Plith/Views/AudioCardView.xaml.cs`
- Modify: `src/Plith/Views/OsdContent.xaml`

**Interfaces:**
- Consumes: `Plith.ViewModels.AudioCardViewModel` (Task 2)
- Produces: `Plith.Views.AudioCardView` — a `UserControl` whose DataContext is an `AudioCardViewModel`

- [ ] **Step 1: Create AudioCardView**

Create `src/Plith/Views/AudioCardView.xaml` holding the volume-row `Grid` moved out of `OsdContent.xaml` verbatim, with the `Audio.` binding prefixes dropped (the DataContext is now the `AudioCardViewModel` itself):

```xml
<UserControl x:Class="Plith.Views.AudioCardView"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:vm="clr-namespace:Plith.ViewModels"
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
             d:DataContext="{d:DesignInstance Type=vm:AudioCardViewModel}"
             mc:Ignorable="d">
    <Grid>
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="10" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <DockPanel Grid.Row="0" LastChildFill="True">
            <TextBlock DockPanel.Dock="Right"
                       Style="{StaticResource OsdValueStyle}"
                       Text="{Binding GainText}" />
            <TextBlock Style="{StaticResource OsdLabelStyle}"
                       VerticalAlignment="Center"
                       TextTrimming="CharacterEllipsis"
                       Text="{Binding Label}" />
        </DockPanel>

        <Grid Grid.Row="2" Height="6">
            <Border CornerRadius="3" Background="{DynamicResource OsdTrackBg}" />
            <Border HorizontalAlignment="Left"
                    CornerRadius="3"
                    Background="{Binding GainColor}">
                <Border.Width>
                    <MultiBinding>
                        <MultiBinding.Converter>
                            <vm:NormalizedToWidthConverter />
                        </MultiBinding.Converter>
                        <Binding Path="GainNormalized" />
                        <Binding Path="ActualWidth" RelativeSource="{RelativeSource AncestorType=Grid}" />
                    </MultiBinding>
                </Border.Width>
            </Border>
        </Grid>
    </Grid>
</UserControl>
```

Create `src/Plith/Views/AudioCardView.xaml.cs`:

```csharp
using System.Windows.Controls;

namespace Plith.Views;

public partial class AudioCardView : UserControl
{
    public AudioCardView() => InitializeComponent();
}
```

**Watch the `RelativeSource AncestorType=Grid` binding.** It resolves to the nearest ancestor `Grid`, which inside this UserControl is the `Grid Grid.Row="2" Height="6"` that wraps both `Border`s — exactly the same element it resolved to inside `OsdContent`. The bar width is therefore unchanged. If you restructure this markup, re-check that binding first; it is the one thing here that silently produces a wrong-width bar rather than a build error.

- [ ] **Step 2: Replace the inline volume row in OsdContent**

In `src/Plith/Views/OsdContent.xaml`, replace the entire `<!-- Volume row -->` `Grid` with:

```xml
<views:AudioCardView DataContext="{Binding Audio}" />
```

- [ ] **Step 3: Build**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 4: Capture the pixel baseline**

Run: `dotnet run --project src/Plith/Plith.csproj`

Trigger a volume change and screenshot the OSD twice: once with no media session, once with a session active. Save both to `docs/screenshots/phase5-baseline-audio.png` and `docs/screenshots/phase5-baseline-audio-media.png`.

These are the reference images for the Task 6 comparison and for the Phase 5 verification gate. Capture them **now**, at the last point where the OSD still renders through the original StackPanel — capturing them after the ItemsControl swap would compare the new build against itself and prove nothing.

- [ ] **Step 5: Commit**

```bash
git add -A src/Plith docs/screenshots
git commit -m "refactor(osd): extract AudioCardView from OsdContent"
```

---

### Task 5: AudioCard and MediaCard

The two `ICard` implementations. Still not wired into rendering — Task 6 does that — so this task is pure additive code plus tests.

**Files:**
- Create: `src/Plith/Cards/AudioCard.cs`
- Create: `src/Plith/Cards/MediaCard.cs`
- Test: `tests/Plith.Tests/AudioCardTests.cs`, `tests/Plith.Tests/MediaCardTests.cs`

**Interfaces:**
- Consumes: `ICard`, `ShowRequest`, `ShowReason` (Task 1); `AudioCardViewModel` (Task 2); `MediaViewModel`, `MediaViewModel.CommandRequested` (Task 3); `SettingsService`, `MediaSnapshot` (existing)
- Produces:
  - `AudioCard : ICard` — `Id = "audio"`, `Order = 20`, `IsVisible => true`, `AudioCardViewModel Vm { get; }`, `void Apply(string label, double normalized, string text, bool muted)`, `void ResetBaseline()`
  - `MediaCard : ICard` — `Id = "media"`, `Order = 10`, `MediaViewModel Vm { get; }`, `void Apply(MediaSnapshot snapshot)`, `event Action<Plith.Views.MediaCommand>? CommandInvoked`

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/AudioCardTests.cs`:

```csharp
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class AudioCardTests
{
    private static SettingsService NewSettings()
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        return new SettingsService(path);
    }

    private static (AudioCard card, List<ShowRequest> shows) NewCard()
    {
        var card = new AudioCard(NewSettings());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);
        return (card, shows);
    }

    [Fact]
    public void FirstApply_IsSilentBaseline()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void SecondApplyWithDifferentValue_RaisesShowRequested()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.6, "+2.0 dB", muted: false);

        Assert.Single(shows);
        Assert.Equal(ShowReason.AudioChange, shows[0].Reason);
        Assert.Equal("audio", shows[0].OriginCardId);
    }

    [Fact]
    public void ApplyWithUnchangedValue_RaisesNothing()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void ApplyWithinEpsilon_RaisesNothing()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        // 0.0005 is the orchestrator's original threshold; 0.0001 must stay below it.
        card.Apply("Bus A1", 0.5001, "0.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void MuteFlipAtUnchangedGain_RaisesShowRequested()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: true);
        Assert.Single(shows);
    }

    [Fact]
    public void ResetBaseline_MakesTheNextApplySilentAgain()
    {
        var (card, shows) = NewCard();
        card.Apply("Bus A1", 0.5, "0.0 dB", muted: false);
        card.ResetBaseline();
        card.Apply("Bus A1", 0.9, "+10.0 dB", muted: false);
        Assert.Empty(shows);
    }

    [Fact]
    public void Apply_AlwaysUpdatesTheViewModelEvenWhenSilent()
    {
        var (card, _) = NewCard();
        card.Apply("Speakers (G733)", 0.75, "75%", muted: false);

        Assert.Equal("Speakers (G733)", card.Vm.Label);
        Assert.Equal(0.75, card.Vm.GainNormalized);
        Assert.Equal("75%", card.Vm.GainText);
    }

    [Fact]
    public void IsVisible_IsAlwaysTrue()
    {
        var (card, _) = NewCard();
        Assert.True(card.IsVisible);
    }
}
```

Create `tests/Plith.Tests/MediaCardTests.cs`:

```csharp
using Plith.Cards;
using Plith.Services;
using Plith.Views;

namespace Plith.Tests;

public class MediaCardTests
{
    private static SettingsService NewSettings(bool autoShowOnMedia = true, bool compactMode = false)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        var m = svc.Current.Clone();
        m.AutoShowOnMedia = autoShowOnMedia;
        m.CompactMode = compactMode;
        svc.Save(m);
        return svc;
    }

    private static MediaSnapshot Playing(string title = "Sample track")
        => new(title, "Sample artist", null, IsPlaying: true, HasSession: true);

    private static MediaSnapshot NoSession()
        => new("", "", null, IsPlaying: false, HasSession: false);

    [Fact]
    public void Apply_WithAutoShowOn_RaisesShowRequested()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing());

        Assert.Single(shows);
        Assert.Equal(ShowReason.MediaChange, shows[0].Reason);
        Assert.Equal("media", shows[0].OriginCardId);
    }

    [Fact]
    public void Apply_WithAutoShowOff_RaisesNothing()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: false));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing());

        Assert.Empty(shows);
    }

    [Fact]
    public void Apply_WithNoSession_RaisesNothingEvenWithAutoShowOn()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(NoSession());

        Assert.Empty(shows);
    }

    [Fact]
    public void IsVisible_IsFalseWithoutASession()
    {
        var card = new MediaCard(NewSettings());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void IsVisible_IsTrueWithASession()
    {
        var card = new MediaCard(NewSettings());
        card.Apply(Playing());
        Assert.True(card.IsVisible);
    }

    [Fact]
    public void IsVisible_IsFalseInCompactModeDespiteASession()
    {
        var card = new MediaCard(NewSettings(compactMode: true));
        card.Apply(Playing());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void SessionAppearing_RaisesVisibilityChanged()
    {
        var card = new MediaCard(NewSettings());
        card.Activate();
        int changes = 0;
        card.VisibilityChanged += () => changes++;

        card.Apply(Playing());

        Assert.True(changes > 0);
    }

    [Fact]
    public void CompactModeToggle_RaisesVisibilityChanged()
    {
        var settings = NewSettings(compactMode: false);
        var card = new MediaCard(settings);
        card.Activate();
        card.Apply(Playing());

        int changes = 0;
        card.VisibilityChanged += () => changes++;

        var m = settings.Current.Clone();
        m.CompactMode = true;
        settings.Save(m);

        Assert.True(changes > 0);
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void ViewModelCommandRequest_SurfacesAsCommandInvoked()
    {
        var card = new MediaCard(NewSettings());
        MediaCommand? seen = null;
        card.CommandInvoked += (_, c) => seen = c;

        card.Vm.RequestCommand(MediaCommand.SkipNext);

        Assert.Equal(MediaCommand.SkipNext, seen);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter "FullyQualifiedName~AudioCardTests|FullyQualifiedName~MediaCardTests"`
Expected: compile failure — `The type or namespace name 'AudioCard' could not be found`.

- [ ] **Step 3: Write AudioCard**

Create `src/Plith/Cards/AudioCard.cs`:

```csharp
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// Volume card. Always visible — the OSD has no state in which it shows nothing about audio.
///
/// Owns the baseline-suppression rule that used to live in OsdOrchestrator.HandleValueChange:
/// the first read after attaching to a source establishes a baseline silently, so switching
/// sources doesn't pop the OSD with whatever value the new source happened to hold.
/// </summary>
public sealed class AudioCard : ICard
{
    // Matches the threshold OsdOrchestrator used before the refactor. Small enough to catch a
    // single volume-key step, large enough to ignore float noise from repeated reads.
    private const double ChangeEpsilon = 0.0005;

    private readonly SettingsService _settings;
    private float? _lastNormalized;
    private bool? _lastMuted;

    public AudioCard(SettingsService settings)
    {
        _settings = settings;
        Vm = new AudioCardViewModel { UseColorThresholds = settings.Current.UseColorThresholds };
    }

    public string Id => "audio";
    public int Order => 20;
    public bool IsVisible => true;
    public object ViewModel => Vm;
    public AudioCardViewModel Vm { get; }

    // Never raised: IsVisible is a constant. Empty accessors rather than a field-like event,
    // because a field-like event that is never invoked trips CS0067.
    public event Action? VisibilityChanged { add { } remove { } }
    public event Action<ShowRequest>? ShowRequested;

    public void Activate() => _settings.Changed += OnSettingsChanged;
    public void Deactivate() => _settings.Changed -= OnSettingsChanged;

    public void OnThemeChanged() => Vm.RefreshThresholdBrushes();

    private void OnSettingsChanged(SettingsModel m) => Vm.UseColorThresholds = m.UseColorThresholds;

    /// <summary>
    /// Applies a normalized 0..1 value plus a pre-formatted display string. The view model is
    /// updated on every call; the show request fires only on a real change after a baseline.
    /// </summary>
    public void Apply(string label, double normalized, string text, bool muted)
    {
        bool isFirstRead = _lastNormalized is null;
        bool changed = isFirstRead
            || Math.Abs(_lastNormalized!.Value - normalized) > ChangeEpsilon
            || _lastMuted != muted;

        _lastNormalized = (float)normalized;
        _lastMuted = muted;

        Vm.Apply(label, normalized, text, muted);

        if (changed && !isFirstRead)
            ShowRequested?.Invoke(new ShowRequest(ShowReason.AudioChange, Id));
    }

    /// <summary>Forget the baseline so the next <see cref="Apply"/> is silent. Called on every
    /// audio-source transition — the new source's first value is not a user action.</summary>
    public void ResetBaseline()
    {
        _lastNormalized = null;
        _lastMuted = null;
    }
}
```

- [ ] **Step 4: Write MediaCard**

Create `src/Plith/Cards/MediaCard.cs`:

```csharp
using Plith.Services;
using Plith.ViewModels;
using Plith.Views;

namespace Plith.Cards;

/// <summary>
/// Now-playing card. Visible only while an SMTC session exists and the user hasn't asked for
/// compact mode — CompactMode means exactly "hide the media card" and nothing else, so the
/// rule belongs here rather than in the shell.
/// </summary>
public sealed class MediaCard : ICard
{
    private readonly SettingsService _settings;
    private bool _lastVisible;

    public MediaCard(SettingsService settings)
    {
        _settings = settings;
        Vm = new MediaViewModel();
        Vm.HasSessionChanged += OnHasSessionChanged;
        Vm.CommandRequested += OnCommandRequested;
        _lastVisible = IsVisible;
    }

    public string Id => "media";
    public int Order => 10;
    public object ViewModel => Vm;
    public MediaViewModel Vm { get; }

    public bool IsVisible => Vm.HasSession && !_settings.Current.CompactMode;

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    /// <summary>Raised when the user clicks a transport button. The orchestrator dispatches it
    /// to the SMTC session.</summary>
    public event EventHandler<MediaCommand>? CommandInvoked;

    public void Activate() => _settings.Changed += OnSettingsChanged;

    public void Deactivate() => _settings.Changed -= OnSettingsChanged;

    public void Apply(MediaSnapshot snapshot)
    {
        Vm.Apply(snapshot);
        RaiseVisibilityIfChanged();

        if (_settings.Current.AutoShowOnMedia && snapshot.HasSession)
            ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaChange, Id));
    }

    private void OnSettingsChanged(SettingsModel m) => RaiseVisibilityIfChanged();

    private void OnHasSessionChanged() => RaiseVisibilityIfChanged();

    private void OnCommandRequested(MediaCommand command)
    {
        CommandInvoked?.Invoke(this, command);
        ShowRequested?.Invoke(new ShowRequest(ShowReason.MediaCommand, Id));
    }

    // Both inputs to IsVisible (HasSession, CompactMode) change independently, and either can
    // fire without the result actually flipping. Gate on the computed value so CardHost isn't
    // asked to reconcile on every settings save.
    private void RaiseVisibilityIfChanged()
    {
        bool now = IsVisible;
        if (now == _lastVisible) return;
        _lastVisible = now;
        VisibilityChanged?.Invoke();
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter "FullyQualifiedName~AudioCardTests|FullyQualifiedName~MediaCardTests"`
Expected: PASS, 17 tests.

If the analyzer objects to the empty `VisibilityChanged` accessors on `AudioCard`, do **not** switch to a field-like event — that trades one warning for CS0067. Suppress the specific rule inline with a justification comment, matching how `App.xaml.cs` already suppresses CA1001.

- [ ] **Step 6: Run the full suite and build**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Run: `dotnet build Plith.slnx -c Debug`
Expected: PASS, `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Cards tests/Plith.Tests
git commit -m "feat(cards): add AudioCard and MediaCard"
```

---

### Task 6: Swap the render path onto CardHost

The task everything else was staged for. Every piece it depends on already exists and is tested; this wires them together and deletes `OsdViewModel`.

**Files:**
- Create: `src/Plith/ViewModels/OsdShellViewModel.cs`
- Create: `src/Plith/Resources/CardTemplates.xaml`
- Modify: `src/Plith/App.xaml`, `src/Plith/App.xaml.cs`
- Modify: `src/Plith/Views/OsdContent.xaml`, `src/Plith/Views/OsdContent.xaml.cs`
- Modify: `src/Plith/Views/OsdHost.cs`
- Modify: `src/Plith/Services/OsdOrchestrator.cs`
- Modify: `src/Plith/Views/SettingsPreview.xaml`, `src/Plith/Views/SettingsPreview.xaml.cs`
- Delete: `src/Plith/ViewModels/OsdViewModel.cs`

**Interfaces:**
- Consumes: `CardHost`, `ShowRequest`, `ShowReason` (Task 1); `AudioCardViewModel` (Task 2); `MediaCardView` (Task 3); `AudioCardView` (Task 4); `AudioCard`, `MediaCard` (Task 5)
- Produces: `OsdShellViewModel` with `ObservableCollection<ICard> VisibleCards`; `OsdHost.Shell` (`OsdShellViewModel`); `OsdHost.HideOsd()`

- [ ] **Step 1: Create the shell view model**

Create `src/Plith/ViewModels/OsdShellViewModel.cs`:

```csharp
using System.Collections.ObjectModel;
using Plith.Cards;

namespace Plith.ViewModels;

/// <summary>
/// Binding root for OsdContent. Phase 5 needs nothing beyond the visible-card list — the
/// collection instance comes straight from CardHost, so no change-forwarding is needed.
/// Shell-level state (notch height, preset mode) arrives in Phase 6.
/// </summary>
public sealed class OsdShellViewModel
{
    public OsdShellViewModel(CardHost host) => VisibleCards = host.VisibleCards;

    public ObservableCollection<ICard> VisibleCards { get; }
}
```

- [ ] **Step 2: Create the card templates dictionary**

Create `src/Plith/Resources/CardTemplates.xaml`:

```xml
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
                    xmlns:vm="clr-namespace:Plith.ViewModels"
                    xmlns:views="clr-namespace:Plith.Views">

    <!-- Implicit DataType templates: ItemsControl resolves each card's view from its
         view-model type, which is what keeps ICard free of any view reference. Adding a
         Phase 6 card means adding one DataTemplate here and nothing else in this file. -->
    <DataTemplate DataType="{x:Type vm:AudioCardViewModel}">
        <views:AudioCardView />
    </DataTemplate>

    <DataTemplate DataType="{x:Type vm:MediaViewModel}">
        <views:MediaCardView />
    </DataTemplate>

</ResourceDictionary>
```

In `src/Plith/App.xaml`, add `<ResourceDictionary Source="Resources/CardTemplates.xaml" />` to `Application.Resources`'s `MergedDictionaries`, **after** the existing theme and palette dictionaries so card templates can reference their brush keys.

- [ ] **Step 3: Rewrite OsdContent**

Replace the `StackPanel` inside the `Border` in `src/Plith/Views/OsdContent.xaml` with:

```xml
<ItemsControl ItemsSource="{Binding VisibleCards}" AlternationCount="64">
    <ItemsControl.ItemsPanel>
        <ItemsPanelTemplate>
            <StackPanel Orientation="Vertical" />
        </ItemsPanelTemplate>
    </ItemsControl.ItemsPanel>
    <ItemsControl.ItemTemplate>
        <DataTemplate>
            <!-- Tag carries the alternation index because an ELEMENT-level binding resolves
                 AncestorType=ContentPresenter to the item container, and a template-trigger
                 binding does not. See the warning below the block. -->
            <StackPanel x:Name="ItemRoot"
                        Tag="{Binding RelativeSource={RelativeSource AncestorType=ContentPresenter},
                                      Path=(ItemsControl.AlternationIndex)}">
                <!-- Separator above every card except the first. In 0.1.5 the divider sat
                     between the media row and the volume row with this exact margin; placing
                     it above each non-first card reproduces that spacing. A Collapsed element
                     contributes nothing to StackPanel layout, so the single-card case is
                     byte-identical to the old collapsed-divider case. -->
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

⚠️ **Do not move the `AncestorType=ContentPresenter` binding into the `DataTrigger`.** An
earlier revision of this plan did exactly that, and it shipped a visible regression. A
template trigger's binding evaluates against the item container, and `FindAncestor` starts
at the container's *parent* — so the search walks past the item's own `ContentPresenter`
and lands on an outer one (`OsdContent`'s UserControl template, then `BandWindow`'s
`ContentControl` template) where `ItemsControl.AlternationIndex` is unset and returns its
default `0`. The trigger then matches **every** item and collapses **every** divider. Since
a `Collapsed` element contributes nothing to `StackPanel` layout, that costs
`14 + 1 + 14 = 29 px` of separation and shortens the whole card, which also shifts it
against a centre anchor. This was measured directly in an isolated WPF layout harness, not
inferred. Element-level bindings resolve the ancestor correctly; template-trigger bindings
do not.

Change the design-time DataContext on the `UserControl` element from `vm:OsdViewModel` to `vm:OsdShellViewModel`. Leave `Width="440"`, the outer `Grid Margin="14" Background="Transparent"`, and the `Border` (corner radius, padding, brushes, drop shadow) exactly as they are — that is shell chrome and changing it breaks the pixel baseline.

`AlternationCount="64"` is what makes `ItemsControl.AlternationIndex` report a true index rather than cycling; 64 is far above any plausible card count.

Replace `src/Plith/Views/OsdContent.xaml.cs`:

```csharp
using System.Windows.Controls;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    public OsdContent() => InitializeComponent();
}
```

Media commands now travel `MediaCardView` → `MediaViewModel.CommandRequested` → `MediaCard.CommandInvoked` → orchestrator, so `OsdContent` no longer relays them.

- [ ] **Step 4: Rewire OsdHost**

In `src/Plith/Views/OsdHost.cs`:

- Constructor signature becomes `public OsdHost(SettingsService settings, ThemeService theme, CardHost cardHost)`.
- Replace `public OsdViewModel ViewModel { get; } = new();` with `public OsdShellViewModel Shell { get; }`, assigned in the constructor as `Shell = new OsdShellViewModel(cardHost);`. **Delete** the `ViewModel` property rather than retyping it, so any consumer left behind surfaces as a compile error instead of silently binding to the wrong thing.
- `_content = new OsdContent { DataContext = Shell };`
- Delete the `public event EventHandler<MediaCommand>? MediaCommandInvoked;` declaration and the `_content.MediaCommandInvoked += ...` subscription.
- In the `_settings.Changed` lambda, delete the `ViewModel.Audio.UseColorThresholds = ...` and `ViewModel.CompactMode = ...` lines — `AudioCard` and `MediaCard` own those now. Keep the `Reposition()` call. Delete the two matching seed assignments in the constructor.
- Store the card host: `private readonly CardHost _cardHost;`
- In `ExitPositionEditMode`, replace the trailing `ShowOsd(TimeSpan.FromMilliseconds(Math.Max(_settings.Current.ShowDurationMs, 1500)));` with:

```csharp
_cardHost.RequestShow(new ShowRequest(
    ShowReason.EditModeExit,
    null,
    TimeSpan.FromMilliseconds(Math.Max(_settings.Current.ShowDurationMs, 1500))));
```

- Add `HideOsd`, next to `ShowOsd`:

```csharp
/// <summary>Take the OSD down now, ignoring its hide timer. Used when the suppression gate
/// closes while the card is already on screen.</summary>
public void HideOsd()
{
    if (_isEditMode) return;   // edit mode owns its own always-on visibility
    _hideTimer?.Stop();
    if (Opacity < 0.01) return;
    FadeOutAndHide();
}
```

- [ ] **Step 5: Reduce OsdOrchestrator to a source driver**

In `src/Plith/Services/OsdOrchestrator.cs`:

- Constructor becomes:

```csharp
public OsdOrchestrator(AudioCard audioCard, MediaCard mediaCard, SettingsService settings,
                       Dispatcher dispatcher, MediaSessionClient media, DiagnosticLog? log = null)
```

  It no longer takes `OsdHost`. Store the two cards and the dispatcher; drop the `_osd` field. Replace the `private readonly MediaSessionClient _media = new();` field initializer with the injected instance, and drop `_media.Dispose()` from `Dispose()` — `App` owns that lifetime from now on, because Task 9's `FullscreenVideoWatcher` needs the same client and is constructed before the orchestrator. Taking the dependency now means the signature is written once instead of changing again in Task 9.
- Delete the `_lastNormalized`, `_lastMuted`, and `VisibleFor` members, the whole `HandleValueChange` method, and the `_osd.MediaCommandInvoked += OnMediaCommandInvoked` / `-=` pair.
- Everywhere the old code did `_lastNormalized = null; _lastMuted = null;` — in `OnSettingsChanged`, in `ReconcileActiveSource`, and in `TryConnectVoicemeeter`'s success branch — call `_audioCard.ResetBaseline();` instead.
- `HandleVoicemeeterChange` ends with `_audioCard.Apply(snap.Label, normalized, text, snap.Muted);`
- `OnWindowsAudioChanged` ends with `_audioCard.Apply(snapshot.DeviceLabel, snapshot.ScalarVolume, text, snapshot.Muted);`
- `OnMediaChanged` becomes `_mediaCard.Apply(snapshot);` and loses its `AutoShowOnMedia` branch entirely — `MediaCard.Apply` owns that rule now.
- `OnMediaCommandInvoked` keeps its `switch` dispatching to `_media`, but drops the trailing `_osd.ShowOsd(VisibleFor);` — `MediaCard` raises `ShowReason.MediaCommand` itself. Subscribe it to `_mediaCard.CommandInvoked` in the constructor and unsubscribe in `Dispose`.
- `WindowsVolumeEvent` and the `NativeFlyoutSuppressor` coupling are unchanged.
- The `AudioCardViewModel.VoicemeeterMinDb` / `MaxDb` references in `HandleVoicemeeterChange` are unchanged from Task 2.

- [ ] **Step 6: Rewire App startup**

In `src/Plith/App.xaml.cs`, add fields `private CardHost? _cardHost; private AudioCard? _audioCard; private MediaCard? _mediaCard; private MediaSessionClient? _mediaSession;` and replace the OSD construction block:

```csharp
_mediaSession = new MediaSessionClient();

_audioCard = new AudioCard(_settings);
_mediaCard = new MediaCard(_settings);

_cardHost = new CardHost(_settings);
_cardHost.Register(_mediaCard);   // Order 10 — renders above
_cardHost.Register(_audioCard);   // Order 20

_osd = new OsdHost(_settings, _theme, _cardHost);
_cardHost.ShowRequested += d => _osd.ShowOsd(d);
_cardHost.HideRequested += () => _osd.HideOsd();
_cardHost.Start();

_orchestrator = new OsdOrchestrator(_audioCard, _mediaCard, _settings, _osd.Dispatcher, _mediaSession, _diagnosticLog);
_orchestrator.Start();
```

Replace the theme hook at line 45 with `_theme.ThemeApplied += () => _cardHost?.NotifyThemeChanged();`. Because `_cardHost` is constructed after `_theme.Start()`, `AudioCardViewModel`'s constructor already resolves brushes on creation, so no first-paint gap opens.

Replace the two direct show calls:

```csharp
_volumeKeyHook.VolumeKeyPressed += () =>
{
    _flyoutSuppressor?.OpenSuppressionWindow();
    Dispatcher.BeginInvoke(() => _cardHost?.RequestShow(new ShowRequest(ShowReason.VolumeKey)));
};

_hotkey.Pressed += () => _cardHost?.RequestShow(new ShowRequest(ShowReason.SummonHotkey));
```

Add to `OnExit`, immediately **after** the existing `Orchestrator` step — the orchestrator must stop feeding cards before the host deactivates them, and the session client outlives both:

```csharp
DisposeStep("CardHost",          () => _cardHost?.Dispose());
DisposeStep("MediaSessionClient", () => _mediaSession?.Dispose());
```

- [ ] **Step 7: Migrate SettingsPreview off OsdViewModel**

In `src/Plith/Views/SettingsPreview.xaml.cs`, replace the `PreviewViewModel` property with two:

```csharp
public AudioCardViewModel PreviewAudio { get; }
public MediaViewModel PreviewMedia { get; }
```

Seed them in the constructor with the same sample values, set `DataContext = this;`, and add a plain `ShowMediaCard` flag with change notification for the XAML to bind:

```csharp
public partial class SettingsPreview : UserControl, INotifyPropertyChanged
{
    public AudioCardViewModel PreviewAudio { get; } = new()
    {
        Label = "Bus A1",
        GainText = "+3.0 dB",
        GainNormalized = 0.85,
    };

    public MediaViewModel PreviewMedia { get; } = new();

    private bool _showMediaCard = true;
    public bool ShowMediaCard
    {
        get => _showMediaCard;
        private set
        {
            if (_showMediaCard == value) return;
            _showMediaCard = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ShowMediaCard)));
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    // ... existing ctor body, with DataContext = this;
}
```

`UpdateCompact(bool compact)` becomes `ShowMediaCard = !compact;` and `UpdateColorThresholds(bool thresholds)` becomes `PreviewAudio.UseColorThresholds = thresholds;`.

In `src/Plith/Views/SettingsPreview.xaml`, change the design-time DataContext to `d:DataContext="{d:DesignInstance Type=views:SettingsPreview}"`, repoint the volume-row bindings from `Audio.X` to `PreviewAudio.X`, and repoint the media block's DataContext to `{Binding PreviewMedia}`. The two `ShowMediaCard` visibility bindings keep their paths — they now resolve against the control itself.

**The preview stays a hand-built mock and must not acquire a `CardHost`.** It exists to animate live theme and toggle changes inside the Settings window; giving it a show pipeline would put a second OSD authority inside the app.

- [ ] **Step 8: Delete OsdViewModel**

```bash
git rm src/Plith/ViewModels/OsdViewModel.cs
```

- [ ] **Step 9: Build and fix every compile error**

Run: `dotnet build Plith.slnx -c Debug`

Every remaining reference to `OsdViewModel`, `OsdHost.ViewModel`, or `OsdHost.MediaCommandInvoked` now fails to compile. That is the point of deleting rather than retyping — work through the list; there should be none beyond the files named in this task.

Expected once clean: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 10: Run the full test suite**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: PASS.

- [ ] **Step 11: Verify pixel-identical rendering**

Run: `dotnet run --project src/Plith/Plith.csproj`

Capture the same two screenshots as Task 4 (volume-only, and volume with an active media session) and compare against `docs/screenshots/phase5-baseline-audio.png` and `phase5-baseline-audio-media.png`.

They must match. **This comparison is the only real evidence for the "zero user-visible change" success metric — a green test suite is not.** If the spacing is off by a pixel or two, the divider margin or the `Border` padding moved; re-read Step 3 before adjusting anything else.

Also confirm by hand:
- Volume change pops the OSD; a source switch (start or stop Voicemeeter) does **not** pop it.
- The media card appears and disappears with the session, and the OSD recentres when it does.
- Transport buttons still control playback and keep the OSD alive.
- Compact mode hides the media card.
- The summon hotkey and the volume keys still pop the OSD.
- Position edit mode still enters, drags, saves, and cancels.

- [ ] **Step 12: Commit**

```bash
git add -A src/Plith tests/Plith.Tests
git commit -m "refactor(osd): render through CardHost"
```

---

# Stage B — Fullscreen video suppression (Tasks 7–9)

---

### Task 7: The suppression decision function

A pure static function, tested exhaustively before any Win32 code exists to call it.

**Files:**
- Create: `src/Plith/Services/FullscreenVideoDetector.cs`
- Test: `tests/Plith.Tests/FullscreenVideoDetectorTests.cs`

**Interfaces:**
- Produces: `internal static class FullscreenVideoDetector` with
  `static bool ShouldSuppress(bool enabled, bool foregroundCoversMonitor, uint notificationState, bool foregroundOwnsPlayingSmtc, string foregroundProcessName, IReadOnlyCollection<string> hideList)`
  and `static IReadOnlyCollection<string> ParseHideList(string? raw)`, plus
  `const uint QUNS_RUNNING_D3D_FULL_SCREEN = 3`, `const uint QUNS_BUSY = 2`.

The type is `internal`; add `[assembly: InternalsVisibleTo("Plith.Tests")]` to `src/Plith/AssemblyInfo.cs` if it isn't there already.

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/FullscreenVideoDetectorTests.cs`:

```csharp
using Plith.Services;

namespace Plith.Tests;

public class FullscreenVideoDetectorTests
{
    private static readonly string[] DefaultList = { "mpv", "PotPlayerMini64" };

    private const uint D3D = FullscreenVideoDetector.QUNS_RUNNING_D3D_FULL_SCREEN;
    private const uint Busy = FullscreenVideoDetector.QUNS_BUSY;

    [Fact]
    public void Disabled_NeverSuppresses()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: false, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "vlc", hideList: DefaultList));
    }

    [Fact]
    public void NotFullscreen_DoesNotSuppress()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: false, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "vlc", hideList: DefaultList));
    }

    [Fact]
    public void ExclusiveFullscreenGame_DoesNotSuppress_EvenWithPlayingMedia()
    {
        // The whole point of Phase 4h: an exclusive-fullscreen game must never lose the OSD,
        // even if Spotify happens to be playing behind it.
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: D3D,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "cs2", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenBrowserPlayingMedia_Suppresses()
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: true, foregroundProcessName: "chrome", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenListedPlayerWithoutSmtc_Suppresses()
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "mpv", hideList: DefaultList));
    }

    [Fact]
    public void FullscreenBorderlessGame_DoesNotSuppress()
    {
        // Borderless-windowed games report QUNS_BUSY like any other fullscreen window. What
        // keeps them safe is that they own no playing SMTC session and aren't in the list.
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "valorant", hideList: DefaultList));
    }

    [Theory]
    [InlineData("MPV")]
    [InlineData("mpv.exe")]
    [InlineData("MPV.EXE")]
    public void HideListMatch_IsCaseInsensitiveAndTolerates_ExeSuffix(string processName)
    {
        Assert.True(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: processName, hideList: DefaultList));
    }

    [Fact]
    public void EmptyProcessName_DoesNotMatchAnything()
    {
        Assert.False(FullscreenVideoDetector.ShouldSuppress(
            enabled: true, foregroundCoversMonitor: true, notificationState: Busy,
            foregroundOwnsPlayingSmtc: false, foregroundProcessName: "", hideList: DefaultList));
    }

    [Fact]
    public void ParseHideList_SplitsTrimsAndDropsEmptyEntries()
    {
        var parsed = FullscreenVideoDetector.ParseHideList(" mpv , ,PotPlayerMini64 ,");
        Assert.Equal(new[] { "mpv", "PotPlayerMini64" }, parsed);
    }

    [Fact]
    public void ParseHideList_HandlesNullAndEmpty()
    {
        Assert.Empty(FullscreenVideoDetector.ParseHideList(null));
        Assert.Empty(FullscreenVideoDetector.ParseHideList("   "));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~FullscreenVideoDetectorTests`
Expected: compile failure — `The name 'FullscreenVideoDetector' does not exist`.

- [ ] **Step 3: Write the detector**

Create `src/Plith/Services/FullscreenVideoDetector.cs`:

```csharp
namespace Plith.Services;

/// <summary>
/// The suppression decision, isolated from every Win32 call so it can be tested exhaustively.
/// FullscreenVideoWatcher gathers the inputs; this decides.
///
/// The rule fails toward "do not hide" in every ambiguous case. A user running neither a
/// listed player nor a media-publishing app sees byte-identical 0.1.5 behaviour, and an
/// exclusive-fullscreen game can never be suppressed no matter what else matches.
/// </summary>
internal static class FullscreenVideoDetector
{
    /// <summary>SHQueryUserNotificationState: a full-screen (non-D3D) window is running.</summary>
    public const uint QUNS_BUSY = 2;

    /// <summary>SHQueryUserNotificationState: a D3D exclusive-fullscreen app is running.
    /// This is the games case and is a hard veto.</summary>
    public const uint QUNS_RUNNING_D3D_FULL_SCREEN = 3;

    public static bool ShouldSuppress(
        bool enabled,
        bool foregroundCoversMonitor,
        uint notificationState,
        bool foregroundOwnsPlayingSmtc,
        string foregroundProcessName,
        IReadOnlyCollection<string> hideList)
    {
        if (!enabled) return false;
        if (!foregroundCoversMonitor) return false;
        if (notificationState == QUNS_RUNNING_D3D_FULL_SCREEN) return false;

        return foregroundOwnsPlayingSmtc || MatchesHideList(foregroundProcessName, hideList);
    }

    private static bool MatchesHideList(string processName, IReadOnlyCollection<string> hideList)
    {
        if (string.IsNullOrWhiteSpace(processName)) return false;

        var bare = TrimExe(processName);
        foreach (var entry in hideList)
        {
            if (string.Equals(bare, TrimExe(entry), StringComparison.OrdinalIgnoreCase))
                return true;
        }
        return false;
    }

    // Process.ProcessName never carries the extension, but a user typing the hide list by hand
    // will write "mpv.exe" as often as "mpv". Normalise both sides.
    private static string TrimExe(string name) =>
        name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
            ? name[..^4]
            : name;

    public static IReadOnlyCollection<string> ParseHideList(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return Array.Empty<string>();
        return raw.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~FullscreenVideoDetectorTests`
Expected: PASS, 12 tests (the `[Theory]` contributes 3).

- [ ] **Step 5: Build**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Services/FullscreenVideoDetector.cs src/Plith/AssemblyInfo.cs tests/Plith.Tests/FullscreenVideoDetectorTests.cs
git commit -m "feat(osd): add fullscreen video suppression rule"
```

---

### Task 8: Expose the SMTC session owner

`ForegroundOwnsPlayingSmtc` needs to know which app owns the current media session.

**Files:**
- Modify: `src/Plith/Services/MediaSessionClient.cs`

**Interfaces:**
- Produces: `MediaSnapshot` gains a trailing `string SourceAppUserModelId` member; `MediaSessionClient.CurrentSourceAppUserModelId` (`string`, empty when there is no session) and `MediaSessionClient.IsCurrentSessionPlaying` (`bool`).

- [ ] **Step 1: Extend the snapshot record**

In `src/Plith/Services/MediaSessionClient.cs`, add a trailing member to `MediaSnapshot`:

```csharp
public sealed record MediaSnapshot(
    string Title,
    string Artist,
    byte[]? ThumbnailBytes,
    bool IsPlaying,
    bool HasSession,
    string SourceAppUserModelId = "");
```

A defaulted trailing parameter keeps every existing positional construction — including the ones in `MediaCardTests` from Task 5 — compiling unchanged.

- [ ] **Step 2: Populate it**

`MediaSnapshot` is constructed in exactly two places, both inside `EmitSnapshotAsync`:

- the no-session early return (`MediaSessionClient.cs:101`) — leave the positional arguments as they are; the defaulted empty AUMID is correct there. Immediately before it, set `CurrentSourceAppUserModelId = string.Empty; IsCurrentSessionPlaying = false;`
- the live-session emit (`MediaSessionClient.cs:131`) — becomes:

```csharp
var aumid = string.Empty;
try { aumid = session.SourceAppUserModelId ?? string.Empty; } catch { /* session died mid-read */ }

CurrentSourceAppUserModelId = aumid;
IsCurrentSessionPlaying = playing;

Changed?.Invoke(new MediaSnapshot(title, artist, thumb, playing, HasSession: true, aumid));
```

Add the two properties:

```csharp
/// <summary>AUMID of the app owning the current session, or empty when there is none.
/// Used by FullscreenVideoWatcher to decide whether the foreground window is playing media.</summary>
public string CurrentSourceAppUserModelId { get; private set; } = string.Empty;

/// <summary>True while the current session reports Playing.</summary>
public bool IsCurrentSessionPlaying { get; private set; }
```

- [ ] **Step 3: Build and test**

Run: `dotnet build Plith.slnx -c Debug`
Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: `Build succeeded.`, `3 Warning(s)`; tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/Plith/Services/MediaSessionClient.cs
git commit -m "feat(media): expose SMTC session owner and play state"
```

---

### Task 9: FullscreenVideoWatcher, settings, and wiring

**Files:**
- Create: `src/Plith/Services/FullscreenVideoWatcher.cs`
- Modify: `src/Plith/Services/SettingsModel.cs`, `src/Plith/Services/SettingsService.cs`
- Modify: `src/Plith/Views/SettingsWindow.xaml`, `src/Plith/Views/SettingsWindow.xaml.cs`
- Modify: `src/Plith/App.xaml.cs`
- Test: `tests/Plith.Tests/SettingsServiceTests.cs` (extend)

**Interfaces:**
- Consumes: `FullscreenVideoDetector` (Task 7), `MediaSessionClient.CurrentSourceAppUserModelId` / `IsCurrentSessionPlaying` (Task 8), `IShowSuppressor` (Task 1)
- Produces: `FullscreenVideoWatcher : IShowSuppressor, IDisposable` with `void Start()`; `SettingsModel.HideDuringFullscreenVideo` (`bool`, default `true`), `SettingsModel.FullscreenVideoHideList` (`string`, default `"mpv,PotPlayerMini64"`)

- [ ] **Step 1: Write the failing settings round-trip test**

Append to `tests/Plith.Tests/SettingsServiceTests.cs`:

```csharp
[Fact]
public void FullscreenVideoSettings_RoundTripThroughIni()
{
    var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
    var svc = new SettingsService(path);

    var m = svc.Current.Clone();
    m.HideDuringFullscreenVideo = false;
    m.FullscreenVideoHideList = "mpv,vlc";
    svc.Save(m);

    var reloaded = new SettingsService(path);
    reloaded.Load();

    Assert.False(reloaded.Current.HideDuringFullscreenVideo);
    Assert.Equal("mpv,vlc", reloaded.Current.FullscreenVideoHideList);
}

[Fact]
public void FullscreenVideoSettings_DefaultToEnabledWithSeededHideList()
{
    var svc = new SettingsService(Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini"));
    svc.Load();   // no file on disk -> defaults

    Assert.True(svc.Current.HideDuringFullscreenVideo);
    Assert.Equal("mpv,PotPlayerMini64", svc.Current.FullscreenVideoHideList);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettingsServiceTests`
Expected: compile failure — `'SettingsModel' does not contain a definition for 'HideDuringFullscreenVideo'`.

- [ ] **Step 3: Add the settings fields**

In `src/Plith/Services/SettingsModel.cs`, next to the other OSD-behaviour fields:

```csharp
/// <summary>Hide the OSD while a fullscreen window is playing media. Games are exempt —
/// see FullscreenVideoDetector for the rule.</summary>
public bool HideDuringFullscreenVideo { get; set; } = true;

/// <summary>Comma-separated process names treated as fullscreen video players even when they
/// publish no SMTC session. Escape hatch for players like mpv.</summary>
public string FullscreenVideoHideList { get; set; } = "mpv,PotPlayerMini64";
```

Add both to `Clone()`.

In `src/Plith/Services/SettingsService.cs`, add to the `Load` initializer under `SectionOsd`:

```csharp
HideDuringFullscreenVideo = ParseBool(data[SectionOsd]["HideDuringFullscreenVideo"], true),
FullscreenVideoHideList = string.IsNullOrWhiteSpace(data[SectionOsd]["FullscreenVideoHideList"])
    ? "mpv,PotPlayerMini64"
    : data[SectionOsd]["FullscreenVideoHideList"],
```

and to `Save`:

```csharp
data[SectionOsd]["HideDuringFullscreenVideo"] = m.HideDuringFullscreenVideo.ToString(inv);
data[SectionOsd]["FullscreenVideoHideList"] = m.FullscreenVideoHideList ?? string.Empty;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~SettingsServiceTests`
Expected: PASS.

- [ ] **Step 5: Write the watcher**

Create `src/Plith/Services/FullscreenVideoWatcher.cs`:

```csharp
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Threading;
using Plith.Cards;

namespace Plith.Services;

/// <summary>
/// Watches for a fullscreen window that is playing video and reports it as a suppression
/// state to <see cref="CardHost"/>.
///
/// Two triggers, because neither alone is sufficient: the foreground WinEvent hook catches
/// alt-tabbing into a player, and the 1 s poll catches entering fullscreen with F11 — which
/// changes no foreground window. Each evaluation is one GetForegroundWindow, one
/// GetWindowRect, one SHQueryUserNotificationState and a string compare, so 1 Hz is cheap.
/// </summary>
public sealed class FullscreenVideoWatcher : IShowSuppressor, IDisposable
{
    private const uint EVENT_SYSTEM_FOREGROUND = 0x0003;
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(1);

    private readonly SettingsService _settings;
    private readonly MediaSessionClient _media;
    private readonly DiagnosticLog? _log;
    private readonly DispatcherTimer _pollTimer;
    private nint _hook;
    private WinEventDelegate? _callback;
    private bool _suppressed;
    private bool _disposed;

    public FullscreenVideoWatcher(SettingsService settings, MediaSessionClient media, Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _settings = settings;
        _media = media;
        _log = log;
        _pollTimer = new DispatcherTimer(DispatcherPriority.Background, dispatcher) { Interval = PollInterval };
        _pollTimer.Tick += (_, _) => Evaluate();
    }

    public bool IsSuppressed => _suppressed;

    public event Action<bool>? SuppressionChanged;

    public void Start()
    {
        _callback = OnForegroundChanged;
        _hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
            hmodWinEventProc: 0, _callback,
            idProcess: 0, idThread: 0, WINEVENT_OUTOFCONTEXT);
        if (_hook == 0)
            _log?.Warn("FullscreenVideo", "SetWinEventHook failed; falling back to poll only.");

        _pollTimer.Start();
        Evaluate();
    }

    private void OnForegroundChanged(
        nint hWinEventHook, uint eventType, nint hwnd,
        int idObject, int idChild, uint idEventThread, uint dwmsEventTime) => Evaluate();

    private void Evaluate()
    {
        if (_disposed) return;

        bool next;
        try
        {
            next = FullscreenVideoDetector.ShouldSuppress(
                enabled: _settings.Current.HideDuringFullscreenVideo,
                foregroundCoversMonitor: ForegroundCoversMonitor(out var processName),
                notificationState: QueryNotificationState(),
                foregroundOwnsPlayingSmtc: ForegroundOwnsPlayingSmtc(processName),
                foregroundProcessName: processName,
                hideList: FullscreenVideoDetector.ParseHideList(_settings.Current.FullscreenVideoHideList));
        }
        catch (Exception ex)
        {
            // A window can die between GetForegroundWindow and Process lookup. Never let a
            // transient interop failure suppress the OSD — fail toward showing it.
            _log?.Warn("FullscreenVideo", $"Evaluate threw: {ex.GetType().Name}: {ex.Message}");
            next = false;
        }

        if (next == _suppressed) return;
        _suppressed = next;
        _log?.Info("FullscreenVideo", $"Suppression -> {next}");
        SuppressionChanged?.Invoke(next);
    }

    private static bool ForegroundCoversMonitor(out string processName)
    {
        processName = string.Empty;

        var hwnd = GetForegroundWindow();
        if (hwnd == 0) return false;

        // The desktop itself is technically fullscreen; never treat it as a video window.
        var cls = new char[64];
        int len = GetClassName(hwnd, cls, cls.Length);
        var className = len > 0 ? new string(cls, 0, len) : string.Empty;
        if (className is "Progman" or "WorkerW") return false;

        _ = GetWindowThreadProcessId(hwnd, out uint pid);
        if (pid != 0)
        {
            try { using var p = Process.GetProcessById((int)pid); processName = p.ProcessName; }
            catch { /* process exited between the two calls */ }
        }

        if (!GetWindowRect(hwnd, out var rect)) return false;

        // Monitor bounds come from GetMonitorInfo, NOT from WpfScreenHelper's Screen.Bounds.
        // GetWindowRect reports physical pixels while Screen.Bounds reports device-independent
        // units, so mixing them silently breaks every comparison on a non-100% DPI display —
        // a 3840-wide fullscreen window would measure 3072 against a 3840 monitor at 125% and
        // never register as fullscreen. Staying inside Win32 keeps both sides in one space.
        var monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if (monitor == 0) return false;

        var info = new MONITORINFO { cbSize = Marshal.SizeOf<MONITORINFO>() };
        if (!GetMonitorInfo(monitor, ref info)) return false;

        var b = info.rcMonitor;
        // A few pixels of tolerance: some players sit one pixel proud of the monitor edge.
        const int Tolerance = 2;
        return rect.Left <= b.Left + Tolerance
            && rect.Top <= b.Top + Tolerance
            && rect.Right >= b.Right - Tolerance
            && rect.Bottom >= b.Bottom - Tolerance;
    }

    // AUMID -> process matching is a heuristic: for Win32 apps the AUMID is in practice the
    // executable name, but for packaged apps it is a package family name that will not match
    // a process name at all. Every miss fails toward "do not hide", and the user's hide list
    // is the override. See the spec's "AUMID -> process matching is a heuristic" section.
    private bool ForegroundOwnsPlayingSmtc(string processName)
    {
        if (!_media.IsCurrentSessionPlaying) return false;
        if (string.IsNullOrWhiteSpace(processName)) return false;

        var aumid = _media.CurrentSourceAppUserModelId;
        if (string.IsNullOrWhiteSpace(aumid)) return false;

        return aumid.Contains(processName, StringComparison.OrdinalIgnoreCase)
            || processName.Contains(aumid, StringComparison.OrdinalIgnoreCase);
    }

    private static uint QueryNotificationState()
        => SHQueryUserNotificationState(out var state) == 0 ? state : 0;

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _pollTimer.Stop();
        if (_hook != 0)
        {
            try { _ = UnhookWinEvent(_hook); } catch { }
            _hook = 0;
        }
        _callback = null;
    }

    private delegate void WinEventDelegate(
        nint hWinEventHook, uint eventType, nint hwnd,
        int idObject, int idChild, uint idEventThread, uint dwmsEventTime);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    private struct MONITORINFO
    {
        public int cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
    }

    private const uint MONITOR_DEFAULTTONEAREST = 2;

    [DllImport("user32.dll")]
    private static extern nint MonitorFromWindow(nint hwnd, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetMonitorInfo(nint hMonitor, ref MONITORINFO lpmi);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint SetWinEventHook(
        uint eventMin, uint eventMax, nint hmodWinEventProc,
        WinEventDelegate lpfnWinEventProc, uint idProcess, uint idThread, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWinEvent(nint hWinEventHook);

    [DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetWindowRect(nint hWnd, out RECT lpRect);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern int GetClassName(nint hWnd, char[] lpClassName, int nMaxCount);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint GetWindowThreadProcessId(nint hWnd, out uint lpdwProcessId);

    [DllImport("shell32.dll")]
    private static extern int SHQueryUserNotificationState(out uint pquns);
}
```

- [ ] **Step 6: Wire it into App**

Task 6 already made `App` the owner of `MediaSessionClient`, so the watcher only needs inserting into the existing chain. `CardHost` takes its suppressor at construction, so the watcher must be built before it.

Add the field `private FullscreenVideoWatcher? _fullscreenWatcher;` and amend the Task 6 block in `OnStartup` — two changed lines and one added line, everything else stays:

```csharp
_mediaSession = new MediaSessionClient();

_audioCard = new AudioCard(_settings);
_mediaCard = new MediaCard(_settings);

_fullscreenWatcher = new FullscreenVideoWatcher(_settings, _mediaSession, Dispatcher, _diagnosticLog);   // added

_cardHost = new CardHost(_settings, _fullscreenWatcher);   // changed: suppressor argument
_cardHost.Register(_mediaCard);
_cardHost.Register(_audioCard);

_osd = new OsdHost(_settings, _theme, _cardHost);
_cardHost.ShowRequested += d => _osd.ShowOsd(d);
_cardHost.HideRequested += () => _osd.HideOsd();
_cardHost.Start();

_orchestrator = new OsdOrchestrator(_audioCard, _mediaCard, _settings, _osd.Dispatcher, _mediaSession, _diagnosticLog);
_orchestrator.Start();
_fullscreenWatcher.Start();   // added — after the orchestrator, so the first Evaluate sees a live session client
```

Add `DisposeStep("FullscreenVideoWatcher", () => _fullscreenWatcher?.Dispose());` to `OnExit`, **before** the `CardHost` step — the watcher must stop raising `SuppressionChanged` into a host that is about to be disposed.

- [ ] **Step 7: Add the Settings UI**

In `src/Plith/Views/SettingsWindow.xaml`, in the OSD behaviour section directly after the `CompactToggle` row, add a row matching the surrounding `RowStyle` pattern containing a `CheckBox x:Name="FullscreenVideoToggle"` labelled "Hide during fullscreen video" with the hint "Hides the OSD while a fullscreen window is playing video. Games are never affected.", and below it a `TextBox x:Name="FullscreenHideListBox"` labelled "Also hide in these apps" with the hint "Comma-separated process names, for players that don't report media to Windows."

In `src/Plith/Views/SettingsWindow.xaml.cs` there are three existing places to extend, each already handling `CompactToggle` — follow it exactly, do not invent a second pattern:

- **load** (near line 490, `CompactToggle.IsChecked = m.CompactMode;`) — add:
  ```csharp
  FullscreenVideoToggle.IsChecked = m.HideDuringFullscreenVideo;
  FullscreenHideListBox.Text = m.FullscreenVideoHideList;
  ```
- **auto-save subscriptions** (near line 538, the `CompactToggle.Checked/Unchecked += (_, _) => AutoSave();` pair) — add the matching pair for `FullscreenVideoToggle`, plus `FullscreenHideListBox.LostFocus += (_, _) => AutoSave();` (`LostFocus`, not `TextChanged` — saving on every keystroke would rewrite the INI once per character)
- **model build** (near line 584, `m.CompactMode = CompactToggle.IsChecked == true;`) — add:
  ```csharp
  m.HideDuringFullscreenVideo = FullscreenVideoToggle.IsChecked == true;
  m.FullscreenVideoHideList = FullscreenHideListBox.Text ?? string.Empty;
  ```

There is no `SyncPreview` entry for these two — neither affects the mini preview card.

- [ ] **Step 8: Build and run the full suite**

Run: `dotnet build Plith.slnx -c Debug`
Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: `Build succeeded.`, `3 Warning(s)`; tests PASS.

- [ ] **Step 9: Manual verification**

Run: `dotnet run --project src/Plith/Plith.csproj`

Confirm all four:
1. Fullscreen YouTube or Netflix in a browser, playing → volume keys change volume but **no OSD appears**.
2. Exit fullscreen → the OSD comes back.
3. A fullscreen game → the OSD **still appears**. Test a borderless-windowed title, not only an exclusive-fullscreen one; borderless is the case the rule is most at risk of getting wrong.
4. Toggle the setting off → the OSD appears during fullscreen video again.

Check `plith.log` for the `FullscreenVideo` transition lines to confirm the watcher is evaluating rather than silently erroring.

- [ ] **Step 10: Commit**

```bash
git add -A src/Plith tests/Plith.Tests
git commit -m "feat(osd): hide during fullscreen video playback"
```

---

# Stage C — Accessibility (Tasks 10–14)

---

### Task 10: Name the media transport buttons

The smallest real defect in the app: three buttons whose only content is a Segoe Fluent Icons private-use codepoint, which a screen reader reads aloud verbatim.

**Files:**
- Modify: `src/Plith/Views/MediaCardView.xaml`

**Interfaces:**
- Consumes: `MediaViewModel.PlayPauseLabel` (Task 3)

- [ ] **Step 1: Add automation names**

In `src/Plith/Views/MediaCardView.xaml`:

```xml
<Button x:Name="PrevButton"
        Style="{StaticResource MediaButtonStyle}"
        Content="&#xE892;"
        AutomationProperties.Name="Previous track"
        ToolTip="Previous" />
<Button x:Name="PlayPauseButton"
        Style="{StaticResource PlayPauseButtonStyle}"
        Margin="2,0"
        Content="{Binding PlayPauseGlyph}"
        AutomationProperties.Name="{Binding PlayPauseLabel}"
        ToolTip="Play / Pause" />
<Button x:Name="NextButton"
        Style="{StaticResource MediaButtonStyle}"
        Content="&#xE893;"
        AutomationProperties.Name="Next track"
        ToolTip="Next" />
```

Also give the album-art placeholder `TextBlock` (the `&#xE940;` glyph) `AutomationProperties.Name=""` with a comment noting it is decorative — the script in Task 14 treats an explicit empty name as intentional, and the real album-art information is carried by the card's own name in Task 11.

- [ ] **Step 2: Build**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 3: Verify with Narrator**

Start Narrator (`Ctrl+Win+Enter`), run Plith, summon the OSD with a media session active, and hover the transport buttons. Narrator must announce "Previous track", "Play" or "Pause", and "Next track" — not a glyph or "button".

- [ ] **Step 4: Commit**

```bash
git add src/Plith/Views/MediaCardView.xaml
git commit -m "fix(a11y): name media transport buttons"
```

---

### Task 11: OSD live regions

**Files:**
- Modify: `src/Plith/Views/AudioCardView.xaml`, `src/Plith/Views/MediaCardView.xaml`
- Modify: `src/Plith/ViewModels/AudioCardViewModel.cs`, `src/Plith/ViewModels/MediaViewModel.cs`

**Interfaces:**
- Produces: `AudioCardViewModel.AccessibleSummary` (`string`), `MediaViewModel.AccessibleSummary` (`string`)

- [ ] **Step 1: Add the summary properties**

In `AudioCardViewModel`, add:

```csharp
/// <summary>What a screen reader announces when the volume changes. The OSD never takes
/// focus, so this live-region text is the only audio feedback a non-sighted user gets.</summary>
public string AccessibleSummary => _muted ? $"{_label}, muted" : $"{_label}, {_gainText}";
```

and fire `OnPropertyChanged(nameof(AccessibleSummary))` from the `Label`, `GainText`, and `Muted` setters.

In `MediaViewModel`, add:

```csharp
/// <summary>Live-region text for the media card.</summary>
public string AccessibleSummary => _hasSession
    ? $"{_title} by {_artist}, {(_isPlaying ? "playing" : "paused")}"
    : string.Empty;
```

and fire `OnPropertyChanged(nameof(AccessibleSummary))` from the `Title`, `Artist`, `IsPlaying`, and `HasSession` setters.

- [ ] **Step 2: Mark the card roots as live regions**

On the root `Grid` of `AudioCardView.xaml` and the root `Grid` of `MediaCardView.xaml`:

```xml
AutomationProperties.Name="{Binding AccessibleSummary}"
AutomationProperties.LiveSetting="Polite"
```

- [ ] **Step 3: Build**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

- [ ] **Step 4: Verify with Narrator, and record the outcome honestly**

With Narrator running, change the volume. Narrator should announce the new value without being focused on the OSD.

**If it does not announce:** the `CreateWindowInBand` HWND may not participate in the UIA tree the way a normal top-level window does. This is a known risk flagged in the spec. Do **not** chase a workaround inside Phase 5. Leave the properties in place — they cost nothing and are correct — and add a short note to `docs/ROADMAP.md` under Phase 6 recording what was observed. Report the result as it happened; a live region that does not announce is not a passing accessibility item.

- [ ] **Step 5: Commit**

```bash
git add -A src/Plith docs/ROADMAP.md
git commit -m "feat(a11y): announce OSD changes as live regions"
```

---

### Task 12: SettingsWindow automation metadata

**Files:**
- Modify: `src/Plith/Views/SettingsWindow.xaml`, `src/Plith/Views/SettingsWindow.xaml.cs`

- [ ] **Step 1: Name every interactive control**

Add `AutomationProperties.Name` to each of these, or `AutomationProperties.LabeledBy="{Binding ElementName=...}"` where an adjacent `TextBlock` already carries the label text:

`MinimizeButton` ("Minimize"), `MaximizeButton` ("Maximize"), `CloseButton` ("Close"), `ThemeCombo`, `CustomHueSlider` ("Hue"), `CustomSatSlider` ("Saturation"), `CustomLumSlider` ("Lightness"), `CustomHexBox` ("Accent colour hex value"), `DurationSlider`, `OpenPositionOverlayButton`, `HoverToggle`, `OpacitySlider`, `ColorThresholdsToggle`, `CompactToggle`, `HotkeyCaptureButton`, `HotkeyClearButton`, `SourceCombo`, `EndpointCombo`, `BusCombo`, `AutoShowMediaToggle`, `AutoStartToggle`, `UpdateCheckButton`, `UpdateOpenPageButton`, `UpdateDownloadButton`, plus `FullscreenVideoToggle` and `FullscreenHideListBox` from Task 9.

Add `AutomationProperties.HelpText` on the three colour sliders and on `FullscreenHideListBox`, where the name alone does not convey the expected value.

- [ ] **Step 2: Fix the accent swatches**

`CreateSwatch` and `CreateCustomSwatch` in `SettingsWindow.xaml.cs` build each swatch as a `Border` with a `MouseLeftButtonUp` handler. A `Border` is not focusable and exposes no invoke pattern, so the swatches are today **entirely unreachable by keyboard and invisible to a screen reader** — a name alone does not fix that.

Change the swatch root from `Border` to `Button` carrying a `ControlTemplate` that renders the existing `Border` visual tree unchanged, replace the `MouseLeftButtonUp` subscription with `Click`, and set `AutomationProperties.Name` to the preset's display name (the string already passed as `tooltip`) — "Custom colour" for the custom swatch. This gets keyboard focus, `Space`/`Enter` activation, and UIA `Invoke` for free.

Keep `AccentSwatch.Root` typed as `FrameworkElement` so the record still holds either shape without further churn.

- [ ] **Step 3: Check tab order and focus visuals**

Tab through the whole Settings window from the title bar to the bottom. Every interactive control must be reachable, in visual order, with a visible focus indicator. `MediaButtonStyle` sets `FocusVisualStyle="{x:Null}"`; check whether any Settings button style does the same and restore a focus visual where it does.

- [ ] **Step 4: Build and verify with Narrator**

Run: `dotnet build Plith.slnx -c Debug`
Expected: `Build succeeded.`, `3 Warning(s)`.

With Narrator running, tab through Settings. Every control announces a meaningful name; no control announces only its type.

- [ ] **Step 5: Commit**

```bash
git add src/Plith/Views/SettingsWindow.xaml src/Plith/Views/SettingsWindow.xaml.cs
git commit -m "feat(a11y): add automation metadata to settings"
```

---

### Task 13: High-contrast palette

**Files:**
- Create: `src/Plith/Resources/Palette.HighContrast.xaml`, `src/Plith/Resources/OsdPalette.HighContrast.xaml`
- Modify: `src/Plith/Services/ThemeService.cs`

**Interfaces:**
- Produces: `ThemeService.IsHighContrast` (`bool`). `ThemeService.BuildAccentOverride()` returns an **empty** `ResourceDictionary` while high contrast is active.

- [ ] **Step 1: Audit and record what breaks**

Enable Windows high contrast (`Left Alt + Left Shift + Print Screen`), run the current build, and write down which OSD and Settings brushes become illegible. Put the list in the commit body — it is the evidence that the palette below addresses real breakage rather than guessed breakage.

- [ ] **Step 2: Create the high-contrast palettes**

Create both files with the **same brush keys** their dark/light counterparts define, each mapped to a `SystemColors` key rather than a literal, e.g.:

```xml
<SolidColorBrush x:Key="OsdSurfaceBrush" Color="{x:Static SystemColors.WindowColor}" />
<SolidColorBrush x:Key="OsdBorder" Color="{x:Static SystemColors.WindowTextColor}" />
<SolidColorBrush x:Key="OsdDivider" Color="{x:Static SystemColors.WindowTextColor}" />
<SolidColorBrush x:Key="OsdTextPrimary" Color="{x:Static SystemColors.WindowTextColor}" />
<SolidColorBrush x:Key="OsdAccent" Color="{x:Static SystemColors.HighlightColor}" />
```

`OsdPalette.HighContrast.xaml` must define **all 13** keys that `OsdPalette.Dark.xaml` defines, no more and no fewer:

`OsdSurfaceBrush`, `OsdBorder`, `OsdDivider`, `OsdHighlight`, `OsdTextPrimary`, `OsdTextSecondary`, `OsdTextTertiary`, `OsdTrackBg`, `OsdAccent`, `OsdGainMuted`, `OsdGainGreen`, `OsdGainAmber`, `OsdGainRed`.

Map the four `OsdGain*` keys to `SystemColors.HighlightColor` except `OsdGainMuted`, which maps to `SystemColors.GrayTextColor`. High contrast has no palette for a green/amber/red loudness scale, and inventing one defeats the mode — the bar stays a single system colour and the numeric readout carries the value.

`Palette.HighContrast.xaml` must likewise define all **23** keys present in `Palette.Dark.xaml`. Verify the counts before moving on:

```bash
grep -c 'x:Key=' src/Plith/Resources/OsdPalette.HighContrast.xaml   # expect 13
grep -c 'x:Key=' src/Plith/Resources/Palette.HighContrast.xaml      # expect 23
```

A missing key does not error — the `DynamicResource` lookup silently falls through and the element renders with nothing.

Give `OsdBorder` and `OsdDivider` full opacity. Their dark-theme values are alpha-blended, which is exactly what makes them vanish in high contrast.

- [ ] **Step 3: Teach ThemeService about high contrast**

In `ThemeService`:

- Add the two new URIs alongside the existing four.
- Add `public bool IsHighContrast => SystemParameters.HighContrast;`
- Replace the `_isEffectiveDark` bool with a `PaletteKind { Dark, Light, HighContrast }` field so the swap short-circuit can tell a high-contrast transition from a polarity one. Keep `IsEffectiveDark` as a public property (`SettingsWindow` uses it for its DWM tint), returning `true` for `Dark`, `false` for `Light`, and for `HighContrast` deriving it from the luminance of `SystemColors.WindowColor`.
- In `Apply`, choose `PaletteKind.HighContrast` whenever `SystemParameters.HighContrast` is true, regardless of the user's theme setting.
- **Skip the accent override entirely in high contrast.** Make `BuildAccentOverride()` return `new ResourceDictionary()` when `IsHighContrast`, and have `ApplyAccentOverride` remove any previously applied override. This is the step that actually matters: an accent tint layered over system colours defeats the whole point of high contrast, and returning an empty dictionary from `BuildAccentOverride` means `OsdHost.RefreshAccentMirror` needs no change at all.
- Subscribe to `SystemParameters.StaticPropertyChanged` in `Start()`, re-applying when `e.PropertyName == nameof(SystemParameters.HighContrast)`; unsubscribe in `Dispose()`.

- [ ] **Step 4: Build and test**

Run: `dotnet build Plith.slnx -c Debug`
Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj`
Expected: `Build succeeded.`, `3 Warning(s)`; tests PASS — `AccentThemeTests` must still pass untouched.

- [ ] **Step 5: Verify in high contrast**

Run the app, toggle high contrast on with the OSD visible. The OSD must switch to system colours live, with the border and divider clearly visible. Toggle back off; the accent theme must return.

- [ ] **Step 6: Commit**

```bash
git add src/Plith/Resources src/Plith/Services/ThemeService.cs
git commit -m "feat(a11y): add high-contrast palette"
```

Put the Step 1 audit list in the commit body.

---

### Task 14: Accessibility lint script

**Files:**
- Create: `scripts/check-a11y.ps1`

- [ ] **Step 1: Write the script**

Create `scripts/check-a11y.ps1`:

```powershell
#requires -Version 7
<#
.SYNOPSIS
  Fails when an interactive XAML control carries no accessible name.

.DESCRIPTION
  AutomationProperties live in XAML, where unit tests are weak — asserting on them needs an
  STA thread and a loaded visual tree. This static check is the regression guard instead.

  A control passes when it declares AutomationProperties.Name (including an explicitly empty
  one, which marks a decorative element) or AutomationProperties.LabeledBy.
#>
[CmdletBinding()]
param(
    [string] $Root = (Join-Path $PSScriptRoot '..' 'src')
)

$ErrorActionPreference = 'Stop'

$interactive = @('Button', 'ComboBox', 'Slider', 'ToggleButton', 'CheckBox', 'TextBox', 'RadioButton')
$failures = [System.Collections.Generic.List[string]]::new()

foreach ($file in Get-ChildItem -Path $Root -Filter '*.xaml' -Recurse) {
    $lines = Get-Content -LiteralPath $file.FullName
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $match = [regex]::Match($lines[$i], '<(' + ($interactive -join '|') + ')[\s>]')
        if (-not $match.Success) { continue }

        # Element attributes can wrap across lines; scan forward to the tag's closing bracket.
        $element = ''
        for ($j = $i; $j -lt $lines.Count; $j++) {
            $element += $lines[$j]
            if ($lines[$j] -match '/?>\s*$') { break }
        }

        if ($element -notmatch 'AutomationProperties\.(Name|LabeledBy)') {
            $rel = Resolve-Path -Relative -LiteralPath $file.FullName
            $failures.Add("$rel($($i + 1)): <$($match.Groups[1].Value)> has no AutomationProperties.Name or LabeledBy")
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host "Accessibility check failed:`n" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    Write-Host "`nAdd AutomationProperties.Name, or AutomationProperties.Name=`"`" for a purely decorative element." -ForegroundColor Yellow
    exit 1
}

Write-Host "Accessibility check passed: every interactive control has an accessible name." -ForegroundColor Green
exit 0
```

- [ ] **Step 2: Run it**

Run: `pwsh -File scripts/check-a11y.ps1`
Expected: exit 0. If it reports failures, they are real gaps missed in Tasks 10–12 — fix the XAML, not the script. Style-level controls inside `ControlTemplate` definitions that report as false positives should be given an explicit `AutomationProperties.Name=""` only when genuinely decorative; otherwise name them.

- [ ] **Step 3: Verify it actually catches a regression**

Temporarily delete one `AutomationProperties.Name` from `SettingsWindow.xaml`, re-run the script, confirm it exits 1 and names that line, then restore the attribute. A guard that has never failed is not a guard.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-a11y.ps1
git commit -m "chore(a11y): add XAML accessible-name lint"
```

---

## Phase 5 completion gate

Run `superpowers:verification-before-completion` before claiming Phase 5 done. Every claim needs command output behind it:

- [ ] `dotnet build Plith.slnx -c Debug` → `Build succeeded.`, exactly `3 Warning(s)` (the pre-existing CA1861 trio)
- [ ] `dotnet test tests/Plith.Tests/Plith.Tests.csproj` → all pass
- [ ] `dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj` → all pass (untouched by Phase 5; confirm it stayed that way)
- [ ] `pwsh -File scripts/check-a11y.ps1` → exit 0
- [ ] OSD screenshots match the Task 4 baselines in both the audio-only and audio+media states
- [ ] Narrator announces the transport buttons, every Settings control, and — if the band window permits it — OSD volume changes. Record the OSD live-region result honestly either way.
- [ ] High contrast renders a legible OSD and Settings window
- [ ] The OSD still draws over an exclusive-fullscreen game **and** over a borderless-windowed game
- [ ] The OSD hides during fullscreen Netflix or VLC playback and returns on exit

Then run `superpowers:requesting-code-review` before the branch merges.
