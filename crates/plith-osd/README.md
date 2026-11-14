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
