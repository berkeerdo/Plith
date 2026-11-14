# 0001 Rewrite Plith in Rust, in this repo

Date: 2026-11-14. Status: accepted 2026-11-14.

## Context
WPF 0.3.1 is far less fluid than Oyenza Studio (Rust, Direct2D, DirectComposition) on the same
PC, and its shelf needs a second process because the whole app runs at high integrity. Berke
asked for a complete redesign and a from-scratch rewrite, not a port.

## Decision
- New code is a Rust workspace (`Cargo.toml`, `crates/`) at the root of this repo.
- The WPF tree (`src/`, `tests/`, `scripts/`, `tools/`, `Plith.slnx`) is frozen as the working
  reference. Only release-blocking fixes go in. It is deleted in one commit once the Rust build
  reaches parity (ROADMAP R7).
- WPF documentation stays in place and is listed as frozen in `docs/README.md`; the WPF roadmap
  and project CLAUDE.md moved to `docs/wpf/`.

## Why the same repo
GitHub Releases is where users download Plith; issues and history stay together; the project
memory is keyed to this folder. A new repo would split all three for no technical gain.

## Consequences
- The root holds both `Plith.slnx` and `Cargo.toml` until R7.
- `scripts/structure-check.mjs` sees the WPF tree too; its oversized files are frozen in
  `structure-baseline.json`, never grown.
