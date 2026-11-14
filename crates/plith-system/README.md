# plith-system

System services for Plith: display brightness (built-in panels through WMI, external
monitors through DDC/CI) and suppression of the native Windows volume flyout. Design:
`docs/specs/2026-11-14-rust-rewrite-design.md`, section 7. Windows layer is `cfg(windows)`.

## Public API

| Item | Purpose |
|---|---|
| `BrightnessService` | `new()`, `start() -> Status`, `stop()`, `status()`, `snapshot() -> Vec<DeviceLevel>`, `subscribe() -> Receiver<Vec<DeviceLevel>>`, `set_waker(..)` |
| `BrightnessService::set_percent(u8)` | The only hardware write. Clamped to 100, returns at once (`false` when not running); the newest value wins, one write in flight |
| `FlyoutSuppressor::start(Mode)` | Three WinEvent hooks on their own thread. `Mode::Hide` hides the shell volume flyout, `Mode::DryRun` only reports |
| `FlyoutSuppressor` methods | `matches() -> &Receiver<Match>`, `suppression_window()`, `open_window()`, `stop()` (restores hidden windows, safe twice, also on drop) |
| `VolumeKeyHook::start(Option<Arc<SuppressionWindow>>)` | Low level keyboard hook, returns the hook and a `Receiver<()>` with one signal per volume key down. Keys are never swallowed |
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
- Brightness flyout suppression is NOT covered. The native brightness flyout is a deferred
  gap: the consumer should wire `WmiMonitorBrightnessEvent` (the brightness service already
  listens for it on built-in panels) to the suppressor's `open_window`.

## Read-only probes

```
cargo run --package plith-system --example probe_brightness
cargo run --package plith-system --example probe_flyout
```

`probe_brightness` lists displays and prints 5 s of change events and never calls
`set_percent`. `probe_flyout` is a dry run: it hooks for 10 s, logs what it would hide when a
volume key is pressed by hand, hides nothing and sends no input.

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
- A run killed while `Mode::Hide` is active can leave a window hidden; restart Explorer.
