# Plith

A dynamic-island notch for Windows 11, for people who care about sound and play games:
Voicemeeter and Windows audio, media, a shelf, system controls, an OSD over exclusive fullscreen.
Being rewritten from scratch in **Rust** (ADR 0001). The WPF app (0.3.1) in `src/` is the frozen
working reference until parity; touch it only for a release-blocking fix (`docs/wpf/CLAUDE-wpf.md`).

## Read first
`docs/STATUS.md` -> `docs/plans/active/` (with lanes the master also reads `docs/LANES.md`).
Design: `docs/specs/2026-11-14-rust-rewrite-design.md`. Map of docs: `docs/README.md`.

## Stack
- Rust stable, edition 2024, Cargo resolver 3, workspace in `crates/`.
- Win32 + D3D11 + Direct2D + DirectWrite + DirectComposition through the `windows` crate (one
  pinned version in `[workspace.dependencies]`). Check its API with context7 before use.
- `plith.exe` runs at medium integrity; only `plith-osd.exe` is UIAccess (ADR 0002).
- The UI thread never draws; the render thread owns every graphics object.

## Structure (skill `praxvon-structure`, `rust-tauri.md` reference)
- Limits: 500 source, 150 entry (`main.rs` composes only), 800 test, 80 per function.
- `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`.
- Pure logic lives in `plith-core` with unit tests; Windows code stays out of it.
- `node scripts/structure-check.mjs` and `cargo clippy --workspace -- -D warnings` before every
  merge. Oversized WPF files are frozen in `structure-baseline.json`.

## Measuring (the lesson the WPF app paid for)
Green builds and tests cannot see a wrong shape, a stutter or a window that is not on screen.
Before saying something works, run it and say which half you measured.
- Every motion writes a `motion` line to `%LOCALAPPDATA%\Plith\plith.log`: frames, fps, hitches,
  worst gap, draw and present ms. The bar is Oyenza on this PC: 240 fps at 240 Hz, 0 to 1 hitches.
- Motion is measured only in a **console** session. Over RDP Windows turns animation off.
- **Never move the mouse from a script** on this PC. Trigger motions with the demo driver.
- Builds and full test runs go through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.

## Conventions
- All code, comments, docs, commits in English; only user-facing strings in Turkish.
- Conventional Commits, subject max 50 chars, bullet body. No AI attribution anywhere.
- Never the em dash, in code, docs or commits (`structure-check.mjs` enforces it).
- Ask before every commit or push. Lanes commit to their own branch (skill `parallel-lanes`).
- A decision that shapes later work becomes an ADR in `docs/decisions/`.
