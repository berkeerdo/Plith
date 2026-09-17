# Shelf: Drop Catcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a person drag files onto the notch and have them stay there, on a window that cannot receive drops.

**Architecture:** The OSD window runs at High integrity because it has UIAccess, and UAC refuses the drag Explorer tries to hand it. So a second process at Medium integrity — `Plith.DropCatcher` — owns the drop. It cannot sit above the notch, because a Medium window cannot enter the UIAccess band, so it takes the notch's place for the duration of a drag: the notch sees the drag coming, hides, the catcher appears in the same rectangle, receives the drop, and sends the paths back over a named pipe whose ACL is opened deliberately.

**Tech Stack:** WPF / .NET 10, `System.IO.Pipes` with an explicit `PipeSecurity`, `GetCursorPos` + `GetAsyncKeyState` polling, the existing `NotchHoverPoller` and `BandWindow`.

**Spec:** `docs/ROADMAP.md` — Phase 7, the Shelf card entry, which records the measurements this plan is built on.

> **Tasks 1-4 are done and their boxes are ticked. A tick means the step was taken, NOT that
> nothing was deviated from.** Four deviations, each recorded in the code that carries it:
>
> 1. **The wire carries physical pixels, not DIP.** Two processes each doing their own DIP
>    arithmetic disagree on any monitor that is not at 100%.
> 2. **Task 4 Step 1 could not be done as written.** `efd0db8^` does not contain the drag
>    detection — the spike lived only in a working tree and was never committed. It was rebuilt
>    from the roadmap's description, which that commit wrote for exactly this case.
> 3. **The plan's own `DropChannel` code had two defects** and was not copied as printed: chained
>    Replace calls corrupt ordinary Windows paths, and the numbers were culture-formatted while
>    the decoder parsed invariant.
> 4. **Entering and staying use different rectangles.** Not in the plan; found by running it.
>
> Three defects were found by running rather than building, all with a green build behind them:
> `EnsureHandle` + `SWP_SHOWWINDOW` makes a window WPF does not consider shown (no visual tree,
> no drop target, nothing on screen); `AllowsTransparency` makes it layered and therefore
> invisible to every screen-capture route over RDP; and a single approach threshold pulled the
> notch back 700 ms into a live drag.

## Global Constraints

- All code, comments and commit messages in English. Conventional Commits. No AI attribution anywhere.
- Target `net10.0-windows10.0.22000.0`, x64, `Nullable=enable`.
- `tests/Plith.Installer.Tests` must not be modified (three known pre-existing CA1861 warnings).
- Build with `-m:1`; never delete `obj/` (it silently omits BAML).
- `scripts/check-a11y.ps1`, `scripts/check-shared-xaml.ps1` and `scripts/check-contrast.ps1` must pass before any release build.
- Never delete the `CN=Plith Self-Signed` certificate.
- `Plith.exe` keeps `uiAccess="true"`. That is not negotiable — it is what lets the OSD draw over a full-screen game, and it is also the reason this plan exists.
- `Plith.DropCatcher.exe` must ship **without** `uiAccess` and must never be launched as a child of `Plith.exe`, or Windows gives it High integrity and the whole design collapses. It is launched by the shell.

## What is already measured

These are not assumptions. Each was tested on this machine before the plan was written; re-testing them is not part of the work.

| Question | Answer | How |
|---|---|---|
| Is a drop target registered on the band window? | Yes | Read the window's `OleDropTargetInterface` property |
| Does an OLE drag reach it? | **No** | Armed, filtered, panel open, file released on it — no `DragEnter` |
| Does `WM_DROPFILES` reach it? | **No** | `DragAcceptFiles` + message filter; log caught the release, no message |
| Why? | Plith is High (UIAccess), Explorer is Medium; UAC blocks the COM call | Token integrity levels read directly |
| Can the notch see a drag approaching? | **Yes** | `GetCursorPos` + `GetAsyncKeyState` answer while the source owns the mouse |
| Can a Medium process write to a High process's pipe? | **Only with an explicit ACL** | Default ACL: "Access to the path is denied". Explicit Everyone-RW: connected and delivered |
| Does the Run key give Medium integrity? | Yes for a non-uiAccess exe | Plith's own entry is there; uiAccess is what raises Plith |

