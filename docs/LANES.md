# Lanes

Master: session `plith` in `C:\Projects\plith` (main checkout). Lanes read this file from that
path, never from their worktree. Skills: `parallel-lanes`, `project-docs`, `praxvon-structure`.

## Rules

| Lane | Work | Branch (base `main`) | Model | Owns | Must not touch |
|---|---|---|---|---|---|
| `engine` | R1b engine under load | `feature/engine-r1b` | opus | `crates/plith-core`, `crates/plith-render`, `crates/plith-shell`, `crates/plith`, `docs/plans/active/engine-*.md` | service crates, `plith-ipc`, `plith-osd`, other docs |

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
| `engine` | R1b: missed vblanks with a game behind, collapse and page durations, topmost IME skip, drop split and mask hit modes, deferred minors | `feature/engine-r1b` @ `main` | R1 merged as `1aabe4a` | writes the R1b plan first |

R2 (`services`) merged as `4b50e83`; lane stopped, worktree removed.
Master open items: R3 visual direction with Berke; delete merged branches `feature/engine-slice`,
`feature/engine-slice-reworded`, `feature/services-reworded` after Berke's yes.
