# Status

Updated 2026-11-14. Rewritten, not appended; old content goes to `journal/`.

## Now
- Phase: **R1 and R2 in lanes, R3 next in the master.** Plith is being rewritten from scratch in
  Rust (ADR 0001). WPF 0.3.1 is the frozen working reference.
- R0 done: docs layout, ADR 0001 and 0002, design doc
  `specs/2026-11-14-rust-rewrite-design.md` (approved by Berke 2026-11-14), Cargo workspace with
  nine crates, `scripts/check-deps.mjs`, CI `.github/workflows/rust.yml` (green on 0155a6d).
- Lanes `engine` (R1, opus) and `services` (R2, sonnet) run per `LANES.md`; each writes its
  plan first and waits for the master's approval.
- **Next (master):** R3 visual direction with Berke (praxvon-direction), starting from the Loft
  palette on branch `feature/notch-in-loft`.

## Open risks
- Branch `feature/shelf-one-surface` (worktree `.claude/worktrees/shelf-one-surface`) holds WPF
  shelf work the rewrite makes obsolete; it is not merged.
- Branch `feature/notch-in-loft` (worktree `C:\Projects\plith-notch-loft`) holds the Loft palette
  that R3 starts from.

## Recent decisions
- ADR 0001 Rewrite in Rust, in this repo
- ADR 0002 Medium-integrity notch, UIAccess OSD helper
