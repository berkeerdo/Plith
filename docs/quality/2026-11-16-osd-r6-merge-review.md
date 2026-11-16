# Merge review: feature/osd-r6 (fresh read)

Reviewed: `git diff main...feature/osd-r6` (79 files). I read every file in `plith-ipc/src`, the
security-relevant parts of `plith-osd/src` (main, app, args, band, watch, log, model, render,
render/stack, render/motion), `plith-system` (guard, hook_thread, flyout, flyout/hook,
hook/state, hook/query, journal, registry, volume_keys, volume_keys/hook, brightness/listener,
service, wmi, panel_event) and `scripts/install-osd.ps1` and `build-osd.ps1`, plus the plan
Record and the ADR 0009 draft. Nothing was built or run. Line numbers refer to the branch.

## Verdict

**Mergeable, no Critical findings.** The pipe hardening holds up. These are all present:
protected DACL for the user SID only, an explicit medium no-write-up label,
`PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE` with one instance, SQOS
identification on the client, a server identity check before the first byte, and a strict
bounded decoder whose only effect is drawing. The hook procs are contained correctly.

Four Important items should be fixed or tracked before the wiring job turns Hide mode and the
helper on in `crates/plith`:
- I1: install signer and hash pinning.
- I2: the Hello pid comes from the message.
- I3: brightness suppression is broad and unmeasured.
- I4: the journal is shared between instances.

Nothing in `crates/plith` consumes this code yet, so none of these is live today.

What I checked and found sound:
- Low integrity and AppContainer processes cannot open the pipe. The `ML;;NW;;;ME` label stops
  the first; the DACL has no package SIDs, which stops the second.
- Another user cannot open the pipe (the DACL), and the client refuses another user's server
  (`identity.rs:51-53`).
- The server cannot impersonate the client (`SECURITY_IDENTIFICATION`, `identity.rs:26`).
- Oversized or garbage messages are drained or rejected (`server.rs:295-298`, `message.rs:168-235`).
- `Stopper` cancel semantics are race-free: the flag is set before registration, or the cancel
  loop covers the gap.
- `ShowWindowAsync` from the render thread cannot deadlock against the UI thread's blocking send.
- Panic containment is correct. Every `extern "system"` proc either runs its body in
  `catch_unwind` (flyout `state.rs:150`, keys `hook.rs:92`) or cannot panic (`band.rs:116`).
  The thread locals read outside the guard (`DISABLED`, `HELD`) have no destructor, so `with`
  cannot panic during teardown. `panic` stays at the default `unwind` (workspace
  `Cargo.toml` has no `panic = "abort"`), so the containment is real.
- Project rules pass. No `mod.rs`. Largest source file is 413 lines (`brightness/service.rs`),
  largest test is 384, `main.rs` is 75. The longest function is `app::run` at 64 lines. No em
  dash in the diff. No Windows code in `plith-core`. The only non-ASCII text is user-facing
  Turkish (`MUTED_WORD`, `upper_tr`) and test data.

## Important

### I1. The install gate does not pin the signer or the binary
`scripts/install-osd.ps1:62-63, 72-74`.

**What happens:** `Get-AuthenticodeSignature ... Status -eq 'Valid'` accepts any signer that
chains to a trusted root. The source is `target\release\plith-osd.exe`, which the medium user
can write.

**Failure scenario:**
1. The developer key (`CN=Plith Self-Signed`) sits in `CurrentUser\My` and its root is trusted
   machine-wide (UIAccess needs that). Any medium process of the same user can therefore sign
   arbitrary code with it.
2. Same-user malware swaps `target\release\plith-osd.exe` for its own exe with
   `uiAccess='true'`, signed with that key.
3. The developer then runs `install-osd.ps1 -Yes` under sudo.
4. The malware lands in a secure location. The notch launches it, and it runs as UIAccess at
   High integrity: UIPI is bypassed, so it can drive elevated windows on the desktop. That is
   an elevation path.

The script also never checks the ACL of `C:\Program Files\Plith` and `\osd`. If an earlier
install or quarantine left a writable ACL there, any non-admin process can replace the exe
later, because UIAccess checks the path, not the ACL.