## File Structure

**New project — `src/Plith.DropCatcher/`**
- `Plith.DropCatcher.csproj` — WPF exe, net10.0-windows, x64.
- `app.manifest` — `asInvoker`, **no** uiAccess, PerMonitorV2.
- `App.xaml` / `App.xaml.cs` — single-instance guard, connects to the pipe, owns the window.
- `CatcherWindow.xaml` / `.cs` — borderless, topmost, transparent, `AllowDrop`; shown only while a drag is in flight.
- `CatcherClient.cs` — the pipe client: reads commands, writes drops.

**Shared contract — `src/Plith/Services/Shelf/`**
- `DropChannel.cs` — the wire format and the pipe name. One file, referenced by both sides via a linked compile item so the two cannot drift.
- `DropChannelServer.cs` — Plith's end: creates the pipe with the permissive ACL, dispatches commands, raises `FilesDropped`.

**Plith side**
- `src/Plith/Views/Presentation/NotchHoverPoller.cs` — restore `DraggingOverChanged` (removed in `efd0db8`; the mechanism is described in the roadmap).
- `src/Plith/Views/OsdHost.cs` — hand the rectangle over and take it back.
- `src/Plith/Services/Shelf/ShelfStore.cs` — what is staged, and where it persists.
- `src/Plith/Views/Widgets/ShelfWidget.xaml` / `.cs` — the notch page.

**Installer**
- `src/Plith.Installer/Plith.Installer.csproj` — publish and embed the catcher alongside Plith.
- The catcher gets its own Run-key entry so the shell starts it at Medium.

---

### Task 1: The pipe, with the ACL that makes it reachable

The riskiest assumption in the design, so it goes first and is proven in code rather than in a scratch script.

**Files:**
- Create: `src/Plith/Services/Shelf/DropChannel.cs`
- Create: `src/Plith/Services/Shelf/DropChannelServer.cs`
- Test: `tests/Plith.Tests/DropChannelTests.cs`

**Interfaces:**
- Consumes: nothing.
- Produces: `DropChannel.PipeName(string userSid) -> string`; `DropChannel.Encode(DropMessage) -> string`; `DropChannel.TryDecode(string, out DropMessage) -> bool`; `DropMessage(DropVerb Verb, double X, double Y, double W, double H, IReadOnlyList<string> Paths)`; `DropVerb { Show, Hide, Dropped, Hello }`; `DropChannelServer.Start()`, `.SendAsync(DropMessage)`, `event Action<DropMessage> Received`, `.Dispose()`.

- [x] **Step 1: Write the failing test for the wire format**

```csharp
[Fact]
public void Encode_ThenDecode_RoundTripsAShowCommand()
{
    var sent = new DropMessage(DropVerb.Show, 708, 0, 384, 130, []);

    Assert.True(DropChannel.TryDecode(DropChannel.Encode(sent), out var back));

    Assert.Equal(DropVerb.Show, back.Verb);
    Assert.Equal(384, back.W);
    Assert.Empty(back.Paths);
}

/// <summary>A path with a tab or a newline in it must not be able to forge a second message.
/// The catcher runs at a lower integrity level than Plith, so everything arriving from it is
/// untrusted input by definition — this is the only place that is enforced.</summary>
[Theory]
[InlineData("C:\\a\tb.txt")]
[InlineData("C:\\a\nDropped\t0\t0\t0\t0\tC:\\evil.exe")]
public void Encode_NeutralisesSeparatorsInsidePaths(string hostile)
{
    var encoded = DropChannel.Encode(new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, [hostile]));

    Assert.True(DropChannel.TryDecode(encoded, out var back));
    Assert.Single(back.Paths);
}
```

