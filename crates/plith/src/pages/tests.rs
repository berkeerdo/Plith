use super::*;
use crate::services::Services;
use crate::services::fixture::FixtureServices;
use plith_core::draw::Node;
use plith_core::launch::Fixture;

fn fixture_facts(kind: Fixture) -> Facts {
    FixtureServices::new(kind).facts(1.0)
}

fn inputs<'a>(facts: &'a Facts, activity: &'a Activity) -> Inputs<'a> {
    Inputs {
        facts,
        activity,
        pick: PickMode::None,
        now_s: 1.0,
        clock: "21:47",
    }
}

#[test]
fn changes_mark_the_pages_they_reach() {
    assert_eq!(Dirty::from_changes(Changes::NONE), Dirty::default());
    assert_eq!(
        Dirty::from_changes(Changes::AUDIO),
        Dirty {
            pages: [true, true, false, true],
            pill: true,
        }
    );
    assert_eq!(
        Dirty::from_changes(Changes::MEDIA),
        Dirty {
            pages: [false, true, false, false],
            pill: true,
        }
    );
    assert_eq!(
        Dirty::from_changes(Changes::SYSTEM),
        Dirty {
            pages: [false, false, false, true],
            pill: false,
        }
    );
    assert_eq!(
        Dirty::from_changes(Changes::AUDIO | Changes::MEDIA | Changes::SYSTEM),
        Dirty {
            pages: [true, true, false, true],
            pill: true,
        },
        "the shelf reads no service"
    );
    assert_eq!(
        Dirty::page(PageId::Media),
        Dirty {
            pages: [false, true, false, false],
            pill: false,
        }
    );
    assert!(!Dirty::default().any());
}

#[test]
fn content_holds_every_page_the_pill_and_the_art() {
    let facts = fixture_facts(Fixture::Banana);
    let activity = Activity::default();
    let pages = Pages::new(&inputs(&facts, &activity));
    let content = pages.content();
    for (i, list) in content.pages.iter().enumerate() {
        assert!(!list.nodes.is_empty(), "page {i} is empty");
        assert!((pages.shapes()[i].height - list.size.1).abs() < 1e-9);
    }
    assert!(!content.pill.nodes.is_empty());
    assert_eq!(content.art.len(), 1);
    let has_image = content.pages[1]
        .nodes
        .iter()
        .any(|n| matches!(n, Node::Image { .. }));
    assert!(has_image);
    assert!(!pages.targets()[PageId::Audio.index()].is_empty());
}

#[test]
fn no_art_publishes_no_art() {
    let mut facts = fixture_facts(Fixture::Bare);
    facts.art = None;
    facts.media.art = None;
    let activity = Activity::default();
    assert!(
        Pages::new(&inputs(&facts, &activity))
            .content()
            .art
            .is_empty()
    );
}

#[test]
fn rebuild_touches_only_the_dirty_lists() {
    let mut facts = fixture_facts(Fixture::Banana);
    let activity = Activity::default();
    let mut pages = Pages::new(&inputs(&facts, &activity));
    let before = pages.content();
    facts.system.brightness = Some(("DELL AW2725DF".to_string(), 20));
    facts.pill.value = Some(1);
    pages.rebuild(Dirty::page(PageId::System), &inputs(&facts, &activity));
    let after = pages.content();
    for i in 0..3 {
        assert!(Arc::ptr_eq(&before.pages[i], &after.pages[i]), "page {i}");
    }
    assert!(Arc::ptr_eq(&before.pill, &after.pill));
    assert_ne!(before.pages[3], after.pages[3]);
}

#[test]
fn a_taller_audio_page_changes_its_shape() {
    let activity = Activity::default();
    let banana = fixture_facts(Fixture::Banana);
    let potato = fixture_facts(Fixture::Potato);
    let short = Pages::new(&inputs(&banana, &activity)).shapes()[0];
    let tall = Pages::new(&inputs(&potato, &activity)).shapes()[0];
    assert!(tall.height > short.height);
}
