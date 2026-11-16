use super::*;
use plith_core::scene::{Scene, SceneMailbox, Suppression};
use std::sync::atomic::Ordering;

/// The scene after `step`, as the UI thread's expand, turn and collapse paths leave it (the app
/// tests cover those paths themselves).
fn apply(step: DemoStep, scene: Scene) -> Scene {
    match step {
        DemoStep::Expand => Scene {
            expanded: true,
            ..scene
        },
        DemoStep::Collapse => Scene {
            expanded: false,
            ..scene
        },
        DemoStep::Page(page) => Scene {
            expanded: true,
            page,
            ..scene
        },
        DemoStep::Wait(_) | DemoStep::Content(_) => scene,
    }
}

/// A step sink that applies scene steps to `mailbox`, standing in for the UI thread.
fn onto(mailbox: &SceneMailbox) -> impl Fn(DemoStep) + '_ {
    move |step| {
        let _ = mailbox.update(|scene| apply(step, scene));
    }
}

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
fn empty_script_ends_at_once_with_huge_repeat() {
    let script = DemoScript {
        steps: Vec::new(),
        repeat: u32::MAX,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    assert!(play(&script, &stop, &onto(&mailbox), &|| {}, &|_| {}));
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
    assert!(play(&script, &stop, &onto(&mailbox), &|| {}, &|_| {}));
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
    assert!(play(&script, &stop, &onto(&mailbox), &|| {}, &|_| {}));
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
    assert!(play(&script, &stop, &onto(&mailbox), &|| {}, &|_| {}));
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
    assert!(!play(&script, &stop, &onto(&mailbox), &|| {}, &|_| {}));
    stop.store(true, Ordering::Relaxed);
    assert!(!sleep_stoppable(60_000, &stop));
}

#[test]
fn sleep_without_stop_completes() {
    assert!(sleep_stoppable(60, &AtomicBool::new(false)));
}

#[test]
fn play_wakes_after_each_scene_step() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand, DemoStep::Wait(0), DemoStep::Collapse],
        repeat: 2,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    let wakes = std::cell::Cell::new(0);
    assert!(play(
        &script,
        &stop,
        &onto(&mailbox),
        &|| wakes.set(wakes.get() + 1),
        &|_| {}
    ));
    assert_eq!(
        wakes.get(),
        4,
        "two scene steps per round, waits do not wake"
    );
}

#[test]
fn content_step_asks_the_services_and_wakes_without_touching_the_scene() {
    let script = DemoScript {
        steps: vec![
            DemoStep::Content(2),
            DemoStep::Wait(0),
            DemoStep::Content(0),
        ],
        repeat: 1,
    };
    let mailbox = SceneMailbox::new(Scene::default(), Arc::default());
    let stop = AtomicBool::new(false);
    let asked = std::cell::RefCell::new(Vec::new());
    let wakes = std::cell::Cell::new(0);
    assert!(play(
        &script,
        &stop,
        &onto(&mailbox),
        &|| wakes.set(wakes.get() + 1),
        &|n| asked.borrow_mut().push(n),
    ));
    assert_eq!(*asked.borrow(), vec![2, 0]);
    assert_eq!(wakes.get(), 2);
    assert_eq!(mailbox.latest().generation, 0, "the scene is untouched");
}

#[test]
fn content_requests_cross_threads_through_a_channel() {
    let (tx, rx) = std::sync::mpsc::channel();
    let log = |_: &str| {};
    let request = content_requester(Some(tx), &log);
    request(3);
    request(1);
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![3, 1]);
}

#[test]
fn content_without_a_fixture_is_a_logged_no_op() {
    let lines = std::cell::RefCell::new(Vec::new());
    let log = |line: &str| lines.borrow_mut().push(line.to_string());
    let request = content_requester(None, &log);
    request(1);
    request(2);
    assert_eq!(lines.borrow().len(), 1, "logged once, not per step");
}

#[test]
fn play_hands_scene_steps_to_the_ui_thread_then_wakes_it() {
    let script = DemoScript {
        steps: vec![DemoStep::Expand, DemoStep::Wait(0), DemoStep::Page(2)],
        repeat: 1,
    };
    let events = std::cell::RefCell::new(Vec::new());
    let stop = AtomicBool::new(false);
    assert!(play(
        &script,
        &stop,
        &|step| events.borrow_mut().push(format!("{step:?}")),
        &|| events.borrow_mut().push("wake".to_string()),
        &|_| {},
    ));
    assert_eq!(
        *events.borrow(),
        vec!["Expand", "wake", "Page(2)", "wake"],
        "the demo thread publishes no scene itself"
    );
}