- [x] **Step 2: Run it and watch it fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: FAIL — `DropChannel` does not exist.

- [x] **Step 3: Write `DropChannel`**

```csharp
namespace Plith.Services.Shelf;

public enum DropVerb { Hello, Show, Hide, Dropped }

/// <param name="Paths">Only ever populated on <see cref="DropVerb.Dropped"/>, and always from
/// the catcher — which runs at a lower integrity level than this process. Untrusted.</param>
public readonly record struct DropMessage(
    DropVerb Verb, double X, double Y, double W, double H, IReadOnlyList<string> Paths);

/// <summary>
/// The wire between Plith and the drop catcher.
///
/// One file, compiled into both, so the two ends cannot drift into disagreeing about the format.
/// Text rather than a serializer: the payload is four numbers and a list of paths, and a
/// serializer would be a dependency plus an attack surface for the sake of nothing.
/// </summary>
public static class DropChannel
{
    private const char Separator = '\t';

    /// <summary>Per-user, because two signed-in users each get their own Plith and their own
    /// catcher, and a shared name would let one session's catcher answer another's.</summary>
    public static string PipeName(string userSid) => $"Plith.DropCatcher.{userSid}";

    public static string Encode(DropMessage m)
    {
        // Escaped, not rejected. A file may legitimately contain almost anything, and a path the
        // catcher cannot report is a file that silently fails to arrive.
        var paths = m.Paths.Select(p => p.Replace("\\", "\\\\").Replace("\t", "\\t").Replace("\n", "\\n"));
        return string.Join(Separator, [m.Verb.ToString(), m.X, m.Y, m.W, m.H, .. paths]);
    }

    public static bool TryDecode(string line, out DropMessage message)
    {
        message = default;
        if (string.IsNullOrEmpty(line)) return false;

        var parts = line.Split(Separator);
        if (parts.Length < 5) return false;
        if (!Enum.TryParse<DropVerb>(parts[0], out var verb)) return false;

        static double N(string s) => double.TryParse(s, System.Globalization.NumberStyles.Float,
            System.Globalization.CultureInfo.InvariantCulture, out var v) ? v : 0;

        var paths = parts.Skip(5)
            .Select(p => p.Replace("\\t", "\t").Replace("\\n", "\n").Replace("\\\\", "\\"))
            .ToArray();

        message = new DropMessage(verb, N(parts[1]), N(parts[2]), N(parts[3]), N(parts[4]), paths);
        return true;
    }
}
```

- [x] **Step 4: Run the test and watch it pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: PASS.

- [x] **Step 5: Write the failing test for the server's reachability**

```csharp
/// <summary>
/// The assumption the whole design rests on: a lower-integrity process must be able to reach
/// this pipe. Measured before the design was written — a default ACL answers "Access to the
/// path is denied", and an explicit one connects. This test cannot reproduce the integrity
/// difference in-process, so it asserts the thing that CAUSES the difference: that the pipe is
/// created with a rule for Everyone rather than with the default.
/// </summary>
[Fact]
public void Server_OpensThePipeToEveryone()
{
    using var server = new DropChannelServer("S-1-5-21-test");
    server.Start();

    var security = server.DebugSecurity();
    var rules = security.GetAccessRules(true, false, typeof(System.Security.Principal.SecurityIdentifier));

    Assert.Contains(rules.Cast<System.IO.Pipes.PipeAccessRule>(),
        r => ((System.Security.Principal.SecurityIdentifier)r.IdentityReference).IsWellKnown(
                 System.Security.Principal.WellKnownSidType.WorldSid)
             && r.AccessControlType == System.Security.AccessControl.AccessControlType.Allow);
}
```