**Fix:**
- Require `$sig.SignerCertificate.Thumbprint -eq <expected>`.
- Have `build-osd.ps1` print the SHA-256 and make install show it and ask for it (or compare
  it against an admin-only record).
- After the move, assert that no non-admin SID has write, delete or WRITE_DAC rights on
  `Plith`, `Plith\osd` and the exe.
- Longer term, keep the signing key out of reach of medium processes (a password-protected PFX
  used only during an elevated sign step), or keep the root trusted only for this signer's
  purpose.

### I2. The helper trusts `client_pid` from `Hello`
`plith-osd/src/app.rs:254-261`, `watch.rs:20-39`.

**What happens:** the pid the helper watches comes from message bytes, not from the kernel. A
new detached thread is spawned whenever a `Hello` names a pid other than the last one.

**Failure scenarios** (any same-user medium client can do this):
- It sends `Hello` with alternating pids of long-lived processes. Each one leaves a thread
  blocked forever in `WaitForSingleObject(INFINITE)` inside a High-integrity process, so thread
  and handle exhaustion is unbounded.
- It names a pid that cannot be opened: `ClientGone` arrives at once and the helper exits.
- Pid reuse makes it watch the wrong process.

This is the one place where untrusted input steers an OS call (`OpenProcess`) in the elevated
helper.

**Fix:**
- After `ConnectNamedPipe`, read `GetNamedPipeClientProcessId` in `plith-ipc` and report it
  with `ServerEvent::Connected(pid)`.
- Watch that pid only: one watcher per connection, waiting on the process handle plus a cancel
  event so the old watcher ends.
- Treat the pid in `Hello` as informational, or reject a mismatch.

### I3. Brightness events open the suppression window too broadly
`plith-system/src/brightness/listener.rs:69-77` and the filter in `flyout.rs:28-50, 88-95`.

**What happens:** every `WmiMonitorBrightnessEvent` opens the 400 ms window. That includes the
Quick Settings brightness slider while it is dragged, adaptive or ambient brightness changes,
and battery saver dimming. While the window is open, any shell window with one of five generic
XAML host classes, owned by Explorer, ShellExperienceHost or WindowsInputExperience, in bands
0x4/0x10/0x11/0x12, is hidden on SHOW, CREATE or LOCATIONCHANGE.

**Failure scenario:** a laptop user drags the Quick Settings brightness slider. The events keep
the window open, and the Quick Settings host (or another XAML shell flyout) is hidden under the
cursor, then stays hidden until the shell shows it again.

The Record says brightness suppression was not measured (this PC has no built-in panel).

**Fix:**
- Before enabling it, measure on a laptop with Quick Settings open.
- Narrow the match for brightness (window size and placement, or UIA name of the brightness
  flyout), or open the window only for key-driven changes (`VK_BRIGHTNESS`-like scancodes from
  the LL hook) instead of every WMI event.

### I4. The journal is shared by every instance, and only a handoff note keeps it single
`flyout/hook.rs:120-123`, `hook/query.rs:150-165`, `journal.rs:88-90`.

**What happens:** every `Mode::Hide` suppressor uses the fixed
`%LOCALAPPDATA%\Plith\flyout-hidden.txt` and the fixed temp `flyout-hidden.tmp`.

**Failure scenario:**
1. Instance A is running and has hidden the flyout.
2. Instance B starts. This can be a second notch, or `verify_writes --flyout --yes` run while
   the app runs. B "restores" A's windows and clears the file.
3. A is then killed. Its hidden flyout is no longer journaled and stays hidden until Explorer
   restarts.

Two concurrent writers also race on the shared `.tmp`. The rename error is ignored, so an entry
can be lost.

The Record notes that `plith.exe` must stay single-instance, but nothing in the crate enforces
it.

**Fix:** either have `FlyoutSuppressor::start` in Hide mode take a named mutex
(`Local\plith-flyout-<sid>`) and refuse or fall back to DryRun when it is held, or journal per
process (`flyout-hidden-<pid>.txt`) and sweep only the journals whose pid is dead at start.

## Minor

**M1. The hide is posted before it is journaled** (`flyout/registry.rs:111-123`,
`HiddenRegistry::hide` at 53-64).
- Scenario: `ShowWindowAsync(SW_HIDE)` is posted, then the journal is written. A kill between
  the two (the write is a create, write and rename) leaves a hidden flyout that is not
  journaled.
