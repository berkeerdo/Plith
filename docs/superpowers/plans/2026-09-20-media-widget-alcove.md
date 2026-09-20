# Notch Media Page: Alcove Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the notch's media page as Alcove lays one out, inside the frame Plith already
has, and open the notch on that page while something is playing.

**Architecture:** The blurred-artwork ground and the page's private ink go away, so the page sits
on the themed notch surface like every other page. The tile grows to 56 DIP, a full-width
progress row appears below it, and the transport rail gains an output-device control. SMTC
timeline data reaches the view model on its own event, separate from the snapshot path, because
the snapshot path re-downloads album art. The bar's movement is interpolated by a pure function
from a stamped position, ticked once a second while the page is visible. Which page the notch
opens on becomes a pure policy computed at open time.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`), xunit, `Windows.Media.Control`
(SMTC) via the Windows SDK projections, PowerShell 7 harnesses in `scripts/`.

**Spec:** `docs/superpowers/specs/2026-09-20-media-widget-alcove-design.md`

## Global Constraints

- **The frame does not change.** `NotchGeometry.OpenFrameDip` is `356x116` and no task may touch
  it. Vertical budget inside it: 14 DIP top padding, 73 DIP content band, 29 DIP below (20 of
  which is the page rail's lane).
- **All code, comments, and commit messages in English.** Conventional Commits. No AI attribution
  of any kind in commits, code, or docs.
- **No em dashes and no hyphen used as a sentence separator**, in code comments or docs. Use
  commas, colons, parentheses, or a second sentence.
- **Icons are drawn geometry from `Resources/PlithIcons.xaml`.** `scripts/check-a11y.ps1` fails
  the build on a Segoe MDL2 code point, in code-behind as well as XAML. No new icon is drawn in
  this plan: the output control reuses `IconSpeakerBody`, `IconSpeakerWaveInner`,
  `IconSpeakerWaveOuter`.
- **`AutomationProperties` only on elements WPF gives an automation peer.** Panels, `Border`s and
  `Grid`s have none. `check-a11y.ps1` check 2 enforces this and reads code-behind too.
- **The test project is not STA.** Nothing that constructs a `UserControl` can be unit-tested at
  all. Logic that needs a test goes in a pure class in `Plith.Services` or
  `Plith.Views.Presentation`, which is why `NotchEventPolicy` exists as its own file.
- **Baseline to keep green:** `dotnet build` with 0 warnings, `dotnet test` at **504 passing**
  (487 in `Plith.Tests`, 17 in `Plith.Installer.Tests`, measured 2026-09-20).
- **`MediaTimeline` only ever exists with a positive duration.** `ReadTimeline` returns null
  otherwise, so no consumer needs to guard against divide-by-zero. Both ends state this.

---

### Task 1: `MediaProgress`, the pure part

Where the bar should be, and how a track clock is written. No WPF, no SMTC, so this is the only
part of the feature a unit test can reach.

**Files:**
- Create: `src/Plith/Services/MediaProgress.cs`
- Test: `tests/Plith.Tests/MediaProgressTests.cs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `static TimeSpan MediaProgress.Elapsed(TimeSpan position, DateTimeOffset lastUpdated, TimeSpan duration, bool isPlaying, DateTimeOffset now)`
  - `static string MediaProgress.Clock(TimeSpan span)`

- [x] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/MediaProgressTests.cs`:

```csharp
using Plith.Services;

namespace Plith.Tests;

public class MediaProgressTests
{
    private static readonly DateTimeOffset Now = new(2026, 9, 20, 18, 0, 0, TimeSpan.Zero);

    private static TimeSpan Elapsed(double positionSeconds, double stampAgeSeconds,
                                    double durationSeconds, bool isPlaying = true)
        => MediaProgress.Elapsed(
            TimeSpan.FromSeconds(positionSeconds),
            Now - TimeSpan.FromSeconds(stampAgeSeconds),
            TimeSpan.FromSeconds(durationSeconds),
            isPlaying,
            Now);

    [Fact]
    public void Playing_AdvancesTheReportedPositionByTheAgeOfTheReading()
    {
        Assert.Equal(TimeSpan.FromSeconds(65), Elapsed(60, 5, 200));
    }

    [Fact]
    public void Paused_DoesNotDrift()
    {
        // The whole point of interpolating from a stamp: a paused track whose reading is a
        // minute old is still exactly where it was left.
        Assert.Equal(TimeSpan.FromSeconds(60), Elapsed(60, 60, 200, isPlaying: false));
    }

    [Fact]
    public void PastTheEnd_ClampsToTheDuration()
    {
        Assert.Equal(TimeSpan.FromSeconds(200), Elapsed(195, 30, 200));
    }

    [Fact]
    public void AStampInTheFuture_ReturnsTheReportedPosition()
    {
        // Clock skew, and a source that stamps with its own clock. Subtracting a negative age
        // would run the bar backwards.
        Assert.Equal(TimeSpan.FromSeconds(60), Elapsed(60, -10, 200));
    }

    [Fact]
    public void NoDuration_IsZero()
    {
        // A live stream reports no end time. The caller draws nothing in this case; this is
        // here so the function is still total.
        Assert.Equal(TimeSpan.Zero, Elapsed(60, 5, 0));
    }

    [Fact]
    public void ANegativeReportedPosition_ClampsToZero()
    {
        Assert.Equal(TimeSpan.Zero, Elapsed(-30, 0, 200));
    }

    [Fact]
    public void Clock_WritesMinutesAndPaddedSeconds()
    {
        Assert.Equal("2:27", MediaProgress.Clock(TimeSpan.FromSeconds(147)));
        Assert.Equal("0:00", MediaProgress.Clock(TimeSpan.Zero));
        Assert.Equal("0:07", MediaProgress.Clock(TimeSpan.FromSeconds(7)));
    }

    [Fact]
    public void Clock_WritesHoursOnlyWhenThereAreSome()
    {
        Assert.Equal("1:02:03", MediaProgress.Clock(TimeSpan.FromSeconds(3723)));
        Assert.Equal("59:59", MediaProgress.Clock(TimeSpan.FromSeconds(3599)));
    }

    [Fact]
    public void Clock_OfANegativeSpanIsZero()
    {
        // Remaining time is computed as duration minus elapsed, and a source that reports a
        // position past its own duration would otherwise render "-0:-5".
        Assert.Equal("0:00", MediaProgress.Clock(TimeSpan.FromSeconds(-5)));
    }
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests --filter MediaProgressTests`
Expected: FAIL to compile, `The name 'MediaProgress' does not exist`.

- [x] **Step 3: Write the implementation**

Create `src/Plith/Services/MediaProgress.cs`:

```csharp
namespace Plith.Services;

/// <summary>
/// Where a track has got to, and how to write that as a clock.
///
/// Pure, and that is the reason it is a class of its own rather than two private methods on the
/// widget. The suite is not STA, so a UserControl cannot be constructed in a test at all: pure
/// code is the only part of the media page a test can reach. See NotchEventPolicy for the same
/// move made for the same reason.
///
/// SMTC reports a position with a timestamp rather than a live value. Asking for it every second
/// would be a polling loop against a cross-process source; interpolating from the stamp is both
/// cheaper and what makes a source that only reports on seek still look right.
/// </summary>
public static class MediaProgress
{
    /// <summary>
    /// The position now, given a reading taken at <paramref name="lastUpdated"/>.
    ///
    /// Total on purpose: every degenerate input a real source has been seen to produce returns
    /// something drawable rather than throwing. A duration of zero returns zero because the
    /// caller draws no bar in that case, and a stamp in the future returns the reading itself
    /// rather than running the bar backwards.
    /// </summary>
    public static TimeSpan Elapsed(TimeSpan position, DateTimeOffset lastUpdated, TimeSpan duration,
                                   bool isPlaying, DateTimeOffset now)
    {
        if (duration <= TimeSpan.Zero) return TimeSpan.Zero;

        var elapsed = position;
        if (isPlaying)
        {
            var age = now - lastUpdated;
            if (age > TimeSpan.Zero) elapsed += age;
        }

        if (elapsed < TimeSpan.Zero) return TimeSpan.Zero;
        return elapsed > duration ? duration : elapsed;
    }

