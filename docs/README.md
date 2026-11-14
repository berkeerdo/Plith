# docs map

Read chain at session start: `CLAUDE.md` -> `docs/STATUS.md` -> `docs/plans/active/`
(with lanes, the master also reads `docs/LANES.md`).

| Path | What |
|---|---|
| `VISION.md` | What Plith is and is not |
| `ROADMAP.md` | Board of phases R0 to R7, estimate beside actual |
| `STATUS.md` | Now: active phase, next task, risks, last decisions (rewritten, max 120 lines) |
| `LANES.md` | Only while parallel lanes run; master-owned |
| `decisions/` | ADRs, immutable, `NNNN-<slug>.md` |
| `specs/` | Design docs |
| `plans/active/`, `plans/done/` | The plan being executed; finished plans with the honest banner |
| `journal/YYYY-MM.md` | What left STATUS |
| `research/` | Competitor and engine research |
| `quality/` | Measurements and `structure-debt.md` |

## Frozen: the WPF app (0.3.1)

The WPF tree is the working reference until the Rust build reaches parity (ADR 0001). Its records
stay where they were written and are not updated except for a release-blocking fix:
`wpf/CLAUDE-wpf.md` (its project guide and the lesson about measuring), `wpf/ROADMAP.md`,
`PERF-VERIFICATION.md`, `PHASE5-VERIFICATION.md`, `PHASE6-VERIFICATION.md`,
`SHELF-VERIFICATION.md`, `superpowers/` (WPF specs, plans, design mockups), `screenshots/`.
