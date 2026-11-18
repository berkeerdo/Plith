# plith-ipc

The named pipe protocol between the notch process and the UIAccess OSD helper.

Design: `docs/specs/2026-11-14-rust-rewrite-design.md`, section 7. Helper side:
`crates/plith-osd/README.md`.

## API

Windows only unless marked "pure".

| Item | What it is |
|---|---|
| `OsdLink::start(name) -> OsdLink` | Notch side. Starts a worker thread that connects to `name` (the pipe need not exist yet), checks the server and sends `Hello` on every connection, then forwards the latest pending message. Reconnects at most once per second. |
| `OsdLink::start_with(name, ServerTrust)` | `start` with an explicit trust rule. `ServerTrust::Installed` (the default of `start`) sends only to a server of the same user, outside an `AppContainer`, at an integrity level strictly above the client's: the installed UIAccess helper (High measured for an admin), never a same-user medium squatter. `ServerTrust::DevSameUser` also accepts the client's own level, for an unsigned dev helper and tests only. |
| `OsdLink::send(&self, Message) -> Result<(), EncodeError>` | Never blocks. Latest wins: a message not yet written is replaced. Fails only for an out of range message (percent above 100, label over 64 bytes). |
| `OsdLink::is_connected(&self) -> bool` | True from a successful connect (open, server check, `Hello`) until a write or the next connect fails. A server that dies while nothing is sent is noticed at the next send. Never blocks. |
| `OsdLink::last_error(&self) -> Option<String>` | The latest connect or write failure (`open: ...` when there is no server, `the pipe server is not trusted ...` for a server of another user, an `AppContainer` or a lower integrity level, `write: ...` for a broken pipe). Cleared by a successful connect. Never waits on the worker. |
| `drop(OsdLink)` | Replaces any unsent message with `Bye` (implies hide) and returns within about 1 s, even when the server stopped reading (the blocked write is cancelled). |
| `launch_helper(exe) -> Result<HelperProcess, String>` | Starts the helper with `ShellExecuteExW` and the `open` verb (a plain `CreateProcess` fails on a `uiAccess` exe with `ERROR_ELEVATION_REQUIRED`), hidden, waiting until the start is done. |
| `HelperProcess::pid()`, `has_exited()` | The started helper. `has_exited` is a zero-timeout wait, so it tells a dead helper from a slow one without blocking. Dropping it closes the handle and does not stop the helper. |
| `INSTALLED_HELPER` | `C:\Program Files\Plith\osd\plith-osd.exe`, where `scripts/install-osd.ps1` puts the signed helper. Only there does it get UIAccess. |
| `current_pipe_name()` | `\\.\pipe\plith-osd-<session>-<user SID>` for this process's user and Windows session (session 0 if it cannot be read). Notch and helper both use it. The SID keeps another user's session from pre-creating the name. |
| `pipe_name(session, sid)` | Pure. The same name for a given session id and user SID. |
| `OsdServer::bind(name) -> Result<OsdServer, BindError>` | Helper side. One message-mode pipe instance, remote clients refused, only this user may open it. |
| `BindError::AlreadyRunning` | This user's helper already serves the name. The helper exits 0. |
| `BindError::Squatted` | Another user's (or a less trusted) process holds the name. The helper exits 1. |
| `BindError::Os(text)` | Any other Windows error. |
| `OsdServer::serve(&self, on)` | Blocks its thread (plus one deadline thread of its own) and calls `on(ServerEvent)` per connection in this order: `Connected { pid }`, then `Message(Ok(..))` or `Message(Err(DecodeError))` per message (the admitting `Hello` first), then `Disconnected` (not sent when the server itself stops). `on` returns `Flow::Continue` or `Flow::Stop`. A fatal decode error drops that client and waits for the next. |
| Admission, `ServerEvent::Rejected(Rejection)`, `HELLO_TIMEOUT` (1 s) | A client is admitted only when its first message, within `HELLO_TIMEOUT`, is a `Hello` naming the pid Windows reports for the pipe (`GetNamedPipeClientProcessId`); `Connected { pid }` carries that kernel pid. Otherwise it is dropped and reported once as `Rejected(Timeout | ClosedBeforeHello | NotHello | PidMismatch | NoClientPid)`, with no `Connected` or `Disconnected`. A silent client cannot hold the single instance, and a bind probe no longer counts as a connection. |
| Keepalive: `KEEPALIVE` (10 s), `IDLE_TIMEOUT` (30 s), `OsdServer::set_idle_timeout` | Pure constants plus a test knob. The server drops an admitted client that sends nothing for `IDLE_TIMEOUT` (reported as `Disconnected`), so a client that sent `Hello` and went silent cannot hold the single instance. `OsdLink` sends `Message::Ping` after `KEEPALIVE` without a write, so a live notch is never dropped; `Ping` never reaches the `serve` callback. |
| `OsdServer::stopper() -> Stopper`, `Stopper::stop()` | Ends `serve` from any thread within about 1 s. |
| `Message` | Pure. `Hello { client_pid }`, `Level { meter, percent, muted, label, muted_word }`, `Hide`, `Bye`, `Ping`. |
| `Meter` | Pure. `Volume`, `Brightness`. |
| `encode`, `decode`, `EncodeError`, `DecodeError` | Pure. The wire format below. `DecodeError::is_fatal` is true for a bad magic or a newer version. |
| `MAX_MESSAGE` (144), `MAX_LABEL` (64), `PROTOCOL_VERSION` (2) | Pure. Limits and version. |
| `latest::Slot<T>` | Pure. Latest-wins hand-off: `put` replaces, `take_wait(timeout)`, `close`, `is_closed`. Used inside `OsdLink`. |
| `security::pipe_sddl(sid)` | Pure. The pipe's ACL text: this user only, with a no-write-up label. |
| `security::own_integrity_above_medium() -> Result<bool, String>` | Whether this process runs above medium integrity (for example elevated). `ServerTrust::Installed` then refuses the helper, so the notch logs it and does not start the link (step 6). `security::is_above_medium(rid)` and `MEDIUM_RID` (`0x2000`) are the pure rule. |

