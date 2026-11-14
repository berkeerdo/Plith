use super::*;
use crate::shapes::{PAGES, REST};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

#[test]
fn concurrent_updates_never_lose_a_field() {
    let locked = Suppression {
        locked: true,
        ..Suppression::default()
    };
    let mailbox = Arc::new(SceneMailbox::new(Scene {
        suppression: locked,
        ..Scene::default()
    }));
    let toggler = {
        let mailbox = Arc::clone(&mailbox);
        thread::spawn(move || {
            for _ in 0..1000 {
                mailbox.update(|s| Scene {
                    expanded: !s.expanded,
                    ..s
                });
            }
        })
    };
    let cycler = {
        let mailbox = Arc::clone(&mailbox);
        thread::spawn(move || {
            for _ in 0..1000 {
                mailbox.update(|s| Scene {
                    page: (s.page + 1) % 3,
                    ..s
                });
            }
        })
    };
    toggler.join().unwrap();
    cycler.join().unwrap();
    let (scene, generation) = mailbox.latest();
    assert_eq!(scene.suppression, locked);
    assert_eq!(generation, 2000);
    assert!(!scene.expanded, "1000 toggles end where they began");
    assert_eq!(scene.page, 1000 % 3, "no cycle step was lost");
}

#[test]
fn any_suppression_reason_suppresses() {
    assert!(!Suppression::default().any());
    let display_off = Suppression {
        display_off: true,
        ..Suppression::default()
    };
    let locked = Suppression {
        locked: true,
        ..Suppression::default()
    };
    let fullscreen = Suppression {
        fullscreen: true,
        ..Suppression::default()
    };
    assert!(display_off.any());
    assert!(locked.any());
    assert!(fullscreen.any());
}

#[test]
fn target_is_rest_when_collapsed_and_the_page_shape_when_expanded() {
    assert_eq!(Scene::default().target(), REST);
    for (page, shape) in PAGES.iter().enumerate() {
        let scene = Scene {
            expanded: true,
            page,
            ..Scene::default()
        };
        assert_eq!(scene.target(), *shape);
    }
    let collapsed_on_page = Scene {
        page: 2,
        ..Scene::default()
    };
    assert_eq!(collapsed_on_page.target(), REST);
}

#[test]
fn suppression_does_not_change_the_target() {
    let scene = Scene {
        expanded: true,
        page: 1,
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
    };
    assert_eq!(scene.target(), PAGES[1]);
}

#[test]
fn wait_newer_wakes_on_publish_from_another_thread() {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default()));
    let (tx, rx) = mpsc::channel();
    let waiter = {
        let mailbox = Arc::clone(&mailbox);
        thread::spawn(move || {
            tx.send(mailbox.wait_newer(0)).unwrap();
        })
    };
    thread::sleep(Duration::from_millis(20));
    let expanded = Scene {
        expanded: true,
        ..Scene::default()
    };
    let generation = mailbox.publish(expanded);
    assert_eq!(generation, 1);
    let got = rx.recv_timeout(TIMEOUT).expect("waiter did not wake");
    assert_eq!(got, Some((expanded, 1)));
    waiter.join().unwrap();
}

#[test]
fn wait_newer_returns_at_once_when_already_newer() {
    let mailbox = SceneMailbox::new(Scene::default());
    let generation = mailbox.publish(Scene::default());
    assert_eq!(mailbox.wait_newer(0), Some((Scene::default(), generation)));
}

#[test]
fn latest_wins_when_two_scenes_are_published_before_a_read() {
    let mailbox = SceneMailbox::new(Scene::default());
    assert_eq!(mailbox.latest(), (Scene::default(), 0));
    let first = Scene {
        expanded: true,
        page: 1,
        ..Scene::default()
    };
    let second = Scene {
        expanded: true,
        page: 2,
        ..Scene::default()
    };
    assert_eq!(mailbox.publish(first), 1);
    assert_eq!(mailbox.publish(second), 2);
    assert_eq!(mailbox.wait_newer(0), Some((second, 2)));
    assert_eq!(mailbox.latest(), (second, 2));
}

#[test]
fn stop_releases_a_blocked_waiter() {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default()));
    let (tx, rx) = mpsc::channel();
    let waiter = {
        let mailbox = Arc::clone(&mailbox);
        thread::spawn(move || {
            tx.send(mailbox.wait_newer(0)).unwrap();
        })
    };
    thread::sleep(Duration::from_millis(20));
    mailbox.stop();
    let got = rx.recv_timeout(TIMEOUT).expect("waiter did not wake");
    assert_eq!(got, None);
    waiter.join().unwrap();
    assert_eq!(mailbox.wait_newer(0), None);
}

#[test]
fn shape_cell_starts_empty_and_returns_the_latest_frame() {
    let cell = ShapeCell::default();
    assert_eq!(cell.latest(), None);
    let a = ShapeFrame {
        shape: REST,
        previous: REST,
        generation: 1,
    };
    let b = ShapeFrame {
        shape: PAGES[0],
        previous: REST,
        generation: 2,
    };
    cell.publish(a);
    cell.publish(b);
    assert_eq!(cell.latest(), Some(b));
}

#[test]
fn a_poisoned_lock_does_not_panic() {
    let mailbox = Arc::new(SceneMailbox::new(Scene::default()));
    let poisoner = Arc::clone(&mailbox);
    let _ = thread::spawn(move || {
        let _guard = poisoner.state.lock().unwrap();
        panic!("poison the mailbox");
    })
    .join();
    assert_eq!(mailbox.publish(Scene::default()), 1);
    assert_eq!(mailbox.latest().1, 1);
}