- Fix: write the journal with the handle first, post the hide, and remove the entry if the
  post fails.

**M2. The client trusts any same-user medium server** (`identity.rs:51-53`, used at
`client.rs:290-297`).
- What happens: the helper is measured at High integrity, but the check accepts integrity
  greater than or equal to the client's own level.
- Scenario: a same-user medium process grabs `plith-osd-<session>` in the helper's 2 s exit gap
  or before launch. It receives every level and label, and the real helper exits with
  `Squatted`, so the person gets no OSD.
- Fix: once wired, require High (or check that the server image path equals `INSTALLED_HELPER`
  with `QueryFullProcessImageNameW`). Keep medium allowed only behind a dev flag.

**M3. The pipe name holds only the session id** (`pipe_name.rs:5-7`, `launch.rs:28-36`).
- What happens: the pipe namespace is global across sessions.
- Scenario: another user on another session pre-creates `plith-osd-1`. This user's helper
  always fails with `Squatted`, which is a permanent denial of the OSD.
- Fix: add the user SID (or a hash of it) to the name.

**M4. A server-thread panic leaves a zombie helper** (`app.rs:80-92, 278-285`).
- What happens: if `serve` panics, nobody sends `Event::Quit`. The UI thread keeps its sender,
  the render thread waits with no deadline after `Hello`, and the pipe handle is gone.
- Scenario: the notch sees `has_exited() == false`, cannot connect and never relaunches. A High
  process lingers with no OSD.
- Fix: a drop guard in the server closure that sends `Quit`, like `ExitPost` does for the render
  thread.

**M5. Logging in a High-integrity process** (`log.rs:51-58`, `main.rs:36`, `app.rs:55-66`).
- What happens:
  - The log folder comes from the `LOCALAPPDATA` environment variable, which the launcher
    controls.
  - `%LOCALAPPDATA%\Plith` is writable at medium, so a junction there redirects where the High
    process creates `plith-osd.log`.
  - The `--pipe` argument is logged verbatim, so a newline in it injects log lines.
  - The log has no size cap (already known), and every message adds a line.
- Fix: use `SHGetKnownFolderPath`, refuse a reparse-point folder, log the pipe with `{:?}`, and
  cap or rotate the log.

**M6. One client can hold the single pipe instance forever** (`server.rs:180-190`).
- Scenario: a same-user client sends `Hello` and then stays connected and silent. The real notch
  gets `PIPE_BUSY` forever.
- Fix: after a connect, require a `Hello` within about 1 s. Optionally verify the client pid
  against the launcher (see I2).

**M7. The device-loss rebuild is unmeasured and has a single shot** (`render.rs:362-387`,
`342-359`, `render/stack.rs`).
- What happens: the old DComp target is dropped and a new one is created for the same HWND
  straight away. If the release is deferred, `CreateTargetForHwnd` can fail with
  `DCOMPOSITION_ERROR_WINDOW_ALREADY_COMPOSED`. One failed rebuild ends the helper, and recovery
  then depends on the notch's relaunch.
- Fix: retry the rebuild after a short delay, and measure it (forced TDR) as the Record already
  lists.

**M8. Click-through is unverified** (`band.rs:161-167`).
- What happens: `WS_EX_TRANSPARENT` without `WS_EX_LAYERED` is not guaranteed to make a
  top-level window transparent to hit testing. While the OSD shows (about 1.5 s) it may take
  clicks at the top centre of a game.
- Fix: measure it (`WindowFromPoint` under the OSD while shown). If it does take clicks, add
  `WS_EX_LAYERED`, which works with a DComp swap chain on Windows 8 and later. Low confidence;
  verify before changing.

## Not findings (checked)
- The bind probe in `owner_of_taken_name` connects to a live idle helper and arms its 2 s exit.
  This is already listed in the Record's open minors.
- `HookThread::stop` blocks on join if the hook thread is wedged inside a callback. The
  callbacks do only short local work and one small file write, so this is acceptable.
- `OsdLink` drop detaches a stuck worker after about 1 s, which is bounded and documented.
- The Turkish upper case for labels is user-facing and Berke's call per the Record.
