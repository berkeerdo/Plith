# plith-media

Media services for Plith: System Media Transport Controls (SMTC) sessions, transport
commands, seek and the session list behind the output picker. Design:
`docs/specs/2026-11-14-rust-rewrite-design.md`, section 7.

The pure layer (`timeline`, `snapshot`, `coalesce`) holds every decision and is unit tested.
The Windows layer (`MediaService`, `cfg(windows)`) is thin: it reads SMTC on one worker thread
and publishes whole `MediaSnapshot`s.

## Public API

| Item | Purpose |
|---|---|
| `MediaService::new()` | Idle service; nothing touches WinRT until `start` |
| `start() -> Status` | Starts the worker, waits for the first reading. `Running` or `Unavailable(reason)` |
| `stop()` | Removes event registrations, joins the worker. Safe twice, also on drop |
| `status() -> Status` | A worker that ended without `stop` reports `Unavailable` |
| `snapshot() -> MediaSnapshot` | Latest reading; `MediaSnapshot::none()` before start and after stop |
| `subscribe() -> Receiver<MediaSnapshot>` | One whole snapshot per change |
| `set_waker(Box<dyn Fn() + Send + Sync>)` | Callback after every published change |
| `sessions() -> Vec<SessionInfo>` | Every SMTC session, not only the current one |
| `toggle_play_pause()`, `next()`, `previous()` | Transport writes on the current session |
| `seek(Duration)` | Seek to a position (start already accounted for) |

Writes return `false` when there is no session, the source refuses, or it does not answer
within 2 s. Pure helpers: `Timeline`, `normalize`, `position_at(timeline, playing, now)`,
`MediaSnapshot::assemble`, `Generation`.

## Consumer rules

- **Subscribe first, then snapshot.** `subscribe()` never replays, so subscribe, then call
  `snapshot()`. Drain the channel to the latest value.
- **`start()` blocks; run it off the UI thread.** Worst case about 8 s: COM and the SMTC
  manager request (each capped at 2 s) plus the first track read with its artwork.
- **`set_waker`** runs on the worker thread after every published change, outside every lock.
  It must not block. Post one message to the UI thread (for example `PostMessageW`) and read
  the channel or `snapshot()` there. A new waker replaces the old one.
- **Absent is encoded in the snapshot:** `has_session == false` means no session (and every
  other field is empty or false). Check `has_session` before showing a title. `timeline` is
  `None` for a live stream or a source without an end time, and then `can_seek` is false.
  `thumbnail` is `None` without artwork; empty `title` and `artist` mean the source gave none.
- **Interpolate position yourself** with `position_at` between events; the service publishes
  on change, not on a clock.
- After a failed or timed-out read the previous title and artist can stay for a moment: the
  last good value is kept so the display does not blank, and is replaced on the next
  successful read.

## Read-only probe

```
cargo run --package plith-media --example probe_media
```

Prints every session, the current track, its timeline and capabilities, then 15 s of change
events. It never sends a command.

## Hardware write verification

`cargo run --package plith-media --example verify_media_writes -- --media-transport --yes`
changes the playing media: play/pause is toggled twice and must end in the original state,
then the position moves +5 s (or -5 s near the end) and back. Restore guards put the original
state back on failure, panic or Ctrl+C. Without `--yes` it reads the session, prints what it
would do and exits 0 having sent nothing. Use `--yes` only after the owner approves. Nothing
runs without `--media-transport`.

## Thread model

One worker thread owns COM and every WinRT object. SMTC event handlers only post to that
thread; they never read or write from the callback. Commands and `sessions()` travel to the
worker over a channel and carry the caller's deadline, so a request that queued too long is
dropped; callers wait at most 6 s. A generation counter discards results of reads that were
overtaken by a newer event.

## Known limits and unverified on hardware

- Play/pause, track change and the transport commands have unit tests for the pure layer;
  the live behavior against real players (browser, Spotify, system player) is unverified
  beyond the probe and `--media-transport`.
- Whether a given source honors `seek` is asked of SMTC per session, never assumed.
- Artwork is passed as encoded bytes as the source supplies them (PNG or JPEG).
- The output picker data is only the session list here; device switching lives in
  `plith-audio`.
