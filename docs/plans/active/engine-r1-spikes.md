# Engine R1 spikes: hit testing

Status: candidates built in Task 8 (`crates/plith-shell/src/hit.rs`, `hit/apply.rs`,
`hit/mask.rs`), selected with `--hit <region|split|mask>`. Default is `region` until Task 10
decides. Results below are filled in during the Task 9 smoke run and the Task 10 click tests.

## Candidates

All three are fed by `WM_APP_SHAPE` on the UI thread. The render thread posts the drawn shape
(coalesced), the handler reads the newest frame and applies it. The shell times every apply with
`Instant` and exposes `Shell::hit_stats()` (count, mean ms, max ms).

1. **region** (`--hit region`): `SetWindowRgn` on the single DirectComposition window. The region
   is `CreateRoundRectRgn` of the island rectangle outset by 1 px on every side (corner radius
   grows with it) so the anti-aliased edge is not clipped. Cheapest in code; the system clips
   both input and any drawing to the region.
2. **split** (`--hit split`): the visual window gets `WS_EX_TRANSPARENT | WS_EX_LAYERED` (clicks
   fall through, still a DComp target) and a second window `Plith.NotchInput` of the same rect
   and no content takes the region and the input. Keeps the region off the window that is drawn
   into. Open question: whether a layered, transparent window still presents DComp content.
3. **mask** (`--hit mask`): the input window is layered and updated with `UpdateLayeredWindow`
   from a surface-sized 32-bit DIB: alpha 1 (of 255) inside the rounded rectangle, alpha 0
   outside, rebuilt on every shape (not 30 Hz). This is the approach Oyenza uses. Cost grows
   with the surface area.

## Results

To be filled in Tasks 9 and 10. Edge clipped: is the anti-aliased rim of the island cut off.
Outside and inside: does a click, pen tap or touch land on the right window.

| Candidate | Count | Mean ms | Max ms | Edge clipped | Mouse outside | Mouse inside | Pen outside | Pen inside | Touch outside | Touch inside |
|-----------|-------|---------|--------|--------------|---------------|--------------|-------------|------------|---------------|--------------|
| region    | 2462  | 0.611   | 1.484  |              |               |              |             |            |               |              |
| split     |       |         |        |              |               |              |             |            |               |              |
| mask      |       |         |        |              |               |              |             |            |               |              |

## Decision

Pending (ADR proposal P2).