- [x] **Step 6: Run it and watch it fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: FAIL — `DropChannelServer` does not exist.

- [x] **Step 7: Write `DropChannelServer`**

```csharp
using System.IO;
using System.IO.Pipes;
using System.Security.AccessControl;
using System.Security.Principal;

namespace Plith.Services.Shelf;

/// <summary>
/// Plith's end of the wire.
///
/// The ACL is the whole point of this class. Plith runs at High integrity because it has
/// UIAccess, and a pipe it creates with the default security carries a High mandatory label —
/// a Medium process cannot write to it. Measured: default ACL gives "Access to the path is
/// denied"; an explicit rule for Everyone connects and delivers.
///
/// That is a deliberate widening, so it is stated rather than buried: any process on this
/// machine can connect to this pipe and send a Dropped message. Everything arriving is treated
/// as a claim about paths, never as a command — see ShelfStore, which stats what it is told
/// about and stores nothing it cannot see.
/// </summary>
public sealed class DropChannelServer : IDisposable
{
    private readonly string _pipeName;
    private readonly DiagnosticLog? _log;
    private NamedPipeServerStream? _pipe;
    private CancellationTokenSource? _cts;

    public DropChannelServer(string userSid, DiagnosticLog? log = null)
    {
        _pipeName = DropChannel.PipeName(userSid);
        _log = log;
    }

    public event Action<DropMessage>? Received;

    public void Start()
    {
        _cts = new CancellationTokenSource();
        _pipe = CreatePipe();
        _ = Task.Run(() => AcceptLoop(_cts.Token));
    }

    private NamedPipeServerStream CreatePipe()
    {
        var security = new PipeSecurity();
        security.AddAccessRule(new PipeAccessRule(
            new SecurityIdentifier(WellKnownSidType.WorldSid, null),
            PipeAccessRights.ReadWrite, AccessControlType.Allow));

        return NamedPipeServerStreamAcl.Create(
            _pipeName, PipeDirection.InOut, 1, PipeTransmissionMode.Byte,
            PipeOptions.Asynchronous, 0, 0, security);
    }

    internal PipeSecurity DebugSecurity() => _pipe!.GetAccessControl();

    // Remaining members (AcceptLoop, SendAsync, Dispose) are written in Step 9.
}
```

- [x] **Step 8: Run the test and watch it pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: PASS.

- [x] **Step 9: Write the read/write loop**

```csharp
    private async Task AcceptLoop(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await _pipe!.WaitForConnectionAsync(ct);
                using var reader = new StreamReader(_pipe, leaveOpen: true);

                while (!ct.IsCancellationRequested && _pipe.IsConnected)
                {
                    var line = await reader.ReadLineAsync(ct);
                    if (line is null) break;
                    if (DropChannel.TryDecode(line, out var message)) Received?.Invoke(message);
                }
            }
            catch (OperationCanceledException) { return; }
            catch (IOException ex)
            {
                // The catcher died or was restarted. Ordinary, not a fault: rebuild the pipe and
                // wait again rather than leaving the shelf permanently deaf.
                _log?.Info("DropChannel", $"Client gone: {ExceptionText.Describe(ex)}");
            }

            try { _pipe!.Disconnect(); } catch { }
        }
    }

    public async Task SendAsync(DropMessage message)
    {
        if (_pipe is not { IsConnected: true }) return;

        try
        {
            var writer = new StreamWriter(_pipe, leaveOpen: true) { AutoFlush = true };
            await writer.WriteLineAsync(DropChannel.Encode(message));
        }
        catch (IOException ex)
        {
            _log?.Info("DropChannel", $"Send failed: {ExceptionText.Describe(ex)}");
        }
    }

    public void Dispose()
    {
        _cts?.Cancel();
        _pipe?.Dispose();
        _cts?.Dispose();
    }
```

