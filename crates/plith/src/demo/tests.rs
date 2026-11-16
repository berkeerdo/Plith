use super::*;
use plith_core::scene::Suppression;
use std::sync::atomic::Ordering;

fn scene(expanded: bool, page: usize) -> Scene {
    Scene {
        expanded,
        page,
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..Scene::default()
    }
}

#[test]
fn expand_keeps_the_page() {
    assert_eq!(apply(DemoStep::Expand, scene(false, 2)), scene(true, 2));
}

#[test]
fn collapse_keeps_the_page() {
    assert_eq!(apply(DemoStep::Collapse, scene(true, 1)), scene(false, 1));
}

#[test]
fn page_sets_page_and_expands() {
    assert_eq!(apply(DemoStep::Page(1), scene(false, 0)), scene(true, 1));
}

#[test]
fn wait_changes_nothing() {
    assert_eq!(apply(DemoStep::Wait(10), scene(true, 2)), scene(true, 2));
}

#[test]
fn empty_script_ends_at_once_with_huge_repeat() {
    let script = DemoScript {
        steps: Vec::new(),
        repeat: u32::MAX,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    assert!(play(&script, &mailbox, &stop));
    assert_eq!(mailbox.latest().generation, 0, "nothing is published");
}

#[test]
fn play_publishes_only_changes_so_a_repeated_round_adds_nothing() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand, DemoStep::Wait(0), DemoStep::Page(1)],
        repeat: 2,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    assert!(play(&script, &mailbox, &stop));
    let latest = mailbox.latest();
    assert_eq!(latest.generation, 2, "the second round changes nothing");
    assert_eq!((latest.scene.expanded, latest.scene.page), (true, 1));
}

#[test]
fn play_publishes_changing_steps_every_round() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand, DemoStep::Collapse],
        repeat: 2,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    assert!(play(&script, &mailbox, &stop));
    assert_eq!(mailbox.latest().generation, 4);
}

#[test]
fn play_reads_the_latest_scene_so_a_click_is_respected() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand],
        repeat: 1,
    };
    let mailbox = SceneMailbox::new(scene(false, 2), Arc::default());
    let stop = AtomicBool::new(false);
    assert!(play(&script, &mailbox, &stop));
    assert_eq!(mailbox.latest().scene, scene(true, 2));
}

#[test]
fn a_set_stop_flag_ends_play_and_sleep() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand, DemoStep::Wait(60_000)],
        repeat: 1,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(true);
    assert!(!play(&script, &mailbox, &stop));
    stop.store(true, Ordering::Relaxed);
    assert!(!sleep_stoppable(60_000, &stop));
}

#[test]
fn sleep_without_stop_completes() {
    assert!(sleep_stoppable(60, &AtomicBool::new(false)));
}
