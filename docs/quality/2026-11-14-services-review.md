# Services branch review (origin/feature/services-reworded vs main)

Scope: the five axes in the brief only. Read-only; nothing built or run.
Result: 0 BLOCKING, 10 LATER. Nothing found that must change before merge.

Paths below are relative to the branch root. Line numbers are from `git show` of the branch.

## Axis 1: service contract consistency

All five worker services (EndpointService, MicrophoneService, VoicemeeterService, MediaService,
BrightnessService) expose `new/start/stop/status/snapshot/subscribe/set_waker` with the same
names. Shared semantics that DO hold: `start` blocks and returns `Status`, never panics, is
idempotent while running; `stop` takes the thread lock first, is idempotent, publishes an
empty value afterwards; `Drop` calls `stop`; `subscribe` returns an unbounded `Receiver` of the
full value; `set_waker` replaces the callback and the callback runs outside every lock.

Divergences:

- L1 (LATER) The two hook owners are outside the contract. `VolumeKeyHook::start` and
  `FlyoutSuppressor::start` return `Result<_, String>` (not `Status`), have no
  `status/snapshot/subscribe/set_waker`, and hand out a bare `Receiver<()>`
  (`plith-system/src/volume_keys/hook.rs:79-99`) or `&Receiver<Match>`
  (`plith-system/src/flyout/hook.rs:249-252`). `stop` takes `&mut self` and returns `Result`.
  Wiring a key press into a WM_APP wake therefore needs a bridge thread that blocks on the
  receiver, or a UI timer that polls it. Give both a `set_waker` (call it after the
  `try_send`, outside the proc, from a tiny forwarder) or document the bridge now.
- L2 (LATER) `VoicemeeterService::start` does not reap a dead poll thread:
  `plith-audio/src/voicemeeter/service.rs:107-109` returns `self.status()` whenever the slot
  is `Some`, even if the thread finished. The other four use `slot_alive` and start a fresh
  worker. After a poll-thread panic, `start` returns "stopped unexpectedly" forever until the
  caller does `stop()` first. Also semantic drift: VM `Status::Unavailable` can coexist with a
  live thread (engine absent, `start` keeps polling, `service.rs:136-148`), while for the other
  four `Unavailable` means no worker. Document it or add a variant.
- L3 (LATER) Four `Status` enums with identical shape: `endpoints/service.rs:21`,
  `voicemeeter/service.rs:16` (both in plith-audio, so a consumer of one crate imports two
  different `Status` types), `plith-media/src/snapshot.rs:9`, `plith-system/src/brightness.rs:62`.
  The design doc forbids shared types from plith-core, so one per crate is fine, but inside
  plith-audio the two should be one type (mic already reuses `endpoints::Status`).
- L4 (LATER) "Empty" representation and write style differ.
  Empty value: `Option<T>` for mic and Voicemeeter, an empty/default value for endpoints, media
  (`has_session:false`) and brightness (`Vec::new()`).
  Writes: endpoints `set_volume/toggle_mute/set_default_endpoint` block on a reply up to 3 s
  (`endpoints/notify.rs` REPLY_TIMEOUT); media `toggle_play_pause/next/previous/seek/sessions`
  block up to 6 s (`plith-media/src/service.rs` ASK_CAP and `ask`); brightness `set_percent`
  and endpoints `set_pin` return at once; Voicemeeter `set_gain/toggle_mute` run the DLL call
  on the caller thread under the API gate. Only `start` documents "call it off the UI thread";
  the blocking writes do not. From the UI thread these must go through a worker or the docs
  must say so.

## Axis 2: duplication and drift

- Intended copies: `waker.rs` in all three crates is byte-identical (same md5). No drift.
- L5 (LATER) Unintended duplication that should become one piece later:
  - `slot_alive`: three identical copies (`endpoints/service.rs:41`, `media/service.rs:39`,
    `brightness/service.rs:33`), each with its own copy of the same test. `worker_status`
    exists once (endpoints) and is inlined in media and brightness.
  - `locked()` mutex helper: six copies (endpoints, microphone, voicemeeter, media, brightness
    service.rs, plus `brightness/writer.rs:27`).
  - The `start/stop/status` bodies (thread slot, ready channel, stop-takes-thread-lock-first)
    are the same 40 lines in endpoints, microphone, media and brightness.
  - `ComGuard` has DRIFTED: `endpoints/com.rs:28-53` tolerates `RPC_E_CHANGED_MODE` and does not
    uninit then; `plith-media/src/com.rs` and `brightness/service.rs:98-115` do not and would
    fail with "COM initialization failed". Harmless today (all on fresh MTA threads) but the
    three will not stay equal by hand.
  - Example scaffolding: ctrl-C handler, restore guard, `event_arrived`, `drain` exist in
    `plith-audio/examples/verify_writes/support.rs`, `plith-media/examples/verify_media_writes.rs:33-90`
    and `plith-system/examples/verify_writes.rs:48-112`, already diverging (media/system keep them
    inside a private `mod`, audio shares via `support.rs`).
  Suggested later home: a small internal crate for `slot_alive/locked/ComGuard/Status`
  scaffolding (not plith-core types in the public API).

