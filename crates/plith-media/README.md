# plith-media

Media services: SMTC sessions, seek and the data behind the output picker.

Design: `docs/specs/2026-11-14-rust-rewrite-design.md`, section 7.

## Behavior worth knowing

- `start()` blocks (about 8 s in the worst case: the manager request and the first track read,
  each capped at 2 s per operation). Call it off the UI thread.
- After a failed or timed-out read the previous title and artist can stay for a moment: the
  service keeps the last good value instead of blanking the display, and replaces it on the
  next successful read.
- `set_waker` registers a callback run after every published change, for a UI thread that
  cannot block on the channel.
