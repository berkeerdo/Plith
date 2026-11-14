# Status

Updated 2026-11-14. Rewritten, not appended; old content goes to `journal/`.

## Now
- Phase: **R0 Setup**. Plith is being rewritten from scratch in Rust (ADR 0001). WPF 0.3.1 is the
  frozen working reference.
- Done: docs layout, VISION, ROADMAP, ADR 0001 and 0002, design doc
  `specs/2026-11-14-rust-rewrite-design.md`, approved by Berke 2026-11-14.
- **Next:** the Cargo workspace and CI (R0 plan), then lanes `engine`
  (R1) and `services` (R2) open while the master runs R3 visual direction with Berke.

## Open risks
- Branch `feature/shelf-one-surface` (worktree `.claude/worktrees/shelf-one-surface`) holds WPF
  shelf work the rewrite makes obsolete; it is not merged.
- Branch `feature/notch-in-loft` (worktree `C:\Projects\plith-notch-loft`) holds the Loft palette
  that R3 starts from.

## Recent decisions
- ADR 0001 Rewrite in Rust, in this repo
- ADR 0002 Medium-integrity notch, UIAccess OSD helper
