//! Full and visible motion durations, simulated at 1/240 s the way the render loop steps.
//! Expected full values come from the R1b plan simulation (2026-11-14), tolerance one frame.

use plith_core::geometry::IslandShape;
use plith_core::island::IslandMotion;
use plith_core::page::PageTurn;
use plith_core::scene::{Scene, Suppression};
use plith_core::spring::Spring;
use plith_core::tuning::{BRISK, R1, TuningSet};
use plith_core::visible::{DrawnState, visibly_differs};

const H: f64 = 1.0 / 240.0;
const FRAME_MS: f64 = 1000.0 / 240.0;

/// The page shapes the R1 log was measured with; the durations are pinned to these sizes.
const R1_SHAPES: [IslandShape; 4] = [
    IslandShape {
        width: 620.0,
        height: 216.0,
        radius: 40.0,
    },
    IslandShape {
        width: 620.0,
        height: 320.0,
        radius: 40.0,
    },
    IslandShape {
        width: 480.0,
        height: 180.0,
        radius: 40.0,
    },
    IslandShape {
        width: 480.0,
        height: 180.0,
        radius: 40.0,
    },
];

fn scene(expanded: bool, page: usize) -> Scene {
    Scene {
        expanded,
        page,
        suppression: Suppression::default(),
        shapes: R1_SHAPES,
    }
}

fn content_target(scene: Scene) -> f64 {
    if scene.expanded { 1.0 } else { 0.0 }
}

fn ms(frames: usize) -> f64 {
    f64::from(u32::try_from(frames).unwrap()) * FRAME_MS
}

/// Returns `(visible_ms, full_ms)` of the motion from `from` to `to` under `set`.
fn simulate(set: TuningSet, from: Scene, to: Scene) -> (f64, f64) {
    let mut island = IslandMotion::new(from.target(), set);
    let mut pages = PageTurn::new(from.page, set);
    let mut content = Spring::new(content_target(from), set.opacity).with_eps(0.002, 0.01);
    island.retarget(to.target());
    content.retarget(content_target(to), set.opacity);
    if to.page != from.page {
        pages.turn_to(to.page);
    }
    let rest = DrawnState::at_rest(to.target(), to.expanded, to.page);
    let (mut frames, mut last_visible) = (0_usize, 0_usize);
    loop {
        let i = island.step(H);
        let p = pages.step(H);
        let c = content.step(H);
        frames += 1;
        let drawn = DrawnState {
            shape: island.shape(),
            content: content.value(),
            pages: pages.layers(),
        };
        if visibly_differs(&drawn, &rest) {
            last_visible = frames;
        }
        if i && p && c {
            break;
        }
        assert!(frames < 2000, "never settled");
    }
    (ms(last_visible), ms(frames))
}

fn motions() -> [(&'static str, Scene, Scene); 5] {
    [
        ("expand", scene(false, 0), scene(true, 0)),
        ("collapse", scene(true, 0), scene(false, 0)),
        ("page 0->1", scene(true, 0), scene(true, 1)),
        ("page 1->2", scene(true, 1), scene(true, 2)),
        ("page 2->0", scene(true, 2), scene(true, 0)),
    ]
}

fn assert_full(set: TuningSet, expected: [f64; 5]) {
    for ((name, from, to), want) in motions().into_iter().zip(expected) {
        let (_, full) = simulate(set, from, to);
        println!("{} {name}: full {full:.1} ms (plan {want})", set.name);
        assert!(
            (full - want).abs() <= 4.2,
            "{} {name}: full {full:.1} ms, expected {want}",
            set.name
        );
    }
}

#[test]
fn r1_full_durations_match_r1_log() {
    assert_full(R1, [800.0, 813.0, 775.0, 421.0, 563.0]);
}

#[test]
fn brisk_full_durations() {
    assert_full(BRISK, [592.0, 813.0, 421.0, 421.0, 421.0]);
}

#[test]
fn visible_durations_are_shorter_where_a_tail_hides() {
    println!("tune     motion     visible ms  full ms");
    for set in [R1, BRISK] {
        for (name, from, to) in motions() {
            let (visible, full) = simulate(set, from, to);
            println!("{:<8} {:<10} {visible:>10.1} {full:>8.1}", set.name, name);
            assert!(visible <= full + 1e-9, "{} {name}", set.name);
            if set.name == "r1" && name == "collapse" {
                assert!(visible <= 450.0, "r1 collapse visible {visible}");
            }
        }
    }
}
