//! A theme change reaches the loop beside the mailbox: the painter takes the palette, the page
//! bitmaps are rastered again in it, and a settled island repaints once.

use super::super::RelayoutHandle;
use super::expanded;
use super::harness::*;
use super::prewarm::{open_at_rest, pages};
use plith_core::display::Display;
use plith_core::launch::ClockRequest;
use plith_core::scene::{Scene, Suppression};
use plith_core::theme::Palette;
use plith_core::theme::colour::Rgb;
use plith_core::theme::registry::{AccentPick, Picks, Registry, ThemeId};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn handle_of(run: &Run) -> RelayoutHandle {
    RelayoutHandle {
        mailbox: Arc::clone(&run.mailbox),
        shared: Arc::clone(&run.shared),
        clock: ClockRequest::Auto,
        last: Arc::new(Mutex::new(Display {
            layout: layout(),
            monitor: 0,
            hz: 240.0,
            remote: false,
        })),
    }
}

fn wait_palettes(run: &Run, n: usize) {
    let deadline = Instant::now() + BOUND;
    while lock(&run.palettes).len() < n {
        assert!(Instant::now() < deadline, "expected {n} palette switches");
        thread::sleep(Duration::from_millis(1));
    }
}

fn locked() -> Scene {
    Scene {
        suppression: Suppression {
            locked: true,
            ..Suppression::default()
        },
        ..Scene::default()
    }
}

#[test]
fn an_open_island_at_rest_rasters_every_page_again_and_repaints_once() {
    let run = open_at_rest();
    let handle = handle_of(&run);
    let (frames, warmed) = (run.frames().len(), run.prewarms().len());
    handle.set_palette(york());
    run.wait_frames(frames + 1);
    run.assert_parked();
    assert_eq!(*lock(&run.palettes), vec![york()]);
    assert_eq!(run.frames().len(), frames + 1, "one repaint");
    let prewarms = run.prewarms();
    assert_eq!(prewarms.len(), warmed + 1, "one raster pass");
    assert_eq!(
        prewarms[warmed].0,
        vec![0, 1, 2, 3, 4, 5],
        "every page and the pill"
    );
    let _ = run.stop();
}

#[test]
fn a_collapsed_island_repaints_the_pill_and_rasters_no_page() {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    let handle = handle_of(&run);
    handle.set_palette(sodyum());
    run.wait_frames(2);
    run.assert_parked();
    assert_eq!(*lock(&run.palettes), vec![sodyum()]);
    assert_eq!(run.frames().len(), 2);
    assert!(run.prewarms().is_empty(), "collapsed: no page raster");
    assert_eq!(run.harness.total(), 0, "no clock wait at rest");
    let _ = run.stop();
}

#[test]
fn a_suppressed_island_takes_the_palette_and_paints_it_when_it_returns() {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(locked());
    let handle = handle_of(&run);
    handle.set_palette(kagit());
    wait_palettes(&run, 1);
    run.assert_parked();
    assert_eq!(run.frames().len(), 1, "nothing painted while locked");
    run.mailbox.publish(Scene::default());
    run.wait_frames(2);
    run.assert_parked();
    assert_eq!(*lock(&run.palettes), vec![kagit()]);
    let _ = run.stop();
}

#[test]
fn a_palette_during_a_motion_keeps_the_motion_going() {
    let run = start(Setup {
        content: Some(pages()),
        ..Setup::default()
    });
    run.wait_frames(1);
    run.mailbox.publish(expanded(0));
    run.wait_calls(10);
    handle_of(&run).set_palette(york());
    let report = run.report();
    assert!(report.frames > 10, "{report}");
    assert!(run.fatal.try_recv().is_err());
    wait_palettes(&run, 1);
    assert_eq!(*lock(&run.palettes), vec![york()]);
    let _ = run.stop();
}

/// Gece under a floor and an accent pick, as the settings migration turns a retired theme.
fn migrated(floor: Option<[u8; 3]>, accent: [u8; 3]) -> Palette {
    let accent = AccentPick::Colour(Rgb(accent));
    let picks = Picks {
        accent: Some(&accent),
        floor: floor.map(Rgb),
        windows_accent: None,
    };
    Registry::builtin().resolve(&ThemeId::gece(), picks).open
}

fn kagit() -> Palette {
    migrated(Some([0xF3, 0xF2, 0xED]), [0x3F, 0x6E, 0x00])
}

fn york() -> Palette {
    migrated(Some([0x1C, 0x24, 0x20]), [0xFF, 0xE9, 0xA8])
}

fn sodyum() -> Palette {
    migrated(None, [0xFF, 0xB2, 0x24])
}
