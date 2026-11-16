# plith-system

System services for Plith: display brightness (built-in panels through WMI, external
monitors through DDC/CI) and suppression of the native Windows volume flyout. Design:
`docs/specs/2026-11-14-rust-rewrite-design.md`, section 7. Windows layer is `cfg(windows)`.

## Public API

| Item | Purpose |
|---|---|
| `BrightnessService` | `new()`, `start() -> Status`, `stop()`, `status()`, `snapshot() -> Vec<DeviceLevel>`, `subscribe() -> Receiver<Vec<DeviceLevel>>`, `set_waker(..)` |
| `BrightnessService::set_suppression_window(Option<Arc<SuppressionWindow>>)` | Wires the flyout suppressor's window: every `WmiMonitorBrightnessEvent` (built-in panels only) opens it before the new level is published |
| `BrightnessService::listener_error() -> Option<String>` | Set when the panel listener could not start (COM, WMI connection or subscription) or its event stream broke; the waker is called once when it is recorded. The service stays `Running` |
| `BrightnessService::set_percent(u8)` | The only hardware write. Clamped to 100, returns at once (`false` when not running); the newest value wins, one write in flight |
| `FlyoutSuppressor::start(Mode)` | Three WinEvent hooks on their own thread. `Mode::Hide` hides the shell volume flyout and journals to `default_journal()`, `Mode::DryRun` only reports and uses no journal |
| `FlyoutSuppressor::start_with_journal(Mode, Option<PathBuf>)` | Same with an explicit journal file (tests, tools) |
| `FlyoutSuppressor` methods | `matches() -> &Receiver<Match>`, `suppression_window()`, `open_window()`, `restore_handle()`, `failure()`, `stop()` (restores hidden windows, safe twice, also on drop) |
| `RestoreHandle` | `Clone + Send + Sync`. `restore_now()` shows every hidden window (`ShowWindowAsync(SW_SHOWNA)`, never blocks on the shell), empties the shared set and clears the journal; it does not stop later hides (a hide before exit is journaled); idempotent, callable from any thread, meant for paths that skip `Drop` (backstop, watchdog before `process::exit`). `pending()` counts what is owed |
| Journal | `%LOCALAPPDATA%\Plith\flyout-hidden.txt`, a `boot <unix seconds>` line, then one decimal handle per line (`flyout::journal::format`/`parse`). Rewritten through a temp file and a rename on the hook thread after each hide, cleared on each restore. At start, a journal from another boot (stamp off by more than 120 s) or without a stamp is discarded; otherwise each handle that is still a hidden window with the facts the hide path requires (class, process, band: `flyout::is_orphan`) is shown. The file is cleared either way: this covers a kill no code can catch |
| `VolumeKeyHook::start(Option<Arc<SuppressionWindow>>)` | Low level keyboard hook, returns the hook and a `Receiver<VolumeKey>` with one signal per volume key press (auto-repeat collapsed, the suppression window stays open for the whole hold). Keys are never swallowed |
| `VolumeKeyHook` methods | `failure()`, `stop()` (safe twice, also on drop) |
| `failure() -> Option<String>` | On both owners: the text of a panic contained in the hook proc, or the `GetMessageW` error that ended the hook thread early. After a panic the flyout callback disables itself and restores every hidden window; the keyboard proc disables itself and only passes keys on |
| `VolumeKey`, `HeldKeys` | `Mute`, `Down`, `Up`; `HeldKeys::new()` (`const`) is the per-key hold state `handle_key_event` uses to collapse auto-repeat |
| `guard::contain` | Runs a closure and stops a panic there (`false` when it panicked); `contain_text` returns the panic text |
| Pure helpers | `step_level`, `CoalescingWriter`, `keep_answering`, `should_hide`, `SuppressionWindow` (400 ms), `HiddenRegistry`, `handle_key_event`, WQL text in `brightness::wql` |

`Status` is `Running` or `Unavailable(reason)`. A machine with no controllable display is
`Running` with an empty snapshot.

## Consumer rules

- **Subscribe first, then snapshot.** `subscribe()` never replays; subscribe, then call
  `snapshot()`, and drain the channel to the latest value.
- **`start()` blocks; run it off the UI thread.** `BrightnessService::start` waits for
  discovery, capped at 15 s (typically well under a second; each DDC/CI read takes tens of
  ms). On timeout it returns `Unavailable` and retires the run so a late discovery cannot
  publish. The flyout and key hooks start fast but also belong off the UI thread.
- **`set_waker`** runs on the publishing thread after every distinct change, outside every
  lock, and must not block. Post one message to the UI thread (for example `PostMessageW`)
  and read the channel or `snapshot()` there. A new waker replaces the old one.
- **Absent is an empty value:** no controllable display means an empty `Vec<DeviceLevel>`,
  not zero percent. Show nothing rather than a 0% bar.
- **Wire the pair:** pass `suppressor.suppression_window()` to `VolumeKeyHook::start` so a
  volume key opens the window inline, before the hide filters run.
- **Wire the brightness pair the same way:** `service.set_suppression_window(Some(
  suppressor.suppression_window()))` after both start. Check `listener_error()` (on the waker
  or after `start`) so a dead listener is not mistaken for "no external changes". Unverified
  on hardware here: this PC is a desktop with DDC/CI monitors only, and WMI brightness events
  fire only for built-in panels; only the pure decision is unit tested.

## Read-only probes

```
cargo run --package plith-system --example probe_brightness
cargo run --package plith-system --example probe_flyout
cargo run --package plith-system --example probe_flyout -- --brightness
```

`probe_brightness` lists displays and prints 5 s of change events and never calls
`set_percent`. `probe_flyout` is a dry run: it hooks for 10 s, logs what it would hide when a
volume key is pressed by hand, hides nothing and sends no input. With `--brightness` it
wires the brightness service to a dry run suppressor for 10 s and prints matches and the
listener error; it never calls `set_percent`.

## Hardware write verification

`cargo run --package plith-system --example verify_system_writes` changes the machine and
restores it. Without `--yes` it prints what it would change, installs no hook and exits 0
having written nothing. Use `--yes` only after the owner approves. Sections (none selected
does nothing): `--brightness` (all displays to the first level +5, re-read from hardware with
a fresh service, then restored; skipped when displays sit at different levels) and `--flyout`
(Hide mode for 20 s while a volume key is pressed by hand, then every hidden window is
checked visible again; press the opposite key afterwards).

## Thread model

`BrightnessService` has one worker thread that owns the WMI and DDC handles; writes run
there through the coalescing writer, and event callbacks only post to it. The flyout
suppressor and the keyboard hook each own a dedicated message thread, because WinEvent and
low level hooks need a pump. The keyboard proc only classifies a key and does a non-blocking
send, with no allocation, lock or I/O inline, and always calls the next hook.

## Known limits and unverified on hardware

- WMI read and write VARIANT types and the WMI brightness event listener are unverified on a
  laptop with a built-in panel; the desktop development machine has none.
- DDC/CI write is unverified beyond the 56 ms measured per write and the read path; refused
  displays are reported per device by the verify example.
- The flyout host class, process and Z-band filters are unverified on this Windows build
  (10.0.26300); the band export is undocumented and an unknown band is allowed.
- A run killed while `Mode::Hide` is active leaves the window hidden until Plith starts again
  in `Mode::Hide` (the journal restore) or Explorer restarts. The journal is not fsynced: a
  power loss right after a hide can lose the last entry.
- Panic containment and the journal restore are unit tested in the pure layer; neither the
  contained panic in a live hook nor `Mode::Hide` against the real shell was run here.