- [x] **Step 10: Run the whole suite**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`
Expected: PASS, count up by the new tests.

- [x] **Step 11: Commit**

```bash
git add src/Plith/Services/Shelf tests/Plith.Tests/DropChannelTests.cs
git commit -m "feat(shelf): a pipe a lower-integrity process can reach"
```

---

### Task 2: The catcher process

**Files:**
- Create: `src/Plith.DropCatcher/Plith.DropCatcher.csproj`, `app.manifest`, `App.xaml`, `App.xaml.cs`, `CatcherWindow.xaml`, `CatcherWindow.xaml.cs`, `CatcherClient.cs`
- Modify: `Plith.sln`

**Interfaces:**
- Consumes: `DropChannel` (linked compile item, not a project reference — the catcher must not drag Plith's assembly in).
- Produces: an exe that connects, shows a window on `Show`, hides on `Hide`, and sends `Dropped`.

- [x] **Step 1: Create the project and manifest**

`app.manifest` — the critical file:

```xml
<requestedExecutionLevel level="asInvoker" uiAccess="false" />
```

This is the entire reason the catcher exists. A uiAccess manifest here would raise it to High and it would be as unreachable as Plith.

- [x] **Step 2: Link the shared contract rather than referencing Plith**

```xml
<ItemGroup>
  <Compile Include="..\Plith\Services\Shelf\DropChannel.cs">
    <Link>Shared\DropChannel.cs</Link>
  </Compile>
</ItemGroup>
```

Note for whoever adds more links here: `scripts/check-shared-xaml.ps1` exists because a file compiled into two assemblies must not name one of them. It only reads the installer's csproj today; extend it to this one at the same time.

- [x] **Step 3: Write the window**

Borderless, `WindowStyle=None`, `AllowsTransparency=True`, `Topmost=True`, `ShowInTaskbar=False`, `AllowDrop=True`, `Background` a nearly-transparent brush — fully transparent takes no hit-tests, so it needs about 1% alpha to be droppable while invisible.

- [x] **Step 4: Verify by hand that the catcher receives a drop**

Run the catcher alone with a hard-coded rectangle, drag a file onto it, confirm the log records the paths. This is the step that proves a Medium window can do what the notch cannot; nothing later is worth building if it fails.

- [x] **Step 5: Commit**

```bash
git add src/Plith.DropCatcher Plith.sln
git commit -m "feat(shelf): a medium-integrity window that can receive a drop"
```

---

### Task 3: Launch and lifetime

**Files:**
- Modify: `src/Plith/App.xaml.cs`, `src/Plith.Installer/Services/InstallSteps.cs` (Run-key entry)

- [x] **Step 1: Start the catcher through the shell, never as a child**

```csharp
// Started via explorer.exe, which makes Explorer the parent and gives the catcher Explorer's
// MEDIUM integrity. Launched directly, it would inherit this process's HIGH token and be exactly
// as unable to receive a drop as the window it exists to stand in for.
Process.Start(new ProcessStartInfo("explorer.exe", $"\"{catcherPath}\"") { UseShellExecute = true });
```

- [x] **Step 2: Verify the integrity level of the launched process**

Read its token integrity and assert Medium in the log at startup. If it comes up High, the launch route is wrong and every later task is dead — so it is checked here, once, loudly.

- [x] **Step 3: Commit**

---

### Task 4: The handoff

**Files:**
- Modify: `src/Plith/Views/Presentation/NotchHoverPoller.cs` (restore `DraggingOverChanged` from `efd0db8`), `src/Plith/Views/OsdHost.cs`

- [x] **Step 1: Restore the drag-approach detection**

It was removed deliberately when it had nothing to serve. `git show efd0db8^:src/Plith/Views/Presentation/NotchHoverPoller.cs` has it, including the rule that separates a drag from a press — the button must have gone down outside the notch.

- [x] **Step 2: Hand the rectangle over on drag-approach**

Hide the band window, send `Show` with the panel's screen rectangle in DIP.

- [x] **Step 3: Take it back on `Dropped` or on drag-end**

- [x] **Step 4: Verify by hand — drag a file onto the notch**

Expected in the log: the approach, the handoff, `Dropped` with the file names, the notch returning. This is the moment the feature either exists or does not.

- [x] **Step 5: Commit**

---

### Task 5: What is staged, and where it lives

**Files:**
- Create: `src/Plith/Services/Shelf/ShelfStore.cs`, `tests/Plith.Tests/ShelfStoreTests.cs`

- [x] **Step 1: Write the failing tests**

```csharp
/// <summary>Paths arrive from a lower-integrity process, so they are claims rather than facts.
/// A path that does not resolve to something on disk is dropped rather than stored.</summary>
[Fact]
public void Add_IgnoresAPathThatIsNotThere()
{
    var store = new ShelfStore();
    store.Add(["C:\\does-not-exist-" + Guid.NewGuid() + ".txt"]);
    Assert.Empty(store.Items);
}