    /// <summary>
    /// A track clock: minutes and padded seconds, with hours only when there are any.
    ///
    /// Not a format string on the call site, because the remaining time is written as the
    /// negative of this and a span that has gone slightly negative would print a second minus
    /// sign. Clamped here instead, once.
    /// </summary>
    public static string Clock(TimeSpan span)
    {
        if (span < TimeSpan.Zero) span = TimeSpan.Zero;

        return span.TotalHours >= 1
            ? $"{(int)span.TotalHours}:{span.Minutes:00}:{span.Seconds:00}"
            : $"{(int)span.TotalMinutes}:{span.Seconds:00}";
    }
}
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `dotnet test tests/Plith.Tests --filter MediaProgressTests`
Expected: PASS, 9 tests.

- [x] **Step 5: Run the whole suite**

Run: `dotnet test`
Expected: 513 passing (504 baseline plus 9), 0 failed.

- [x] **Step 6: Commit**

```bash
git add src/Plith/Services/MediaProgress.cs tests/Plith.Tests/MediaProgressTests.cs
git commit -m "feat(media): interpolate the track position from a stamped reading

SMTC reports a position with a timestamp, not a live value. Elapsed()
advances a reading by its own age while playing and freezes it while
paused, and clamps every degenerate case a real source produces:
no duration, a position past the end, a stamp in the future."
```

---

### Task 2: Timeline through the pipeline

The position reaches the view model, on its own event. Nothing on screen changes yet.

**Files:**
- Modify: `src/Plith/Services/MediaSessionClient.cs` (the `MediaSnapshot` record at the top, `AttachCurrent`, `DetachCurrent`, `EmitSnapshotAsync`)
- Modify: `src/Plith/Services/OsdOrchestrator.cs` (constructor subscription near `_media.Changed += OnMediaChanged;`, the "Media session push" region, `Dispose`)
- Modify: `src/Plith/Cards/MediaCard.cs`
- Modify: `src/Plith/ViewModels/MediaViewModel.cs`
- Test: `tests/Plith.Tests/MediaViewModelTests.cs`, `tests/Plith.Tests/MediaCardTests.cs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `sealed record MediaTimeline(TimeSpan Position, TimeSpan Duration, DateTimeOffset LastUpdated)` in `Plith.Services`
  - `MediaSnapshot` gains a sixth positional member, `MediaTimeline? Timeline = null`
  - `event Action<MediaTimeline?>? MediaSessionClient.TimelineChanged`
  - `void MediaCard.ApplyTimeline(MediaTimeline? timeline)`
  - `MediaTimeline? MediaViewModel.Timeline { get; set; }`

- [x] **Step 1: Write the failing tests**

Append to `tests/Plith.Tests/MediaViewModelTests.cs`:

```csharp
    [Fact]
    public void Timeline_RaisesPropertyChangedOnceForItsOwnName()
    {
        var vm = new MediaViewModel();
        var seen = new List<string?>();
        vm.PropertyChanged += (_, e) => seen.Add(e.PropertyName);

        vm.Timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                        DateTimeOffset.UnixEpoch);

        // Exactly one, and named: the media page routes on the name so that a position arriving
        // once a second does not repaint the artwork and re-measure the marquee.
        Assert.Single(seen);
        Assert.Equal("Timeline", seen[0]);
    }

    [Fact]
    public void Timeline_SetToAnEqualValue_RaisesNothing()
    {
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                         DateTimeOffset.UnixEpoch);
        var vm = new MediaViewModel { Timeline = timeline };
        var raised = 0;
        vm.PropertyChanged += (_, _) => raised++;

        vm.Timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                        DateTimeOffset.UnixEpoch);

        Assert.Equal(0, raised);
    }

    [Fact]
    public void Apply_CarriesTheSnapshotsTimeline()
    {
        var vm = new MediaViewModel();
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(30), TimeSpan.FromSeconds(180),
                                         DateTimeOffset.UnixEpoch);

        vm.Apply(new MediaSnapshot("t", "a", null, IsPlaying: true, HasSession: true, timeline));

        Assert.Equal(timeline, vm.Timeline);
    }

    [Fact]
    public void Apply_WithNoSession_ClearsTheTimeline()
    {
        var vm = new MediaViewModel
        {
            Timeline = new MediaTimeline(TimeSpan.FromSeconds(30), TimeSpan.FromSeconds(180),
                                         DateTimeOffset.UnixEpoch),
        };

        vm.Apply(new MediaSnapshot("", "", null, IsPlaying: false, HasSession: false));

        Assert.Null(vm.Timeline);
    }
```

`MediaViewModelTests.cs` already has `using Plith.Cards;` and `using Plith.ViewModels;`. Add
`using Plith.Services;`.

Append to `tests/Plith.Tests/MediaCardTests.cs`:

```csharp
    [Fact]
    public void ApplyTimeline_ReachesTheViewModel()
    {
        var card = new MediaCard(NewSettings());
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(12), TimeSpan.FromSeconds(200),
                                         DateTimeOffset.UnixEpoch);

        card.ApplyTimeline(timeline);

        Assert.Equal(timeline, card.Vm.Timeline);
    }

    [Fact]
    public void ApplyTimeline_RaisesNoShowRequest()
    {
        // This test is the reason ApplyTimeline exists at all. Apply() raises ShowRequested when
        // AutoShowOnMedia is on, and TimelinePropertiesChanged fires about once a second on some
        // sources: routing the position through Apply would summon the OSD every second.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        card.Apply(Playing());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.ApplyTimeline(new MediaTimeline(TimeSpan.FromSeconds(12), TimeSpan.FromSeconds(200),
                                             DateTimeOffset.UnixEpoch));

        Assert.Empty(shows);
    }
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests --filter "MediaViewModelTests|MediaCardTests"`
Expected: FAIL to compile, `MediaTimeline` and `ApplyTimeline` do not exist.

- [x] **Step 3: Add the record and the snapshot member**

In `src/Plith/Services/MediaSessionClient.cs`, replace the record declarations at the top:

```csharp
/// <summary>
/// Where the current track is, and when that was last true.
///
/// A stamped reading rather than a live position, because that is what SMTC reports. Only ever
/// constructed with a positive <paramref name="Duration"/>: see ReadTimeline, which returns null
/// otherwise, so no consumer has to guard a division.
/// </summary>
public sealed record MediaTimeline(TimeSpan Position, TimeSpan Duration, DateTimeOffset LastUpdated);

public sealed record MediaSnapshot(
    string Title,
    string Artist,
    byte[]? ThumbnailBytes,
    bool IsPlaying,
    bool HasSession,
    // Defaulted so the five-argument construction in the tests and in the no-session path keeps
    // compiling and keeps meaning "no timeline".
    MediaTimeline? Timeline = null);
```

- [x] **Step 4: Read and publish the timeline in the client**

In the same file, add the event beside `Changed`:

```csharp
    /// <summary>
    /// Raised when the position moves, carrying only the timeline.
    ///
    /// Separate from <see cref="Changed"/> on purpose. Changed comes from ScheduleEmit, which
    /// re-reads the media properties AND re-downloads the album thumbnail, and
    /// TimelinePropertiesChanged fires about once a second on some sources. Subscribing the
    /// position to that path would download the artwork once per second.
    /// </summary>
    public event Action<MediaTimeline?>? TimelineChanged;
```

Add the subscription in `AttachCurrent` and the matching line in `DetachCurrent`:

```csharp
        _currentSession.TimelinePropertiesChanged += OnTimelineChanged;
```
```csharp
        _currentSession.TimelinePropertiesChanged -= OnTimelineChanged;
