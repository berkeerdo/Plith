# Audio page clarity (Windows outputs view)

Decision: ADR 0013. Branch `feature/audio-clarity`, worktree `.claude/worktrees/audio-clarity`.
Bar: five checks green (fmt, clippy `-D warnings`, tests, `structure-check.mjs`,
`check-deps.mjs`), offscreen renders reviewed by the master. On-screen and volume-write checks
wait for Berke at the console.

## Tasks
- [x] 1. `plith-audio`: `Endpoint.form_factor: Option<u32>` from `PKEY_AudioEndpoint_FormFactor`
      (read-only, next to the names); probe example prints it.
- [x] 2. `plith-core`: `DeviceKind { Headphones, Speaker, Display }` and a pure
      `device_kind(form_factor, names)` with unit tests; `EndpointFacts.kind`, `OutputRow.kind`.
- [x] 3. `plith-core`: glyph nodes per kind (strokes in a 16 DIP box), unit tested for bounds.
- [x] 4. `plith-core`: the no-Voicemeeter layout in its own file (default row with ruler and
      `%NN`, `DİĞER ÇIKIŞLAR` rows, empty and many-output cases, footer copy); Voicemeeter
      layout untouched. Copy constants in `copy.rs`, upper-case checks updated.
- [x] 5. `Action::VolumeRuler(Rect)` resolved by `hit` to `SetVolume(u8)`; the app routes it to
      the endpoint service's `set_volume`. Brightness value shows `%70`.
- [x] 6. `crates/plith` model fills `kind`; fixtures gain a headset, a speaker and a display
      endpoint; `render_pages` renders the new states (default, muted, three outputs, none).
- [x] 7. Master: renders reviewed, five checks green, merge review.
- [ ] 8. Console (Berke): the page on screen, a ruler click changes the volume (his yes).

## Progress log
- 2026-11-17: tasks 1 to 6 on the branch (`810fd5a`, `7769594`, `48a7159`). Master render
  review asked for four fixes (audio ruler 264 to 530, value gap, no stations on other rows,
  no footer when empty), done in `48a7159`. Master ran the five checks: green, 858 tests.
  Probe on this PC: PG27AQDM form factor 9, Realtek 1, Steam Streaming 1; no headset plugged
  in, so form factor 3 and 5 are covered by unit tests only.
