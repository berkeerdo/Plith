# plith-audio

Audio services for Plith: the Voicemeeter Remote API, Windows Core Audio render endpoints
(with pinning) and the default microphone. Headless: no UI, no drawing. Design:
`docs/specs/2026-11-14-rust-rewrite-design.md`, section 7. Windows only (services are `cfg(windows)`).

## Public API

All three services share the same lifecycle. `Status` is `Running` or `Unavailable(reason)`.

| Service | Module | Snapshot type | Writes (guard) |
|---|---|---|---|
| `EndpointService` | `endpoints` | `EndpointState` | `set_volume(f32)` clamps to 0..=1, NaN refused; `toggle_mute()` reads then writes the opposite; `set_default_endpoint(&str)` Console and Multimedia roles only, blank id refused; `set_pin(Option<String>)` |
| `MicrophoneService` | `microphone` | `Option<MicSnapshot>` | `toggle_mute()` returns the new state or `None` |
| `VoicemeeterService` | `voicemeeter` | `Option<VmSnapshot>` | `set_gain(Rail, index, db)` clamped to -60..=12 dB, index checked against the edition; `toggle_mute(Rail, index)` |

Common calls on each: `new()`, `start() -> Status`, `stop()` (safe twice, also on drop),
`status()`, `snapshot()`, `subscribe() -> Receiver<_>`, `set_waker(Box<dyn Fn() + Send + Sync>)`.
`VoicemeeterService` adds `levels() -> VmLevels` (read only peak levels, polled). Pure helpers
(`resolve_target`, `sanitize_volume`, `clamp_gain`, `level_to_db`, `param_name`) are public and unit tested.

## Consumer rules

- **Subscribe first, then snapshot.** `subscribe()` never replays the current value, so
  subscribe, take `snapshot()`, then apply what arrives. Each message is a whole state; drain
  the channel to the latest value.
- **`start()` blocks; run it off the UI thread.** Endpoints and microphone wait for COM and the
  first read (normally well under a second). Voicemeeter waits for the DLL load and login
  (also short). Every failure comes back as `Unavailable`, nothing panics.
- **`set_waker`** registers one callback, run on the publishing thread after every distinct
  change, outside every lock. It must not block: post a single message to the UI thread
  (for example `PostMessageW` with a custom message) and read the channel or `snapshot()` there.
  A new waker replaces the old one.
- **Absent is encoded, never defaulted.** Endpoints: an empty `EndpointState` (`default_id`
  `None`, no `active`, `attached` `None`). Microphone: `None` means no capture device and must
  not be shown as unmuted. Voicemeeter: `None` means the engine is not installed, not running
  or went away (a `None` is also published on that transition). Writes return `false` or `None`
  when nothing is attached.
- Voicemeeter login code 1 (API in, application not launched) returns
  `Unavailable("Voicemeeter is not running")` and keeps polling; `Running` follows once the
  first snapshot is built.

## Read-only probes

Safe to run any time; they never write.

```
cargo run --package plith-audio --example probe_endpoints
cargo run --package plith-audio --example probe_microphone
cargo run --package plith-audio --example probe_voicemeeter
```

## Hardware write verification

`cargo run --package plith-audio --example verify_audio_writes` changes the machine and
restores it through drop guards. Without `--yes` it only prints what it would change and
exits 0 having written nothing. Use `--yes` only after the owner approves. Sections (none
selected does nothing): `--endpoint-volume`, `--endpoint-mute`, `--mic-mute`,
`--default-endpoint --to <id substring>`, `--voicemeeter-gain`. Each prints BEFORE,
AFTER-CHANGE, AFTER-RESTORE and PASS or FAIL.

## Thread model

One service thread owns every COM attach and detach (endpoints, microphone) or the DLL poll
(Voicemeeter). Handles only read shared state under short locks and post messages to that
thread; writes are executed there and answered over a reply channel. Notification callbacks
only post to the thread, they never call COM themselves. `stop()` joins the thread, which
unregisters its own callbacks.

## Known limits and unverified on hardware

- Voicemeeter live path: login, the poll loop and writes against a running engine have unit
  tests for the pure layer only; treat the live path as unverified until `--voicemeeter-gain` passes on the target machine.
- `GetLevel` channel map: `level_channel` and `level_type` follow the Remote API
  documentation per edition; the mapping of strips and buses to left and right pairs is not
  confirmed against real meters.
- Live volume callbacks: endpoint and microphone change notifications are exercised by
  hand with the probes, not by an automated hardware test.
- Pinning falls back to the Windows default while the pinned endpoint is inactive.
- The microphone follows the Communications capture role; an app muting its own stream does
  not show as muted.
