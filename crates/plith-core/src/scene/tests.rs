use super::*;
use crate::draw::{DisplayList, Ink, Node, PageContent, Rect};
use crate::shapes::{PAGES, REST};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

fn expanded_scene() -> Scene {
    Scene {
        expanded: true,
        ..Scene::default()
    }
}

fn mailbox(scene: Scene) -> SceneMailbox {
    SceneMailbox::new(scene, Arc::new(PageContent::empty()))
}

fn content_with_fill(width: f64) -> Arc<PageContent> {
    let mut content = PageContent::empty();
    content.pages[0] = Arc::new(DisplayList {
        nodes: vec![Node::Fill {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: 10.0,
            },
            radius: 2.0,
            ink: Ink::Floor,
        }],
        size: (620.0, 300.0),
    });
    Arc::new(content)
}

#[test]
fn concurrent_updates_never_lose_a_field() {
    let locked = Suppression {
        locked: true,
        ..Suppression::default()
    };
    let mailbox = Arc::new(mailbox(Scene {
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
    let Snapshot {
        scene, generation, ..
    } = mailbox.latest();
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
        ..Scene::default()
    };
    assert_eq!(scene.target(), PAGES[1]);
}

#[test]
fn wait_newer_wakes_on_publish_from_another_thread() {
    let mailbox = Arc::new(mailbox(Scene::default()));
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
    assert_eq!(generation, Some(1));
    let got = rx.recv_timeout(TIMEOUT).expect("waiter did not wake");
    let snapshot = got.expect("not stopped");
    assert_eq!((snapshot.scene, snapshot.generation), (expanded, 1));
    waiter.join().unwrap();
}

#[test]
fn wait_newer_returns_at_once_when_already_newer() {
    let mailbox = mailbox(Scene::default());
    let generation = mailbox.publish(expanded_scene());
    assert_eq!(generation, Some(1));
    let snapshot = mailbox.wait_newer(0).expect("newer");
    assert_eq!((snapshot.scene, snapshot.generation), (expanded_scene(), 1));
}

#[test]
fn latest_wins_when_two_scenes_are_published_before_a_read() {
    let mailbox = mailbox(Scene::default());
    assert_eq!(mailbox.latest().generation, 0);
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
    assert_eq!(mailbox.publish(first), Some(1));
    assert_eq!(mailbox.publish(second), Some(2));
    let snapshot = mailbox.wait_newer(0).expect("newer");
    assert_eq!((snapshot.scene, snapshot.generation), (second, 2));
    let latest = mailbox.latest();
    assert_eq!((latest.scene, latest.generation), (second, 2));
}

#[test]
fn stop_releases_a_blocked_waiter() {
    let mailbox = Arc::new(mailbox(Scene::default()));
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
    assert!(got.is_none());
    waiter.join().unwrap();
    assert!(mailbox.wait_newer(0).is_none());
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
    let mailbox = Arc::new(mailbox(Scene::default()));
    let poisoner = Arc::clone(&mailbox);
    let _ = thread::spawn(move || {
        let _guard = poisoner.state.lock().unwrap();
        panic!("poison the mailbox");
    })
    .join();
    assert_eq!(mailbox.publish(expanded_scene()), Some(1));
    assert_eq!(mailbox.latest().generation, 1);
}

#[test]
fn update_same_scene_returns_none() {
    let mailbox = mailbox(Scene::default());
    assert_eq!(mailbox.update(|s| s), None);
    assert_eq!(mailbox.publish(Scene::default()), None);
    assert_eq!(mailbox.latest().generation, 0);
    assert_eq!(
        mailbox.update(|s| Scene {
            expanded: true,
            ..s
        }),
        Some(1)
    );
    assert_eq!(
        mailbox.update(|s| Scene {
            expanded: true,
            ..s
        }),
        None
    );
}

#[test]
fn publish_same_content_does_not_bump() {
    let mailbox = mailbox(Scene::default());
    let first = content_with_fill(5.0);
    assert_eq!(mailbox.publish_content(Arc::clone(&first)), Some(1));
    assert_eq!(mailbox.publish_content(Arc::clone(&first)), None);
    assert_eq!(mailbox.publish_content(content_with_fill(5.0)), None);
    assert_eq!(mailbox.latest().generation, 1);
}

#[test]
fn publish_new_content_bumps_and_wakes() {
    let mailbox = Arc::new(mailbox(Scene::default()));
    let (tx, rx) = mpsc::channel();
    let waiter = {
        let mailbox = Arc::clone(&mailbox);
        thread::spawn(move || {
            tx.send(mailbox.wait_newer(0)).unwrap();
        })
    };
    thread::sleep(Duration::from_millis(20));
    let wanted = content_with_fill(7.0);
    assert_eq!(mailbox.publish_content(Arc::clone(&wanted)), Some(1));
    let snapshot = rx
        .recv_timeout(TIMEOUT)
        .expect("waiter did not wake")
        .expect("not stopped");
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.content, wanted);
    waiter.join().unwrap();
}

#[test]
fn target_reads_scene_shapes() {
    let mut shapes = PAGES;
    shapes[2].height = 99.0;
    let scene = Scene {
        expanded: true,
        page: 2,
        shapes,
        ..Scene::default()
    };
    assert_eq!(scene.target(), shapes[2]);
    assert_ne!(scene.target(), PAGES[2]);
    assert_eq!(Scene::default().shapes, PAGES);
}

#[test]
fn publish_content_compares_outside_the_lock() {
    let mailbox = Arc::new(mailbox(Scene::default()));
    // Same answers as a compare under the lock: equal content never bumps, new content does.
    let first = content_with_fill(3.0);
    assert_eq!(mailbox.publish_content(Arc::clone(&first)), Some(1));
    assert_eq!(mailbox.publish_content(content_with_fill(3.0)), None);
    assert_eq!(mailbox.publish_content(content_with_fill(4.0)), Some(2));
    // Racing publishers: every bump is counted once and the stored content is one they sent.
    let publishers: Vec<_> = (0..4_u32)
        .map(|t| {
            let mailbox = Arc::clone(&mailbox);
            thread::spawn(move || {
                (0..50_u32)
                    .filter_map(|i| {
                        mailbox.publish_content(content_with_fill(f64::from(t * 1000 + i % 7)))
                    })
                    .count()
            })
        })
        .collect();
    let bumps: usize = publishers.into_iter().map(|h| h.join().unwrap()).sum();
    let latest = mailbox.latest();
    assert_eq!(usize::try_from(latest.generation).unwrap(), 2 + bumps);
    let Node::Fill { rect, .. } = &latest.content.pages[0].nodes[0] else {
        panic!("a fill node");
    };
    assert!(rect.w >= 0.0);
}