## Axis 3: naming and layout

OK: no `mod.rs`, no file named utils/helpers/common/misc, identifiers and comments English,
`foo.rs` + `foo/bar.rs` used throughout (endpoints, microphone, voicemeeter, brightness,
flyout, volume_keys). No em dashes found in crates.

- L6 (LATER) `plith-audio/examples/verify_writes/support.rs` is a catch-all name (same smell as
  helpers). Rename by content (`restore_guard.rs`, `ctrl_c.rs`).
- L7 (LATER) Logic in examples without a test run: `plith-system/examples/verify_writes.rs`
  has `near`, `refused`, `settled_state`, `show` (lines 113-236) and no `#[test]`; its
  `[[example]] verify_system_writes` in `plith-system/Cargo.toml` also lacks `test = true`,
  while audio and media set it. Audio and media examples do carry tests for their verdict
  logic (`judge_toggles`, `seek_target`, `Restore`), but they live in the example file, so move
  to the crate if the logic is meant to stay.

## Axis 4: unsafe and FFI hygiene

Good: every `unsafe` block has a SAFETY comment (script-checked; only fn-pointer type aliases
and `extern fn` declarations lack one, which is correct). COM init/uninit are paired on the
owning thread through `ComGuard`, declared before every COM object so objects drop first
(media `run`, endpoints `run`, mic, brightness worker and `Listener`). Callbacks are removed
on the worker before the manager/enumerator is released (media `run` end, endpoints `run` end).
Hooks are unhooked on the hook thread in `Installed::drop` for every exit path including a
panic; `State::drop` restores hidden windows with `SW_SHOWNA`. WMI VARIANTs are cleared on drop;
DDC handles are destroyed on drop.

- L8 (LATER) No `catch_unwind` in the two `extern "system"` hook procs
  (`flyout/hook.rs:185`, `volume_keys/hook.rs:39`). A panic there aborts the whole process
  (non-unwinding ABI), and per `flyout/registry.rs` docs a hidden shell flyout does not come
  back when the process dies, so the failure mode is "volume flyout gone until Explorer
  restarts". Today only OOM can panic in them, so low odds, but a `catch_unwind` wrapper is two
  lines. Related: `hook_thread.rs` `pump()` ends silently on a `GetMessageW` error (-1), which
  drops the guard and removes the hooks with `status` unaware; `HookThread::drop` swallows a
  failed `stop` (thread and hooks stay alive after the handle is gone).
- L9 (LATER) The brightness event `Listener` (`brightness/service.rs:~118-135`) returns silently
  if COM or `wmi::listen` fails, so the service stays `Running` and never reports external
  brightness changes. No signal reaches the consumer.

## Axis 5: wiring into crates/plith (UI thread + waker posting WM_APP)

No deadlock or lock-order problem found: `publish_with` releases state and subscriber locks
before the waker, the waker is documented as non-blocking, a single publisher per service
(worker, then `stop()` after the join) keeps subscriber order. Points that make wiring awkward:

- L10 (LATER) Collected wiring notes:
  - `stop()`/`Drop` joins the worker and publishes the empty value on the CALLING thread, so
    the waker also fires there (media docs say "on the worker thread",
    `media/service.rs` set_waker doc). On shutdown from the UI thread, join can take up to
    ~8 s for media (blocked WinRT read) or a DDC write in flight for brightness; the waker may
    target an HWND already gone (PostMessage fails harmlessly, keep the closure tolerant).
    Call `stop` off the UI thread or accept the wait.
  - `start` waits unbounded in `ready_rx.recv()` for endpoints, microphone and media while
    holding the thread lock (`endpoints/service.rs:~299`, `media/service.rs:~360`); only
    brightness has a cap (READY_CAP 15 s). A wedged audio service hangs `start`, and then
    `stop()`/`Drop` block on the same lock, so app exit hangs. Add a timeout like brightness.
  - `subscribe()` then `snapshot()` is the only race-free order (a subscriber added after
    `start` never sees the first value); not documented. Unbounded channels are only pruned
    when a send fails, so a consumer that uses waker + `snapshot()` must not also subscribe.
  - Status changes do not wake: a worker death, Voicemeeter `Status` flips, and `levels()`
    (updated every 100 ms with no waker) need a UI timer or polling.
  - `status()` returns `Unavailable("starting or stopping")` when the lock is busy
    (`try_lock`), indistinguishable from failure; a `Starting` variant would help the UI.