```

Add the handler and the read, next to `OnSessionChanged`:

```csharp
    private void OnTimelineChanged(GlobalSystemMediaTransportControlsSession sender,
                                   TimelinePropertiesChangedEventArgs args)
        => TimelineChanged?.Invoke(ReadTimeline(sender));

    /// <summary>
    /// The session's timeline, or null when there is nothing usable to draw.
    ///
    /// Null rather than a zero-length timeline for a live stream or a source that reports no end
    /// time: a bar of unknown length is a lie, and the page draws no bar for null.
    ///
    /// StartTime is subtracted rather than assumed to be zero, because it is not always zero for
    /// chaptered content. A missing LastUpdatedTime becomes now: left at default it is year 1,
    /// and the interpolation would then pin every bar to the end of its track.
    /// </summary>
    private static MediaTimeline? ReadTimeline(GlobalSystemMediaTransportControlsSession session)
    {
        try
        {
            var t = session.GetTimelineProperties();
            if (t is null) return null;

            var duration = t.EndTime - t.StartTime;
            if (duration <= TimeSpan.Zero) return null;

            var position = t.Position - t.StartTime;
            var stamp = t.LastUpdatedTime == default ? DateTimeOffset.Now : t.LastUpdatedTime;
            return new MediaTimeline(position, duration, stamp);
        }
        catch
        {
            // Same contract as every other read in this class: a session that died mid-read
            // costs the caller a null, not an exception on a threadpool thread.
            return null;
        }
    }
```

In `EmitSnapshotAsync`, carry the timeline on the full snapshot too, so a subscriber that only
listens to `Changed` is never left without one:

```csharp
        Changed?.Invoke(new MediaSnapshot(title, artist, thumb, playing, HasSession: true,
                                         ReadTimeline(session)));
```

- [x] **Step 5: Carry it through the card and the view model**

In `src/Plith/ViewModels/MediaViewModel.cs`, add the property beside `IsPlaying`:

```csharp
    private MediaTimeline? _timeline;

    /// <summary>
    /// Where the track is, or null when the source reports no usable duration.
    ///
    /// Its own property rather than three, so a position arriving once a second raises one
    /// notification. The media page routes on the name of this one and repaints only its
    /// progress row.
    /// </summary>
    public MediaTimeline? Timeline
    {
        get => _timeline;
        set => Set(ref _timeline, value);
    }
```

`MediaViewModel.cs` already has `using Plith.Services;`.

In `Apply`, add the line after `AlbumArt`:

```csharp
        Timeline = snapshot.Timeline;
```

In `src/Plith/Cards/MediaCard.cs`, add after `Apply`:

```csharp
    /// <summary>
    /// A new position for the track already showing.
    ///
    /// Deliberately not routed through <see cref="Apply"/>: Apply raises ShowRequested when
    /// AutoShowOnMedia is on, and this arrives about once a second, so the OSD would be summoned
    /// every second by a bar moving. It also cannot change IsVisible, so there is nothing to
    /// reconcile.
    /// </summary>
    public void ApplyTimeline(MediaTimeline? timeline) => Vm.Timeline = timeline;
```

- [x] **Step 6: Marshal it in the orchestrator**

In `src/Plith/Services/OsdOrchestrator.cs`, beside `_media.Changed += OnMediaChanged;` add:

```csharp
        _media.TimelineChanged += OnTimelineChanged;
```

In the "Media session push" region, after `OnMediaChanged`:

```csharp
    private void OnTimelineChanged(MediaTimeline? timeline)
    {
        // Same marshalling as OnMediaChanged: SMTC raises on threadpool threads and the view
        // model is read by the UI.
        if (!_dispatcher.CheckAccess())
        {
            _dispatcher.BeginInvoke(() => { if (!_disposed) OnTimelineChanged(timeline); });
            return;
        }
        if (_disposed) return;

        _mediaCard.ApplyTimeline(timeline);
    }
```

In `Dispose`, beside `_media.Changed -= OnMediaChanged;`:

```csharp
        _media.TimelineChanged -= OnTimelineChanged;
```

- [x] **Step 7: Run the tests to verify they pass**

Run: `dotnet test`
Expected: 519 passing (513 plus 6), 0 failed, and `dotnet build` reporting 0 warnings.

- [x] **Step 8: Commit**

```bash
git add src/Plith/Services/MediaSessionClient.cs src/Plith/Services/OsdOrchestrator.cs \
        src/Plith/Cards/MediaCard.cs src/Plith/ViewModels/MediaViewModel.cs \
        tests/Plith.Tests/MediaViewModelTests.cs tests/Plith.Tests/MediaCardTests.cs
git commit -m "feat(media): plumb the SMTC timeline on its own event

TimelinePropertiesChanged fires about once a second on some sources, and
the snapshot path re-downloads the album thumbnail, so the position gets
its own event and its own card entry point. ApplyTimeline raises no show
request: routed through Apply it would summon the OSD every second.

