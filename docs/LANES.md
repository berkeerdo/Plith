# Lanes

Master: session `plith` in `C:\Projects\plith` (main checkout). Lanes read this file from that
path, never from their worktree. Skills: `parallel-lanes`, `project-docs`, `praxvon-structure`.

## Rules

| Lane | Work | Branch (base `main`) | Model | Owns | Must not touch |
|---|---|---|---|---|---|
| `engine` | R1 engine slice | `feature/engine-slice` | opus | `crates/plith-core`, `crates/plith-render`, `crates/plith-shell`, `crates/plith`, `docs/plans/active/engine-*.md` | service crates, `plith-ipc`, `plith-osd`, other docs |
| `services` | R2 services, headless | `feature/services` | sonnet | `crates/plith-audio`, `crates/plith-media`, `crates/plith-system`, `docs/plans/active/services-*.md` | engine crates, `crates/plith`, `plith-ipc`, `plith-osd`, other docs |

- Design: `docs/specs/2026-11-14-rust-rewrite-design.md`. Project rules: `CLAUDE.md`. Read the
  stack reference `~/.claude/skills/praxvon-structure/references/rust-tauri.md` before the first
  edit.
- Shared files are **add-only**: root `Cargo.toml` (`[workspace.dependencies]` lines),
  `clippy.toml` (`doc-valid-idents`). `Cargo.lock` conflicts are resolved by the master at merge.
  A new shared helper or a change to the lint set: ask the master first.
- Crate dependency rules are enforced by `node scripts/check-deps.mjs`; a lane does not edit it.
- "Done" means green output of: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`.
- Builds and test runs go through `node ~/.claude/bin/heavy-run.mjs -- <cmd>` (machine lock).
- **No script moves the mouse or types keys** on this PC. Motions are triggered by the demo driver.
- **Hardware writes need Berke's yes via the master**: changing volume, mute, the default
  endpoint, brightness or Voicemeeter parameters. Probes read by default.
- Motion is measured only in a console session with Berke present; ask the master to arrange it.
- No `rm` on variable, glob or command-substitution paths; no `bash -c`, `eval` or nested
  one-liners; multi-line commit messages via `git commit -F <file>`.
- Merge order: whichever lane is ready first; the master reviews (code review, structure check,
  commit audit) before each merge into `main`.

## State (rewritten by the master)

Updated 2026-11-14.

| Lane | Job | Branch, last commit | Last message | Waiting on |
|---|---|---|---|---|
| `engine` | Tasks 1-9 done; smoke run passed 20:23 (27 motions, 26 at 240.0 fps 0 hitches, idle 0 wakes); Task 10 next | `feature/engine-slice` @ `6922d3c` | expand 800 ms, collapse 812 ms vs Oyenza 730/660: retune after Berke's feel | Berke at the PC for Task 10 (about 20 min) |
| `services` | execute `services-r2.md` (9 tasks, approved) | `feature/services` @ `82dab67` | Tasks 1-7 done and reviewed (opus whole-branch review, fixes in); Task 8 batch sent to Berke | Berke's yes on Task 8 writes (9 items incl. media transport) |

Master open items: R3 visual direction with Berke; file engine ADRs P1 to P5 after Task 10.
