# Lanes

Master: session `plith` in `C:\Projects\plith` (main checkout). Lanes read this file from that
path, never from their worktree. Skills: `parallel-lanes`, `project-docs`, `praxvon-structure`.

## Rules

| Lane | Work | Branch (base `main`) | Model | Owns | Must not touch |
|---|---|---|---|---|---|
| `pages` | R4 Pages: UI layer, tab bar, audio, media, system pages | `feature/pages-r4` | opus | `crates/plith-core`, `crates/plith-render`, `crates/plith-shell`, `crates/plith`, `crates/plith-audio`, `crates/plith-media`, `docs/plans/active/pages-*.md` | `plith-system`, `plith-ipc`, `plith-osd`, scripts, other docs |
| `osd` | R6 OSD helper: band window spike, pipe, signing, flyout suppression | `feature/osd-r6` | opus | `crates/plith-osd`, `crates/plith-ipc`, `crates/plith-system`, new `scripts/*osd*` files, `docs/plans/active/osd-*.md` | `plith-core`, `plith-render`, `plith-shell`, `plith`, audio and media crates, other docs |

- Design: `docs/specs/2026-11-14-rust-rewrite-design.md`. Project rules: `CLAUDE.md`. Read the
  stack reference `~/.claude/skills/praxvon-structure/references/rust-tauri.md` before the first
  edit.
- Seams between the lanes: `osd` may need a change in `plith-render` (it reuses it): ask the
  master, `pages` owns that crate. `pages` may need a `plith-system` API (brightness, mic, key
  events): ask the master, `osd` owns it. Wiring the pipe sender into `crates/plith` is done
  after both merge, by the master or a follow-up job; `osd` proves its pipe with an example sender.
- Shared files are **add-only**: root `Cargo.toml` (`[workspace.dependencies]` lines),
  `clippy.toml` (`doc-valid-idents`). `Cargo.lock` conflicts are resolved by the master at merge.
  A new shared helper or a change to the lint set: ask the master first.
- Crate dependency rules are enforced by `node scripts/check-deps.mjs`; a lane does not edit it.
- "Done" means green output of: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`.
- Builds and test runs go through `node ~/.claude/bin/heavy-run.mjs -- <cmd>` (machine lock).
- **No script moves the mouse or types keys** on this PC. Motions are triggered by the demo driver.
- **Hardware writes need Berke's yes via the master**: changing volume, mute, the default
  endpoint, brightness, Voicemeeter parameters, turning flyout suppression on, installing under
  Program Files, importing a certificate. Probes read by default.
- Motion and anything over a game is measured only in a console session with Berke present; ask
  the master to arrange it, batched with other on-screen asks.
- No `rm` on variable, glob or command-substitution paths; no `bash -c`, `eval` or nested
  one-liners; multi-line commit messages via `git commit -F <file>`.
- Merge order: whichever lane is ready first; the master reviews (code review, structure check,
  commit audit) before each merge into `main`.

## State (rewritten by the master)

Updated 2026-11-16.

| Lane | Job | Branch, last commit | Last message | Waiting on |
|---|---|---|---|---|
| `pages` | R4: Tasks 1 to 3 done; Task 4 written, compiling; Task 8 writer running | `feature/pages-r4` @ `7c033a1` | 2026-11-16: status after restart; fonts bundled with `.upstream` | Berke: Q1 radius, Q2 Potato density, Q3 KAPALI, Q10 expand/collapse, Q11 footer copy (gate Task 9) |
| `osd` | R6: Task 1 done; spike code done, review fix uncommitted; then Tasks 3 to 7 | `feature/osd-r6` @ `88d9e2a` | 2026-11-16: spike ran unsigned only (plain 238 fps, band refused as expected); never on screen | Berke: batch A (install + console game run), batch B (same + flyout Hide) |

**RDP (2026-11-16):** Berke is on RDP, no game, no console. Builds and tests are allowed; no on-screen run, motion measurement or install until he is back at the PC. PC restarted 2026-11-16, lanes resumed with `lanes restore`.

R1b (`engine`) merged as `e20201c`, R2 (`services`) as `4b50e83`; both lanes stopped, worktrees
removed. Master: `lanes watch --interval 300` runs in the background; restart it each time it fires.