A timeline exists only with a positive duration, so a live stream reports
null and the page draws no bar rather than a bar of unknown length."
```

---

### Task 3: The page's shape and ground

The list row becomes the one thing playing: no artwork ground, a 56 DIP tile, and a rail of four
controls. The progress row's space is reserved here and filled in Task 4.

**Files:**
- Modify: `src/Plith/Views/Widgets/MediaWidget.xaml` (rewritten)
- Modify: `src/Plith/Views/Widgets/MediaWidget.cs`
- Modify: `src/Plith/ViewModels/MediaViewModel.cs:129` (`DecodePixelWidth`)
- Create: `src/Plith/Services/SystemSoundPanel.cs`
- Modify: `scripts/render-widgets.ps1` (two more states)

**Interfaces:**
- Consumes: nothing from Tasks 1 and 2.
- Produces: `static bool SystemSoundPanel.TryOpen()`; the named elements Task 4 fills in:
  `ProgressRow`, `Elapsed`, `Remaining`, `Bar`.

No unit test exists for this task and cannot: the suite is not STA. The test cycle is the render
harness plus two lints, which is what has actually found the layout defects on this branch.

- [ ] **Step 1: Write the sound panel launcher**

Create `src/Plith/Services/SystemSoundPanel.cs`:

```csharp
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
/// in. Returns false rather than throwing, like TryOpenSourceApp: a press on a notch control is
/// not worth taking the OSD down for.
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
```

- [ ] **Step 2: Rewrite the page's XAML**

Replace the whole of `src/Plith/Views/Widgets/MediaWidget.xaml`:

```xml
<UserControl x:Class="Plith.Views.Widgets.MediaWidget"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008"
             xmlns:w="clr-namespace:Plith.Views.Widgets"
             mc:Ignorable="d"
             AutomationProperties.Name="Now playing"
             Focusable="False">
    <UserControl.Resources>
        <ResourceDictionary>
            <ResourceDictionary.MergedDictionaries>
                <ResourceDictionary Source="pack://application:,,,/Plith;component/Resources/PlithIcons.xaml" />
            </ResourceDictionary.MergedDictionaries>

            <!-- No NotchInk override here any more, and that is the point of this pass.

                 The page used to blow the album art up behind a near-black scrim, which made its
                 ground dark in BOTH themes while every other page followed the theme. So it had
                 to declare its own ink, it was an exception to the light theme, and the pair was
                 invisible to check-contrast.ps1, which only measures pairs a file states in one
                 place. The ground is gone, so the exception is gone with it: the ink is inherited
                 like everywhere else. -->

            <Style x:Key="TransportButtonStyle" TargetType="Button">
                <Setter Property="Width" Value="28" />
                <Setter Property="Height" Value="28" />
                <Setter Property="Cursor" Value="Hand" />
                <Setter Property="Background" Value="#0DFFFFFF" />
                <!-- Dimmer than the title beside them: the controls are available, not the thing
                     you came to read. -->
                <Setter Property="Foreground" Value="{DynamicResource NotchInkMuted}" />
                <Setter Property="Template">
                    <Setter.Value>
                        <ControlTemplate TargetType="Button">
                            <!-- TemplateBinding, not a literal: play/pause overrides this to sit
                                 brighter than the others, and a hard-coded value here would make
                                 that override do nothing. -->
                            <Border x:Name="Chrome" CornerRadius="14"
                                    Background="{TemplateBinding Background}">
                                <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center" />
                            </Border>
                            <ControlTemplate.Triggers>
                                <Trigger Property="IsMouseOver" Value="True">
                                    <Setter TargetName="Chrome" Property="Background" Value="#1FFFFFFF" />
                                </Trigger>
                                <Trigger Property="IsKeyboardFocused" Value="True">
                                    <Setter TargetName="Chrome" Property="Background"
                                            Value="{DynamicResource OsdHighlight}" />
                                </Trigger>
                            </ControlTemplate.Triggers>
                        </ControlTemplate>
                    </Setter.Value>
                </Setter>
            </Style>

            <!-- A real ProgressBar, not two Borders, and not for looks: ProgressBar has an
                 automation peer with a value, so where the track has got to reaches a screen
                 reader. NotchHud's level bar is two Borders and stays that way, because it is on
                 screen for two seconds.

                 PART_Track and PART_Indicator are the names ProgressBar's own code looks up when
                 it sizes the fill. Renaming either leaves a bar that never moves. -->
            <Style x:Key="TrackBarStyle" TargetType="ProgressBar">
                <Setter Property="Height" Value="4" />
                <Setter Property="Minimum" Value="0" />
                <Setter Property="Maximum" Value="100" />
                <Setter Property="Background" Value="{DynamicResource NotchTrack}" />
                <Setter Property="Foreground" Value="{DynamicResource OsdAccent}" />
                <Setter Property="Focusable" Value="False" />
                <Setter Property="Template">
                    <Setter.Value>
                        <ControlTemplate TargetType="ProgressBar">
                            <Border CornerRadius="2" ClipToBounds="True"
                                    Background="{TemplateBinding Background}">
                                <Grid x:Name="PART_Track">
                                    <Rectangle x:Name="PART_Indicator" HorizontalAlignment="Left"
                                               Fill="{TemplateBinding Foreground}" />
                                </Grid>
                            </Border>
                        </ControlTemplate>
                    </Setter.Value>
                </Setter>
            </Style>
        </ResourceDictionary>
    </UserControl.Resources>

    <!-- Two rows in a 73 DIP band: the tile row, then the progress row.
         14 above and 29 below (20 of which is the page rail's lane) are the frame's, not ours. -->
    <Grid x:Name="Root" Margin="18,14,18,29">
        <Grid.RowDefinitions>
            <RowDefinition Height="56" />
            <RowDefinition Height="3" />
            <RowDefinition Height="14" />
        </Grid.RowDefinitions>

        <!-- 56 (art) + 12 + 116 (text) + 136 (rail) = 320, the content width. -->
        <Grid Grid.Row="0">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="56" />
                <ColumnDefinition Width="*" />
                <ColumnDefinition Width="Auto" />
            </Grid.ColumnDefinitions>

            <!-- The art and the text are one target; the transport is not part of it. A
                 page-wide click would swallow every press meant for a control on the same row. -->
            <Border x:Name="OpenSourceArea" Grid.Column="0" Grid.ColumnSpan="2"
                    Background="Transparent" />

            <!-- The hairline stays. Album art is photographic and often dark at the edges, so
                 without an inner edge it dissolves into the panel and reads as a smudge rather
                 than a picture with a boundary. -->
            <Border x:Name="ArtHost" Grid.Column="0" Width="56" Height="56" CornerRadius="10"
                    Background="{DynamicResource NotchTrack}">
                <Border.Clip>
                    <RectangleGeometry Rect="0,0,56,56" RadiusX="10" RadiusY="10" />
                </Border.Clip>
                <Grid>
                    <Image x:Name="Art" Stretch="UniformToFill" />
                    <Border CornerRadius="10" BorderThickness="1" BorderBrush="#1FFFFFFF" />
                </Grid>
            </Border>

            <!-- MinWidth 0 is load-bearing: a star column sizes to its content's minimum by
                 default, so a long title would push the rail off the frame instead of being
                 clipped. The gap is on the left only, so the 116 in the comment above is the
                 width the text actually gets. -->
            <StackPanel x:Name="TextColumn" Grid.Column="1" MinWidth="0" Margin="12,0,0,0"
                        VerticalAlignment="Center">
                <!-- TextElement.* would be needed for anything MarqueeText does not own; both of
                     these are inheritable, so setting them here reaches the TextBlock inside. -->
                <w:MarqueeText x:Name="Title"
                               Height="20"
                               FontSize="14.5" FontWeight="SemiBold"
                               Foreground="{DynamicResource NotchInk}" />
                <w:MarqueeText x:Name="Artist"
                               Height="17"
                               Margin="0,2,0,0"
                               FontSize="11.5"
                               Foreground="{DynamicResource NotchInkMuted}" />
            </StackPanel>

            <!-- Round, not rounded-square. Three circles read as a transport; three squares read
                 as a toolbar, and this is the row a person reaches for without looking.
                 The 12 DIP gap before the output control is what stops it reading as a fourth
                 transport button. -->
            <StackPanel Grid.Column="2" Orientation="Horizontal" VerticalAlignment="Center">
                <Button x:Name="Previous" Style="{StaticResource TransportButtonStyle}"
                        AutomationProperties.Name="Previous track">
                    <Viewbox Width="14" Height="14"><Canvas Width="24" Height="24">
                            <Path Data="{StaticResource IconPrevious}"
                                  Fill="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}" />
                        </Canvas></Viewbox>
                </Button>
                <!-- The name is rewritten by Render() on every state change, because "Play" on a
                     button that pauses is worse than no name at all. The value here is what it
                     carries before any session exists, and it is declared rather than left to
                     code so the accessibility lint can see it: a name that only ever exists at
                     runtime is a name nothing can check. -->
                <Button x:Name="PlayPause" Style="{StaticResource TransportButtonStyle}"
                        AutomationProperties.Name="Play"
                        Margin="6,0">
                    <!-- Viewbox over a 24 Canvas, so every transport mark is scaled by the SAME
                         factor. Per-Path Stretch="Uniform" scaled each geometry's own bounds to
                         the box, which made play visibly larger than pause for no reason a
                         person could name. -->
                    <Viewbox Width="14" Height="14">
                        <Canvas Width="24" Height="24">
                            <Path x:Name="PlayPauseGlyph"
                                  Fill="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}" />
                        </Canvas>
                    </Viewbox>
                </Button>
                <Button x:Name="Next" Style="{StaticResource TransportButtonStyle}"
                        AutomationProperties.Name="Next track">
                    <Viewbox Width="14" Height="14"><Canvas Width="24" Height="24">
                            <Path Data="{StaticResource IconNext}"
                                  Fill="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}" />
                        </Canvas></Viewbox>
                </Button>
                <!-- Stroked, not filled: the speaker geometries are line art, which is why
                     NotchHud strokes the same three paths. Filling them draws a blob. -->
                <Button x:Name="Output" Style="{StaticResource TransportButtonStyle}"
                        AutomationProperties.Name="Change output device"
                        ToolTip="Change output device"
                        Margin="12,0,0,0">
                    <Viewbox Width="15" Height="15">
                        <Canvas Width="24" Height="24">
                            <Path Data="{StaticResource IconSpeakerBody}"
                                  Stroke="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}"
                                  StrokeThickness="1.6" StrokeLineJoin="Round"
                                  StrokeStartLineCap="Round" StrokeEndLineCap="Round" />
                            <Path Data="{StaticResource IconSpeakerWaveInner}"
                                  Stroke="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}"
                                  StrokeThickness="1.6"
                                  StrokeStartLineCap="Round" StrokeEndLineCap="Round" />
                            <Path Data="{StaticResource IconSpeakerWaveOuter}"
                                  Stroke="{Binding Foreground, RelativeSource={RelativeSource AncestorType=Button}}"
                                  StrokeThickness="1.6"
                                  StrokeStartLineCap="Round" StrokeEndLineCap="Round" />
                        </Canvas>
                    </Viewbox>
                </Button>
            </StackPanel>
        </Grid>

        <!-- Elapsed, the bar, and what is left, across the full content width. Collapsed until a
             timeline arrives: a bar of unknown length is a lie. Filled in by RenderProgress. -->
        <Grid x:Name="ProgressRow" Grid.Row="2" Visibility="Collapsed">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="34" />
                <ColumnDefinition Width="*" />
                <ColumnDefinition Width="38" />
            </Grid.ColumnDefinitions>

            <TextBlock x:Name="Elapsed" Grid.Column="0"
                       FontSize="10.5" VerticalAlignment="Center"
                       Foreground="{DynamicResource NotchInkMuted}" />
            <ProgressBar x:Name="Bar" Grid.Column="1" Margin="8,0"
                         Style="{StaticResource TrackBarStyle}"
                         AutomationProperties.Name="Playback position" />
            <TextBlock x:Name="Remaining" Grid.Column="2"
                       FontSize="10.5" TextAlignment="Right" VerticalAlignment="Center"
                       Foreground="{DynamicResource NotchInkMuted}" />
        </Grid>
    </Grid>
</UserControl>
```

- [ ] **Step 3: Strip the ground out of the code-behind**

In `src/Plith/Views/Widgets/MediaWidget.cs`, delete:

- the `SizeChanged += (_, e) => Clip = ...BottomRoundedClip(...)` line and its comment
- `RenderBackdrop()` in full, and its call inside `Render`
- the `BackdropOpacity` constant and its comment
- the `using System.Windows.Media.Animation;` import only if nothing else uses it
  (`AnimateTrackChange` does, so it stays)

`NotchGeometry.BottomRoundedClip` itself stays: `WeatherWidget` paints a sky and still needs it.

Wire the output control in the constructor, beside the transport handlers:

```csharp
        Output.Click += (_, _) => Plith.Services.SystemSoundPanel.TryOpen();
```

- [ ] **Step 4: Raise the thumbnail decode**

In `src/Plith/ViewModels/MediaViewModel.cs`, replace the `DecodePixelWidth` line and its comment:

```csharp
            // 192, for a 56 DIP tile: 112 px at 200% DPI and 168 at 300%. It was 96, chosen for a
            // 48 DIP row on the classic card, and the notch page drew its tile from the same
            // bitmap. A cap is still wanted, so this is a bigger cap rather than none: the point
            // is to bound what one track change costs.
            bitmap.DecodePixelWidth = 192;
```

- [ ] **Step 5: Give the render harness the states that matter**

In `scripts/render-widgets.ps1`, the stand-in cover's comment says it exists "so the backdrop has
something to blur". Correct it to say the tile has something to draw, and leave the drawing as
it is.

After the existing `widget-media` render (around line 280), add:

```powershell
# Three states rather than one. The layout's risk is not the happy case: it is a title long
# enough to scroll against a 116 DIP column, and the two states where the progress row is absent.
$longVm = [Plith.ViewModels.MediaViewModel]::new()
$longVm.Title = 'Everything In Its Right Place (Remastered 2026 Edition)'
$longVm.Artist = 'A Band With A Fairly Long Name Too'
$longVm.IsPlaying = $true
$longVm.HasSession = $true
$longVm.AlbumArt = $cover
$mediaLong = [Plith.Views.Widgets.MediaWidget]::new($longVm, $null)
Save-Visual -Element $mediaLong -W $frameW -H $frameH -Name 'widget-media-long-title'

$pausedVm = [Plith.ViewModels.MediaViewModel]::new()
$pausedVm.Title = 'You Feel Be Love'
$pausedVm.Artist = 'Denis Phenomen'
$pausedVm.IsPlaying = $false
$pausedVm.HasSession = $true
$pausedVm.AlbumArt = $cover
$mediaPaused = [Plith.Views.Widgets.MediaWidget]::new($pausedVm, $null)
Save-Visual -Element $mediaPaused -W $frameW -H $frameH -Name 'widget-media-paused'

# No session: no artwork, no progress row, transport disabled. The state a person sees if they
# page to this widget with nothing playing, which is the one nobody looks at until it is wrong.
$emptyVm = [Plith.ViewModels.MediaViewModel]::new()
$mediaEmpty = [Plith.Views.Widgets.MediaWidget]::new($emptyVm, $null)
Save-Visual -Element $mediaEmpty -W $frameW -H $frameH -Name 'widget-media-empty'
```

- [ ] **Step 6: Build, render both themes, and look at the result**

Run:
```
dotnet build
pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark
pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -OutDir "$env:TEMP\plith-render-light"
```
Expected: 0 warnings, and eight PNGs per theme including `widget-media`,
`widget-media-long-title`, `widget-media-paused`, `widget-media-empty`.

**Then open the four media PNGs in both themes and check, by eye:** the tile is 56 and crisp
rather than soft, the title and artist are legible on the themed surface in the LIGHT theme (this
is the case the old ink override existed to fake), the rail's four controls fit without touching
the text, the output control reads as separate from the transport, and the bottom 14 DIP is empty
rather than clipped.

- [ ] **Step 7: Run the lints**

Run:
```
pwsh -File scripts/check-a11y.ps1
pwsh -File scripts/check-contrast.ps1
```
Expected: both pass. `check-contrast.ps1` measures this page for the first time, because the ink
override is gone, so a failure here is a real finding about the themed surface rather than a
regression this task introduced. If a pair fails, report the measured ratio and the pair before
changing any colour.

- [ ] **Step 8: Commit**

```bash
git add src/Plith/Views/Widgets/MediaWidget.xaml src/Plith/Views/Widgets/MediaWidget.cs \
        src/Plith/ViewModels/MediaViewModel.cs src/Plith/Services/SystemSoundPanel.cs \
        scripts/render-widgets.ps1
git commit -m "feat(media): make the page the one thing playing, not a list row

- drop the blurred artwork ground, its scrim and the page's own ink, so
  the page follows the theme and its colour pairs are finally visible to
  check-contrast.ps1
- 56 DIP tile, decoded at 192 px rather than 96, which was chosen for a
  48 DIP row and reused for the notch
- a fourth control on the rail opens Windows' sound settings, set apart
  by a 12 DIP gap so it does not read as transport
- reserve the progress row's 14 DIP, collapsed until a timeline arrives

The render harness now draws three states, including a long title against
the narrower text column and the no-session page."
```

---

### Task 4: The progress row and its tick

The bar and the two clocks, moving once a second while the page is on screen.

**Files:**
- Modify: `src/Plith/Views/Widgets/MediaWidget.cs`

**Interfaces:**
- Consumes: `MediaProgress.Elapsed`, `MediaProgress.Clock` (Task 1); `MediaViewModel.Timeline`
  and `MediaTimeline` (Task 2); `ProgressRow`, `Bar`, `Elapsed`, `Remaining` (Task 3).
- Produces: nothing further.

- [ ] **Step 1: Route the view model's notifications by name**

In the constructor of `src/Plith/Views/Widgets/MediaWidget.cs`, replace

```csharp
        _vm.PropertyChanged += (_, _) => Render();
```

with

```csharp
        _vm.PropertyChanged += (_, e) =>
        {
            // The position arrives about once a second on some sources. A full Render reassigns
            // the artwork and both marquees, so it goes straight to the progress row instead.
            if (e.PropertyName == nameof(MediaViewModel.Timeline)) RenderProgress();
            else Render();
        };
```

- [ ] **Step 2: Add the tick, and start it only while the page is on screen**

Add the field beside `_shownTitle`:

```csharp
    /// <summary>
    /// Moves the bar between readings.
    ///
    /// 1 Hz is enough for both halves of the row: the seconds text changes at 1 Hz, and a 232 DIP
    /// bar over a four minute track advances about one DIP per second.
    ///
    /// Started and stopped with visibility rather than left running. A page that is off the tree
    /// is not being looked at, and this widget is built once and kept for the life of the window,
    /// so a timer left running would tick for the whole session to paint nothing.
    /// </summary>
    private readonly DispatcherTimer _tick;
```

Add `using System.Windows.Threading;` to the imports.

In the constructor, replace the existing `IsVisibleChanged` line:

```csharp
        _tick = new DispatcherTimer(DispatcherPriority.Background, Dispatcher)
        {
            Interval = TimeSpan.FromSeconds(1),
        };
        _tick.Tick += (_, _) => RenderProgress();

        // Re-rendered on the way in as well as on change: a page that has been away misses every
        // notification while it is off the tree, so arriving without this would show whatever was
        // playing when it last left.
        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) { Render(); _tick.Start(); }
            else _tick.Stop();
        };