## Threading

- `OsdLink` owns one worker thread. `send`, `is_connected` and `last_error` never block the
  caller (the notch's UI or audio thread is fine). Only `drop` waits, bounded at about 1 s.
- `OsdServer::serve` blocks the thread that calls it; `Stopper::stop` may run on any thread.
- `launch_helper` blocks until the shell has started the process (`SEE_MASK_NOASYNC`), so do
  not call it on a thread that must stay responsive frame by frame. The Shell asks for COM on
  the calling thread (`CoInitializeEx`, apartment threaded), because a verb can be handled by a
  COM shell extension; a thread that already has COM is the right place. It may be a thread
  that ends right after the call.

## Wiring into crates/plith

Built 2026-11-17 (`docs/plans/done/wiring.md`, Record). `crates/plith/src/osd.rs` and its
`osd/` modules do the steps below in normal runs with real services only (no OSD in `--demo`,
`--idle-check` or `--fixture` runs):
- One supervisor thread (`plith-osd-supervisor`, COM apartment-threaded) launches the helper,
  ticks once a second, relaunches per `plith_core::osd::policy` (3 s apart, gives up after 5
  failures within 60 s for the rest of the session; credit only after 10 s of stable link) and
  logs `osd:` lines, each failure text once until the state changes. It never posts to the UI
  thread.
- The feed sends `Level` through a `Weak` link only when a drain changed the volume, mute or
  brightness (`plith_core::osd::changes`), not on the first reading and not while the open page
  already shows that value.
- `--flyout off|dry|hide` (default `off`): `hide` runs `FlyoutSuppressor` and the volume key hook
  only while the link is up; brightness suppression is not wired (step 5).
- An elevated notch logs that the link fails closed and starts no helper (step 6).
- Exit: suppression stops (restoring flyouts), then the link drops (`Bye`); the join is bounded
  at 1.5 s. Backstop and watchdog exits call `osd::restore_now()` first.
- A dev `plith.exe` trusts only the installed helper, so the installed build must speak the
  current protocol (a stale install fails with `open: ... cannot find the file`).

The original notes the wiring follows:

1. At start, `launch_helper(Path::new(INSTALLED_HELPER))` once, and keep the `HelperProcess`.
   A second helper on the same pipe exits by itself (`AlreadyRunning`). Still launch once: the
   second helper's bind probe connects and disconnects, which cuts a first helper that has no
   `Hello` yet down to its 2 s disconnect exit (known, open).
2. `OsdLink::start(current_pipe_name())` and keep the link for the whole session.
3. On every volume, brightness or mute change, `send(Message::Level { .. })`. `Hide` is
   optional: the helper hides 1.5 s after the last level by itself.
4. Health: if `has_exited()` is true, or `is_connected()` stays false for a few seconds, log
   `last_error()` to `plith.log`, stop hiding the native flyout (do not run
   `FlyoutSuppressor` in `Mode::Hide` while the link is down, or the person gets no OSD at
   all), and relaunch. Relaunch at most every few seconds and give up after a handful of quick
   failures in a row (a `Squatted` pipe or a render failure ends every launch the same way).
5. Brightness suppression is unverified (merge review I3): every `WmiMonitorBrightnessEvent`
   opens the 400 ms hide window, also a Quick Settings slider drag, adaptive brightness or
   battery saver dimming, and the match (five generic XAML host classes of three shell
   processes) can then hide a shell flyout under the cursor. Do not call
   `BrightnessService::set_suppression_window` in `Mode::Hide` until it is measured on a laptop
   with a built-in panel and Quick Settings open; narrow the match (size and placement, or the
   UIA name) or open the window only for key-driven changes if it misfires. Volume suppression
   is not affected.
6. Trust fails closed in two unmeasured cases (re-check N2): the notch running elevated (the
   helper is then at the same level, and `ServerTrust::Installed` refuses it), and a standard
   (non-admin) user if the UIAccess helper does not run above medium there. Detect the first
   (own integrity above medium) and log it to `plith.log` instead of retrying silently;
   measure the second on a standard account before release.
7. Labels: the helper upper-cases every label with invariant rules, as a name from Windows or
   Voicemeeter. A label the notch writes itself in Turkish (for example a fallback
   "Parlaklık") must be upper-cased by the notch with Turkish rules before it is sent, or it
   shows the wrong I.
8. On exit, drop the link (sends `Bye`); the helper fades out and exits. Without `Bye` it still
   exits 2 s after the pipe closes or the notch process ends, and 10 s after start when no
   `Hello` came.

## Wire format

Little endian. Every message starts with `b'P' b'O' version kind`, then the payload.

| Kind | Name | Payload |
|---|---|---|
| 1 | Hello | `client_pid: u32` |
| 2 | Level | `meter u8, percent u8 (0..=100), muted u8 (0 or 1), label_len u8, label UTF-8 (max 64 bytes), word_len u8, muted word UTF-8 (max 64 bytes)` |
| 3 | Hide | none |
| 4 | Bye | none |
| 5 | Ping | none. Keepalive; the server swallows it. |

Meters: 0 volume, 1 brightness. A message is at most 144 bytes. The pipe is
`\\.\pipe\plith-osd-<session_id>-<user_sid>`. A bad magic or any other version (an old helper
answers a version 2 client with a fatal version error and closes the pipe) is fatal
(`DecodeError::is_fatal`); an unknown kind is not.
