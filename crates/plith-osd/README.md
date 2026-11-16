# plith-osd

The UIAccess helper that draws the volume, brightness and mute OSD over exclusive fullscreen games.

Design: `docs/specs/2026-11-14-rust-rewrite-design.md`, section 7.

## Build, sign, install

`PLITH_OSD_UIACCESS=1` at build time embeds `uiAccess='true'`; unset embeds `'false'`
(dev runs and `cargo test`). UIAccess only takes effect when the exe is signed and runs from a
trusted folder such as Program Files.

- `scripts/build-osd.ps1`: release build with UIAccess, signs with the `CN=Plith Self-Signed`
  certificate (fails if it is missing), verifies the signature. Output `target\release\plith-osd.exe`.
- `scripts/install-osd.ps1 [-Yes] [-Uninstall]`: copies to `C:\Program Files\Plith\osd\`.
  Without `-Yes` it only prints what it would do. Run with `-Yes` under `sudo`.

## Run

- `plith-osd [--pipe NAME]`: the OSD. Serves `NAME` (default: this session's pipe), draws the
  300 x 40 DIP seal at the top centre of the primary monitor. It exits (code 0) after `Bye`
  once the fade-out ends (at most 0.5 s), 2 s after the client disconnects or its process ends
  with no new `Hello`, 10 s after start when no `Hello` came, or at once when another helper
  already serves the pipe. A lost GPU device (driver reset) is rebuilt on hardware, then WARP.
  The window is created in the UIAccess band (`CreateWindowInBand`); an unsigned dev build is
  refused that and falls back to a plain topmost window (the log says which one it got).
- `plith-osd --spike [inband|setband|plain] [--seconds N]`: the Task 2 band spike.
- Dev run: `cargo run --package plith-osd -- --pipe \\.\pipe\plith-osd-dev`, then
  `cargo run --package plith-ipc --example send_osd -- --pipe \\.\pipe\plith-osd-dev --dev`
  (`--dev`: the unsigned helper runs at the sender's own level, which the default trust
  refuses).

Threads: the UI thread owns the window and draws nothing; the server thread hands each message
to the render thread over a bounded channel with a blocking send (the render thread drains it
quickly, so no message is ever dropped); the render thread owns the device, the swap chain,
the text formats and the model (`src/model.rs`, pure and unit tested). It sleeps until a message
or the next deadline (hide 1.5 s after the last level, exit) and paces motion on the compositor
clock. The log is `%LOCALAPPDATA%\Plith\plith-osd.log`: one `osd msg:` line per message and one
`motion osd-show` / `motion osd-hide` line per motion.

Display changes: the window procedure handles `WM_DISPLAYCHANGE` and `WM_DPICHANGED`. The UI
thread re-reads the primary monitor's rectangle and DPI, moves the window to the new top centre
(`SetWindowPos`, no activation, no z-order change) and sends the render thread a relayout; the
render thread rebuilds its graphics objects at the new size and scale (the device-loss path)
and repaints if the OSD is showing. The log gets one `osd relayout: rect=.. scale=..` line.

## Fonts

The design asks for Martian Mono (labels and numbers). It is not bundled yet: DirectWrite is
asked for "Martian Mono" and the helper falls back to Consolas when it is not installed (the
`osd render:` log line names the family in use). Bundling the font is a later job.