```

- [ ] **Step 3: Write the progress row's render**

Add after `Render()`:

```csharp
    /// <summary>
    /// Where the track is, in the bar and in the two clocks.
    ///
    /// Its own method, and every early return in this feature lives here rather than in Render.
    /// That is not a style choice: Render's one early return used to sit in the middle of it, so
    /// whenever the backdrop was already correct it stopped there and never reached the play/pause
    /// shape. An early return is only safe in a method that does one thing.
    ///
    /// A null timeline collapses the row rather than drawing an empty bar. A source with no end
    /// time (a live stream) reports null, and a bar of unknown length is a lie.
    /// </summary>
    private void RenderProgress()
    {
        var timeline = _vm.Timeline;
        if (timeline is null || !_vm.HasSession)
        {
            ProgressRow.Visibility = Visibility.Collapsed;
            return;
        }

        ProgressRow.Visibility = Visibility.Visible;

        var elapsed = MediaProgress.Elapsed(timeline.Position, timeline.LastUpdated,
                                            timeline.Duration, _vm.IsPlaying, DateTimeOffset.Now);

        // Duration is positive by construction: ReadTimeline returns null otherwise, which is
        // what makes this division safe without a guard here.
        Bar.Value = elapsed / timeline.Duration * 100;
        Elapsed.Text = MediaProgress.Clock(elapsed);
        Remaining.Text = "-" + MediaProgress.Clock(timeline.Duration - elapsed);
    }
