use super::*;
use plith_core::scene::Suppression;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct FakeStop(Rc<Cell<u32>>);

impl Stopper for FakeStop {
    fn stop_and_join(&self) {
        self.0.set(self.0.get() + 1);
    }
}

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("plith-app-{name}-{}", std::process::id()));
    let log = LogFile::open_at(&dir.join("plith.log")).unwrap();
    let mailbox = Arc::new(SceneMailbox::new(Scene::default(), Arc::default()));
    App::new(mailbox, Arc::new(log), Arc::new(AtomicBool::new(false)))
}

#[test]
fn closing_before_render_attached_is_safe() {
    let mut app = app("early");
    app.on_event(ShellEvent::Closing);
    assert!(app.closing.load(Ordering::Acquire));
}

#[test]
fn closing_stops_the_attached_render() {
    let mut app = app("stop");
    let fake = FakeStop::default();
    app.attach_render(fake.clone());
    app.on_event(ShellEvent::Closing);
    assert_eq!(fake.0.get(), 1);
    assert!(app.closing.load(Ordering::Acquire));
}

#[test]
fn click_toggles_expanded() {
    let mut app = app("click");
    app.on_event(ShellEvent::Click);
    assert!(app.mailbox.latest().scene.expanded);
    app.on_event(ShellEvent::Click);
    assert!(!app.mailbox.latest().scene.expanded);
    assert_eq!(app.mailbox.latest().generation, 2);
}

#[test]
fn suppression_republishes() {
    let mut app = app("suppress");
    let suppression = Suppression {
        locked: true,
        ..Suppression::default()
    };
    app.on_event(ShellEvent::Suppression(suppression));
    assert_eq!(app.mailbox.latest().scene.suppression, suppression);
    assert_eq!(app.mailbox.latest().generation, 1);
}

#[test]
fn wake_changes_nothing_yet() {
    let mut app = app("wake");
    app.on_event(ShellEvent::Wake);
    assert_eq!(app.mailbox.latest().generation, 0);
}