[Fact]
public void Add_KeepsTheMostRecentFirst() { /* ... */ }

[Fact]
public void Add_IsIdempotentForTheSamePath() { /* ... */ }
```

- [x] **Steps 2-5:** run-fail, implement, run-pass, commit.

---

### Task 6: The shelf page

**Files:**
- Create: `src/Plith/Views/Widgets/ShelfWidget.xaml` / `.cs`
- Modify: `src/Plith/Views/OsdHost.cs` (`ApplyWidgetPages`)

- [ ] **Step 1: Build the page against the render harness**

`pwsh -STA -File scripts/render-widgets.ps1` — and render it in **both** themes with a tinted accent. The system page shipped unreadable because every colour judgement was made against a flat dark ground the product does not have; that harness now takes `-Theme` and `-Accent` precisely so this page does not repeat it.

- [ ] **Step 2: Install the page only when the shelf has something**, the same rule the weather page follows.

- [ ] **Steps 3-5:** accessibility names, lint, commit.

---

### Task 7: Dragging back out — measure before building

**Not planned in detail, on purpose.** Dragging an item OUT of the shelf means `DoDragDrop` from a High-integrity process to a Medium one, which is the reverse of the direction that is already known to be blocked and may well behave differently — the source initiates, and UIPI restricts what a *lower* integrity process may send to a higher one, not the other way round.

- [ ] **Step 1: Measure it.** A throwaway build that starts a drag from the notch with one hard-coded file, dropped onto an Explorer window. Log whether it lands.
- [ ] **Step 2: Write the answer into `docs/ROADMAP.md`** next to the inbound measurements, whichever way it goes.
- [ ] **Step 3: Plan the rest only then.** If it is blocked too, the catcher window has to serve as the drag SOURCE as well, which is a different design and deserves its own plan rather than a guess appended to this one.

---

## Self-Review

**Spec coverage:** The roadmap's shelf entry asks for a persistent drop target, staging, and drag-out. Tasks 1-4 deliver the drop path, 5-6 the staging and its surface, and 7 is the drag-out, deliberately left as a measurement rather than a design — the plan says so rather than pretending otherwise.

**Placeholders:** Tasks 5 and 6 carry step summaries rather than full code, and that is a real limit of this plan: they are the two tasks whose shape depends on what Tasks 1-4 actually produce. They should be expanded before they are executed, not before Task 1 is.

**Type consistency:** `DropMessage`, `DropVerb`, `DropChannel.Encode/TryDecode`, `DropChannelServer.Start/SendAsync/Received` are used with the same names and signatures in every task that mentions them.

**The one thing that could still sink it:** Task 2 Step 4. Everything here assumes a Medium-integrity topmost window can receive a drop while the notch is hidden. That is very likely — it is an ordinary window at that point — but it has not been measured, and this plan has been built on measurements everywhere else. If it fails, stop and re-plan rather than working around it.
