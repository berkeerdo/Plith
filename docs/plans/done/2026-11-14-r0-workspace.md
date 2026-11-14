# R0 Workspace Implementation Plan

> A ticked box means the step was taken, not that nothing deviated.
> Deviations: `clippy.toml` gained `doc-valid-idents` for domain names; `plith-osd` may also use
> `plith-core`; the CI path lists are written out twice (no YAML anchors). Details in STATUS.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An empty but lint-clean Rust workspace with all nine crates, their dependency rules
enforced by a script, and CI that runs it all, so lanes `engine` and `services` can open.

**Architecture:** Cargo workspace at the repo root next to the frozen WPF tree; one crate per
spec section 7 entry; shared versions and lints in the root manifest.

**Tech Stack:** Rust 1.98.1 stable, edition 2024, resolver 3, `windows` 0.62.2, Node for checks,
GitHub Actions `windows-latest`.

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` (sections 4 and 7).

## Global Constraints

- Edition 2024, resolver 3, `rust-version = "1.98"`.
- `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs` (clippy `mod_module_files`).
- Limits 500 source, 150 entry, 800 test, 80 per function (clippy `too_many_lines` 80).
- Dependency direction: `plith-core`, `plith-audio`, `plith-media`, `plith-system`, `plith-ipc`
  depend on no `plith-*` crate; `plith-core` has no `windows` dependency; `plith-render` and
  `plith-shell` may depend only on `plith-core` among ours.
- Builds run through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No em dash anywhere; English only; no AI attribution.

## Review Focus

- A crate later adds a forbidden `plith-*` or `windows` dependency: `check-deps.mjs` must fail.
- A WPF-only change must not trigger the Rust workflow, and a Rust change must not skip it.
- `cargo fmt --check` and clippy see every crate, including both binaries.

---

### Task 1: Workspace and crate skeletons

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`
- Create per crate `crates/<name>/Cargo.toml`, `crates/<name>/README.md` (purpose in 2 to 4
  lines, taken from spec section 7), and `src/lib.rs` with a `//!` crate doc, for
  `plith-core`, `plith-render`, `plith-shell`, `plith-audio`, `plith-media`, `plith-system`,
  `plith-ipc`
- Create `crates/plith/src/main.rs` and `crates/plith-osd/src/main.rs`: `fn main() {}` with a
  `//!` doc line

**Interfaces:**
- Produces: package names exactly as listed; `[workspace.dependencies] windows = "0.62.2"`
  (features added later by the crate that needs them); `[workspace.lints]` used by every member
  with `[lints] workspace = true`; `[workspace.package]` edition, rust-version, license MIT.

- [x] **Step 1:** Root `Cargo.toml`: `[workspace] members = ["crates/*"]`, `resolver = "3"`;
  lints: clippy `too_many_lines`, `cognitive_complexity`, `too_many_arguments` = warn,
  `mod_module_files` = deny, `pedantic` = warn with priority -1; rust `unsafe_op_in_unsafe_fn` =
  deny, `missing_docs` = warn. `clippy.toml`: thresholds 80 / 20 / 6 (rust-tauri reference).
  `rust-toolchain.toml`: channel `1.98.1`, components `clippy`, `rustfmt`. `rustfmt.toml`:
  `max_width = 100`.
- [x] **Step 2:** Path dependencies per the Global Constraints: `plith-render` and `plith-shell`
  on `plith-core`; `plith` on all seven libraries; `plith-osd` on `plith-render` and `plith-ipc`.
- [x] **Step 3:** Run `node ~/.claude/bin/heavy-run.mjs -- cargo clippy --workspace --all-targets -- -D warnings`.
  Expected: finishes with no warnings. Then `cargo fmt --all --check`: no output.
- [x] **Step 4:** Run `node scripts/structure-check.mjs`. Expected: `0 violations`.
- [x] **Step 5:** Commit `build: add the Rust workspace and crate skeletons`.

### Task 2: Dependency rule check

**Files:**
- Create: `scripts/check-deps.mjs`, `scripts/check-deps.test.mjs`

**Interfaces:**
- Produces: `export function violations(metadata) -> string[]` (input: parsed
  `cargo metadata --format-version 1 --no-deps` JSON); CLI runs `cargo metadata`, prints each
  violation, exits 1 if any.

- [x] **Step 1:** Write `check-deps.test.mjs` (node:test) with fixture metadata objects:
  `plith-core` depending on `windows` gives `["plith-core may not depend on windows"]`;
  `plith-audio` depending on `plith-core` gives `["plith-audio may not depend on plith-core"]`;
  `plith-render` depending on `plith-shell` gives one violation; the real layout from Task 1
  gives `[]`.
- [x] **Step 2:** Run `node --test scripts/check-deps.test.mjs`. Expected: FAIL, module missing.
- [x] **Step 3:** Implement `violations` from the rule table in Global Constraints.
- [x] **Step 4:** Run the test (PASS) and `node scripts/check-deps.mjs` (exit 0, no output).
- [x] **Step 5:** Commit `build: check the crate dependency rules`.

### Task 3: CI workflow

**Files:**
- Create: `.github/workflows/rust.yml`

- [x] **Step 1:** Workflow on `pull_request` and `push` to `main`, paths `crates/**`,
  `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`,
  `scripts/structure-check.mjs`, `scripts/check-deps.*`, `structure-baseline.json` and the
  workflow itself. `permissions: {}`, `runs-on: windows-latest`, `timeout-minutes: 20`. Steps:
  checkout, toolchain from `rust-toolchain.toml`, cargo cache, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `node scripts/structure-check.mjs`, `node --test scripts/check-deps.test.mjs`,
  `node scripts/check-deps.mjs`.
- [x] **Step 2:** Commit `ci: run Rust lint, tests and structure checks`, push `main` (asked),
  and confirm the run is green with `gh run list --workflow rust.yml --limit 1`.

### Task 4: Record and open lanes

- [x] **Step 1:** ROADMAP R0 to `✅ done, <actual>`; STATUS next = lanes running and R3.
- [x] **Step 2:** `docs/LANES.md` (rules and state) and the two opening prompts in project
  memory; open `engine` (opus) and `services` (sonnet) with `lanes open`, start `lanes watch`.
