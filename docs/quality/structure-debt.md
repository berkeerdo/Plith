# Structure debt

The 13 oversized files frozen in `structure-baseline.json` on 2026-11-14 all belong to the frozen
WPF tree. They are not split: the whole tree is deleted at parity (ROADMAP R7, ADR 0001). They
may not grow in the meantime.

Rust code has no debt rows. A new oversized Rust file is a structure-check failure, not a row here.