```

Add `using Plith.Services;` to the imports if it is not already there.

Call it from `Render()`, at the end, where `RenderBackdrop()` used to be called:

```csharp
        RenderProgress();
```

- [ ] **Step 4: Build and render**

Run:
```
dotnet build
pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark
```
Expected: 0 warnings. The three seeded states still have no timeline, so their progress rows are
still collapsed. That is correct and is why the next step seeds one.

- [ ] **Step 5: Give the harness a state with a timeline, and look at it**

In `scripts/render-widgets.ps1`, after `$mediaVm.AlbumArt = $cover`, add:

```powershell
# A track 2:27 into 4:27, stamped now, so the bar and both clocks are in shot. Seeded on the view
# model rather than through a snapshot: the harness has no SMTC session and does not need one.
$mediaVm.Timeline = [Plith.Services.MediaTimeline]::new(
    [TimeSpan]::FromSeconds(147), [TimeSpan]::FromSeconds(267), [DateTimeOffset]::Now)
$pausedVm.Timeline = [Plith.Services.MediaTimeline]::new(
    [TimeSpan]::FromSeconds(12), [TimeSpan]::FromSeconds(267), [DateTimeOffset]::Now)
```

Note: `$pausedVm` is created later in the file, so put its line beside its own creation rather
than here. The long-title state keeps no timeline deliberately: that is the live-stream case, and
it shows the row collapsing under a title that scrolls.

Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark` and
`pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -OutDir "$env:TEMP\plith-render-light"`

**Then look:** the bar sits at about 55% of its width, `2:27` on the left, `-2:00` on the right,
the fill is the accent colour in both themes, and the paused state shows `0:12` with the bar near
its start. The accent fill against `NotchTrack` must be visible in the LIGHT theme, which is
where an accent chosen for a dark panel goes pale.

- [ ] **Step 6: Run the lints and the suite**

Run:
```
pwsh -File scripts/check-a11y.ps1
pwsh -File scripts/check-contrast.ps1
dotnet test
```
Expected: lints pass, 519 passing.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Views/Widgets/MediaWidget.cs scripts/render-widgets.ps1
git commit -m "feat(media): show where the track is, and keep it moving

The bar and both clocks are painted from the interpolated position and
ticked at 1 Hz, only while the page is on screen. The view model's
notifications are routed by name so a position arriving once a second
repaints the progress row rather than the artwork and both marquees.

