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
| `pages` | R4 merged as `b6bd801` after one merge fix round (I1 to I3 + minors, re-reviewed); lane stopped, worktree kept for the console run (Task 4, 7a, 16, 18 on-screen steps), restarted with `lanes restart pages` when Berke is at the console | `feature/pages-r4` @ `0b750d6` (merged) | 2026-11-16: fix round done, 786 tests, five checks green on the trial merge | Berke at the console; the route write session needs his separate yes |
| `osd` | R6 merged as `4aca03e` after two review rounds; lane stopped, worktree kept for Task 9 (console run over a game), restarted with `lanes restart osd` when Berke is at the console | `feature/osd-r6` @ `70c7300` (merged) | 2026-11-16: re-check fixes done (N1 serve panic, N3 to N5, M5/M6 rests) | Berke at the console; 9.8 flyout Hide plus a kill needs his separate yes |

**Approvals (Berke 2026-11-16, "önerilerini uygula", "her şeyi onaylıyorum"):** merge of each lane into main after a clean master review; device names use invariant upper case, Turkish UI strings Turkish rules.

**RDP (2026-11-16):** Berke is on RDP, no game, no console. Builds and tests are allowed; no on-screen run, motion measurement or install until he is back at the PC. PC restarted 2026-11-16, lanes resumed with `lanes restore`.

Wiring job (next, master or a subagent; both lanes merged): notes in `crates/plith-ipc/README.md` "Wiring" and the osd Record; `crates/plith` must stay single-instance before flyout Hide is wired; R4 review M8 (OSD text stack, `upper_tr` to `tr_upper`).

R1b (`engine`) merged as `e20201c`, R2 (`services`) as `4b50e83`; both lanes stopped, worktrees
removed. Master: `lanes watch --interval 300` runs in the background; restart it each time it fires.