A source with no end time collapses the row instead of drawing a bar
whose length nobody knows."
```

---

### Task 5: Open on the media page while something is playing

**Files:**
- Create: `src/Plith/Views/Presentation/NotchOpeningPolicy.cs`
- Modify: `src/Plith/Services/NotchPager.cs`
- Modify: `src/Plith/Views/OsdHost.cs` (`ApplyWidgetPages`, `OnNotchClicked`)
- Test: `tests/Plith.Tests/NotchOpeningPolicyTests.cs`, `tests/Plith.Tests/NotchPagerTests.cs`

**Interfaces:**
- Consumes: nothing from Tasks 1 to 4.
- Produces:
  - `static int NotchOpeningPolicy.OpeningPage(bool isPlaying, int mediaPageIndex)`
  - `void NotchPager.ResetTo(int index)`

- [ ] **Step 1: Write the failing tests**

Create `tests/Plith.Tests/NotchOpeningPolicyTests.cs`:

```csharp
using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchOpeningPolicyTests
{
    [Fact]
    public void PlayingOpensOnTheMediaPage()
    {
        Assert.Equal(2, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: 2));
    }

    [Fact]
    public void NotPlayingOpensOnTheFirstPage()
    {
        // A paused session is not what is happening now. This is the decision the spec argues:
        // a Spotify left open and paused for days would otherwise lock the notch onto the media
        // page and the clock would never come first again.
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: false, mediaPageIndex: 2));
    }

    [Fact]
    public void PlayingWithNoMediaPageInstalledOpensOnTheFirstPage()
    {
        // The page list is rebuilt as pages come and go, and the index is -1 while there is no
        // media page in it. Opening on -1 would throw or clamp to a page nobody asked for.
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: -1));
    }

    [Fact]
    public void TheMediaPageBeingFirstIsNotASpecialCase()
    {
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: 0));
    }
}
```

Append to `tests/Plith.Tests/NotchPagerTests.cs`:

```csharp
    [Fact]
    public void ResetTo_GoesToThePageAsked()
    {
        var pager = new NotchPager(4);

        pager.ResetTo(2);

        Assert.Equal(2, pager.Index);
    }

    [Fact]
    public void ResetTo_ClampsRatherThanWrapping()
    {
        // GoTo wraps, because a page-dot click past the end means the other end. An opening page
        // is not a gesture: an index past the end is a bug upstream, and wrapping would hide it
        // by opening somewhere plausible.
        var pager = new NotchPager(3);

        pager.ResetTo(7);
        Assert.Equal(2, pager.Index);

        pager.ResetTo(-2);
        Assert.Equal(0, pager.Index);
    }

    [Fact]
    public void ResetTo_ForgetsTheLastGesturesTiming()
    {
        var g = new Gesture();
        g.Feed(NotchPager.CommitThreshold);   // pages once, leaving the pager unarmed

        g.Pager.ResetTo(0);

        // Armed again, and measured from nothing: a swipe after the notch was reopened must not
        // be read as the continuation of the swipe that closed it.
        Assert.True(g.Pager.Accumulate(NotchPager.CommitThreshold, 0));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `dotnet test tests/Plith.Tests --filter "NotchOpeningPolicyTests|NotchPagerTests"`
Expected: FAIL to compile, `NotchOpeningPolicy` and `ResetTo` do not exist.

- [ ] **Step 3: Write the policy**

Create `src/Plith/Views/Presentation/NotchOpeningPolicy.cs`:

```csharp
namespace Plith.Views.Presentation;

/// <summary>
/// Which widget page the notch opens on.
///
/// A class of its own, free of WPF, for the reason NotchEventPolicy is: OsdHost is a BandWindow
/// the test project cannot construct, so a rule that lives inside it has no test, and two
/// previous versions of the event rule reached a running build with nothing between them.
///
/// It also keeps the carousel spec's property that the opening page is COMPUTED at open time and
/// never stored. Remembering the last page is deliberately deferred, and a value written in a
/// completion handler is the exact hazard that produced five defects on this branch.
/// </summary>
public static class NotchOpeningPolicy
{
    /// <summary>
    /// The page to open on.
    ///
    /// Playing rather than merely having a session: the opening page should be what is happening
    /// now, and a paused session is not that.
    /// </summary>
    /// <param name="isPlaying">Whether the current media session reports Playing.</param>
    /// <param name="mediaPageIndex">Where the media page sits in the current page list, or -1
    /// when it is not installed.</param>
    public static int OpeningPage(bool isPlaying, int mediaPageIndex)
        => isPlaying && mediaPageIndex >= 0 ? mediaPageIndex : 0;
}
```

- [ ] **Step 4: Add `ResetTo` to the pager**

In `src/Plith/Services/NotchPager.cs`, replace `Reset()` with:

```csharp
    /// <summary>Return to the first page. Used when the notch closes.</summary>
    public void Reset() => ResetTo(0);

    /// <summary>
    /// Open on a given page, clamping into range.
    ///
    /// Clamps rather than wrapping, which is what <see cref="GoTo"/> does: a dot clicked past the
    /// end means the other end, but an opening index past the end is a bug upstream and wrapping
    /// would hide it by opening somewhere plausible.
    ///
    /// Forgets the last gesture's timing as well as its accumulator, so a swipe long after the
    /// notch reopened is never measured against the one that closed it.
    /// </summary>
    public void ResetTo(int index)
    {
        Index = Math.Clamp(index, 0, PageCount - 1);
        RestAndForgetTiming();
    }
```

- [ ] **Step 5: Track where the media page sits, and open on it**

In `src/Plith/Views/OsdHost.cs`, beside `private int _shelfPageIndex = -1;` add:

```csharp
    /// <summary>Where the media page sits in the current list, so the opening rule can name it.
    /// Read from the same list that installs the pages, so the two cannot disagree about an
    /// order they both take from one place.</summary>
    private int _mediaPageIndex = -1;
```

In `ApplyWidgetPages`, after `pages.Add(_mediaPage);`:

```csharp
        _mediaPageIndex = pages.Count - 1;
```

In `OnNotchClicked`, replace

```csharp
        _pager.Reset();
        _widgets.SyncToPager(0);
```

with

```csharp
        // Computed here, on the way in, rather than remembered. See NotchOpeningPolicy: the page
        // is a function of what is playing right now, and the carousel spec's deferral of
        // remembering the last page is kept deliberately.
        _pager.ResetTo(NotchOpeningPolicy.OpeningPage(_media?.IsPlaying == true, _mediaPageIndex));
        _widgets.SyncToPager(0);
```

`_media` is the `MediaViewModel` handed to `AttachAudioSource`. It is not currently held as a
field: add one beside `_toggleMute` and assign it in `AttachAudioSource`:

```csharp
    /// <summary>The media view model, held so the opening-page rule can ask what is playing.
    /// Null until AttachAudioSource runs, which is why the rule takes a bool rather than the
    /// view model.</summary>
    private ViewModels.MediaViewModel? _media;
```
```csharp
        _media = media;
```

`SyncToPager(0)` keeps its zero: that argument is the slide DIRECTION, not a page, and an opening
frame does not slide.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `dotnet test`
Expected: 526 passing (519 plus 7), 0 failed. `dotnet build` with 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Views/Presentation/NotchOpeningPolicy.cs src/Plith/Services/NotchPager.cs \
        src/Plith/Views/OsdHost.cs tests/Plith.Tests/NotchOpeningPolicyTests.cs \
        tests/Plith.Tests/NotchPagerTests.cs
git commit -m "feat(notch): open on what is playing, not always on the clock

A click on the notch now opens the media page while something is playing,
and the clock page otherwise. Playing rather than merely having a session:
a paused Spotify would otherwise lock the notch onto the media page for
days.

The rule is a pure policy computed at open time, never stored, so it keeps
the property the pager was given for: an index owned by an object with no
animation and no completion handler cannot go stale."
```

---

### Task 6: Run it, and write down what happened

Everything above is green build, green tests, green lints and a render, and this branch's whole
lesson is that those four can all be green while the running product is broken. This task is the
one that presses a key.

**Files:**
- Create: `scripts/drive-media-page.ps1`
- Modify: `docs/PHASE6-VERIFICATION.md` (new section 20)
- Modify: `docs/ROADMAP.md`
- Modify: `CLAUDE.md` (the Status section)
- Modify: `docs/superpowers/plans/2026-09-20-media-widget-alcove.md` (tick the boxes)

**Interfaces:**
- Consumes: everything above.
- Produces: a recorded measurement, not code anything calls.

- [ ] **Step 1: Write the driving script**

Create `scripts/drive-media-page.ps1`. It follows `scripts/drive-shelf-pair.ps1`: same session
precondition, same `Add-Verdict` shape, same UI Automation client. The reason it can drive Plith
at all is recorded in `CLAUDE.md`: `app.manifest` sets `uiAccess="false"` and only Release swaps
in the signed one, so a Debug Plith runs at MEDIUM integrity and `SendInput` reaches it.

```powershell
#requires -Version 7
<#
.SYNOPSIS
  Drives the real notch and answers one question: does it open on the media page while something
  is playing?

.DESCRIPTION
  Refuses to run rather than guessing when its precondition is not met. Two preconditions:

  1. The session must be Active. A disconnected or locked session has no desktop, so nothing can
     be pressed. Same check as drive-shelf-pair.ps1.

  2. Something must actually be playing, and that is asked of SMTC through Plith's OWN
     MediaSessionClient rather than assumed. A run against a silent machine would pass the
     "not playing opens on the clock" half and silently skip the half worth measuring.

  Run with: pwsh -File scripts/drive-media-page.ps1
#>

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-drive-media",
    [string]$Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$plithExe = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0\Plith.exe"
$plithDll = [IO.Path]::ChangeExtension($plithExe, '.dll')
if (-not (Test-Path $plithExe)) { throw "Build $Configuration first: $plithExe not found." }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$session = qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }
if ($session -notmatch 'Active') {
    throw ("This session is not Active (`qwinsta` says: $($session -replace '\s+', ' ')). A " +
           "locked or disconnected session has no desktop: nothing can be pressed. Reconnect " +
           "and re-run.")
}

# Ask SMTC what is playing, through the product's own client.
Add-Type -Path $plithDll -ErrorAction Stop
$probe = [Plith.Services.MediaSessionClient]::new()
$probe.StartAsync().GetAwaiter().GetResult()
Start-Sleep -Milliseconds 800     # the first snapshot is read asynchronously
$playing = $probe.IsCurrentSessionPlaying
$aumid = $probe.CurrentSourceAppUserModelId
$probe.Dispose()

if (-not $playing) {
    throw ("Nothing is playing, so the measurement worth taking cannot be taken. Start a track " +
           "(Spotify, a browser tab, anything that publishes to SMTC), leave it PLAYING, and " +
           "re-run. SMTC reports source '$aumid'.")
}
"playing: $aumid"
```

The rest of the script presses the notch and reads the tree. Copy these four blocks verbatim
from `scripts/drive-shelf-pair.ps1` rather than importing them, which is what `drive-shelf.ps1`
and `drive-shelf-pair.ps1` already do with each other: the `Add-Type` block defining `PairInput`
and `PairWin` (lines 119 to 212), `Find-LayeredWindow`, `Get-Element`, `Get-Names`,
`Move-Pointer`, `Add-Verdict` and `Assert-InputWorks`. **Their exact names matter**, so here they
are as they exist: clicking is `[PairInput]::LeftClick()`, there is no `Invoke-Click`; a window's
rectangle comes from `New-Object 'PairInput+RECT'` plus `[PairWin]::GetWindowRect($h, [ref]$r)`,
there is no `Get-WindowRect`; and there is no `Find-ByName`, so the bar is found with a
`PropertyCondition`.

`Assert-InputWorks` is not optional. It exists because `SendInput` can have LEFTDOWN refused
while LEFTUP is accepted, which sends half a click and then measures a meaningless result with
the product blameless. Call it once before any press.

```powershell
# Start Plith if it is not up. A stale instance holds the single-instance mutex and the new one
# exits silently: that made the drop catcher look guilty for a failure that was not its, recorded
# as instrument defect 5 in docs/SHELF-VERIFICATION.md.
if (-not (Get-Process Plith -ErrorAction SilentlyContinue)) {
    Start-Process $plithExe
    Start-Sleep -Seconds 4
}

Assert-InputWorks

$notch = Find-LayeredWindow -ProcessName 'Plith'
if (-not $notch) { throw "No notch window found. Is the presentation set to Ambient Notch?" }

# Click the resting notch: top-centre of its own window, a few pixels down.
$rect = New-Object 'PairInput+RECT'
[void][PairWin]::GetWindowRect($notch, [ref]$rect)
$x = [int](($rect.Left + $rect.Right) / 2)
$y = [int]($rect.Top + 4)
Move-Pointer -X $x -Y $y -Settle 250
[PairInput]::LeftClick()
Start-Sleep -Milliseconds 700      # the expansion animates

# Read the page, not the pixels. The media page's root carries AutomationProperties.Name
# "Now playing", added in Task 3, so this asks the tree which page is mounted.
$element = Get-Element $notch
$names = Get-Names $element
$onMedia = $names -contains 'Now playing'
Add-Verdict 'a click while playing opens the media page' $onMedia `
    "names in the tree: $($names -join ', ')"

# And the bar reached the tree with a value, which is the other half of the accessibility claim
# Task 3 makes by using a real ProgressBar.
$condition = New-Object Windows.Automation.PropertyCondition(
    [Windows.Automation.AutomationElement]::NameProperty, 'Playback position')
$bar = $element.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
$value = 'bar not in the tree'
$hasValue = $false
if ($bar) {
    $pattern = $bar.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    $value = $pattern.Current.Value
    $hasValue = $value -gt 0
}
Add-Verdict 'the progress bar reports a value to UI Automation' $hasValue "value: $value"

$script:verdicts
if ($script:failed -gt 0) { exit 1 }
```

`Find-LayeredWindow` in `drive-shelf-pair.ps1` takes no `-ProcessName`: it finds the catcher's
window. Narrow the copy to Plith's own process id, so a catcher window left over from a shelf run
cannot be mistaken for the notch.

- [ ] **Step 2: Run it, with something actually playing**

Start a track and leave it playing, then run:
```
pwsh -File scripts/drive-media-page.ps1
```
Expected: both verdicts PASS.

If the first fails with the clock page's names in the tree, the likely causes in order are:
`_mediaPageIndex` never assigned because `ApplyWidgetPages` returned early on its no-change
guard, `_media` still null because `AttachAudioSource` had not run, or the click landing on a HUD
rather than the resting notch. Report which, with the names the tree returned.

- [ ] **Step 3: Take the paused half too**

Pause the track, then run the script again. It will now refuse to run, by design, and that
refusal is itself the second half of the measurement: re-run it with the guard temporarily
inverted, or click the notch by hand and confirm the clock page comes first. Record which you
did. A verdict from a run that skipped its own precondition is not a verdict.

- [ ] **Step 4: Record it in the verification ledger**

Add a section 20 to `docs/PHASE6-VERIFICATION.md`, following the shape of section 19. It must
carry: the date, what was run, the AUMID SMTC reported, both verdicts with their evidence
strings, whether the paused half was measured or reasoned, and anything the renders showed that
the design did not predict. Findings, not a summary: if the title column at 116 DIP reads badly,
that belongs here as a measurement with the render's filename.

- [ ] **Step 5: Update the roadmap and the status**

In `docs/ROADMAP.md`, record the media page's redesign under Phase 6 with its measured state.

In `CLAUDE.md`, add a paragraph to the Status section. It must say which parts were run and which
were not, in this file's own idiom, and it must not claim more than step 2 and step 3 actually
returned. **Read the banner at the top of that Status section first:** it is split across two
branches and says whichever merges second must merge it by hand.

- [ ] **Step 6: Tick this plan's boxes and commit**

Tick every box in this plan that was completed. Boxes are ticked as work proceeds, not at the
end: a plan file whose boxes are all empty reads as "never started", which is how 199 boxes came
to be unticked across three finished phases in this repo.

```bash
git add docs/PHASE6-VERIFICATION.md docs/ROADMAP.md CLAUDE.md \
        scripts/drive-media-page.ps1 docs/superpowers/plans/2026-09-20-media-widget-alcove.md
git commit -m "docs(media): record the hardware run behind the new media page

scripts/drive-media-page.ps1 drives the real notch and asks the UIA tree
which page opened, rather than looking at pixels. It refuses to run with
nothing playing, because a run against a silent machine would pass the
half that does not matter and skip the half that does."
```

---

## Self-Review

**Spec coverage.** Every section of
`docs/superpowers/specs/2026-09-20-media-widget-alcove-design.md` maps to a task:

| Spec section | Task |
|---|---|
| Layout, all numbers | 3 |
| Timeline data, the once-a-second trap, `MediaProgress` | 1, 2 |
| Ground, art and the page's own colours | 3 |
| Output device button | 3 (folded in: the rail's width is part of the layout, and splitting it would ship two different layouts) |
| The opening page | 5 |
| What gets deleted | 3 (ground, clip, backdrop), 5 (`Reset()` at the click site) |
| Testing: unit tests | 1, 2, 5 |
| Testing: render harness, contrast, a11y, hardware drive | 3, 4, 6 |
| Risks 1 to 3 | 3 step 6 (title column), 4 step 5 (sources that report nothing), 6 (the sound panel's weight) |

**Deliberate deviation from the spec, recorded rather than silent:** the spec's testing section
lists a `Fraction` helper in passing; it is not in this plan because `RenderProgress` computes
the elapsed time anyway and dividing it there is one line, so a second pure function would have
no caller. `Elapsed` and `Clock` are the two the code uses.

**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Two steps
deliberately ask for judgement rather than code, and both name exactly what to judge and what to
report: Task 3 step 6 (look at the renders) and Task 6 step 4 (write the findings). The task-6
script is given in two pieces with named helpers to copy from `drive-shelf-pair.ps1`, because
transcribing 300 lines of P/Invoke into a plan would be a copy that can drift from the original.

**Type consistency.** `MediaTimeline(Position, Duration, LastUpdated)` is constructed with that
order in Tasks 2, 4 and the harness. `MediaProgress.Elapsed(position, lastUpdated, duration,
isPlaying, now)` is called with that order in Task 4 and tested with it in Task 1, and note that
its parameter order is not the record's: the harness and `RenderProgress` name the arguments in
the record's order, which is why `RenderProgress` passes them explicitly rather than by position
juggling. `NotchOpeningPolicy.OpeningPage(bool, int)` and `NotchPager.ResetTo(int)` match between
Task 5's tests and its implementation. `ProgressRow`, `Bar`, `Elapsed`, `Remaining` are declared
in Task 3's XAML and used by those names in Task 4. `Elapsed` is both a XAML element name and a
`MediaProgress` method: inside `RenderProgress` the element is `Elapsed.Text` and the method is
called as `MediaProgress.Elapsed(...)`, which compiles, but a reviewer should confirm no bare
`Elapsed(...)` call is introduced.
