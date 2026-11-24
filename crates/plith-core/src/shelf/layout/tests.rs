#![allow(clippy::float_cmp)]

use super::*;

#[test]
fn the_empty_shelf_is_170_high_with_no_wagons() {
    let l = layout(0);
    assert_eq!(l.height, 170.0);
    assert!(l.wagons.is_empty() && l.slot.is_none() && l.tracks.is_empty());
}

#[test]
fn one_wagon_sits_where_the_mock_puts_the_first() {
    let l = layout(1);
    assert_eq!(l.height, 214.0);
    let w = l.wagons[0];
    assert_eq!(
        (w.rect.x, w.rect.y, w.rect.w, w.rect.h),
        (30.0, 62.0, 104.0, 92.0)
    );
    assert_eq!(w.track_y, 170.0);
    assert_eq!(w.stations, [50.0, 114.0]);
    assert_eq!(l.footer_y, 196.0);
}

#[test]
fn the_slot_follows_the_last_wagon() {
    for count in 1..=3 {
        let l = layout(count);
        let slot = l.slot.unwrap();
        let last = l.wagons[count - 1].rect;
        assert_eq!(slot.x, last.x + WAGON_W + GAP, "{count} wagons");
        assert_eq!((slot.y, slot.w, slot.h), (62.0, 104.0, 92.0));
    }
}

#[test]
fn four_wagons_and_the_slot_are_the_mock() {
    let l = layout(4);
    assert_eq!(l.height, 214.0);
    assert_eq!(l.wagons[3].rect.x, 30.0 + 3.0 * 116.0);
    let slot = l.slot.unwrap();
    assert_eq!((slot.x, slot.w), (494.0, 96.0));
    assert_eq!(l.tracks, [170.0]);
}

#[test]
fn the_fifth_wagon_opens_a_second_row_one_pitch_down() {
    let l = layout(5);
    assert_eq!(l.height, 338.0);
    assert_eq!(l.tracks, [170.0, 294.0]);
    let fifth = l.wagons[4].rect;
    assert_eq!((fifth.x, fifth.y), (30.0, 186.0));
    let slot = l.slot.unwrap();
    assert_eq!((slot.x, slot.y), (146.0, 186.0));
    assert_eq!(l.footer_y, 320.0);
}

#[test]
fn eight_wagons_fill_both_rows_and_the_slot_ends_the_second() {
    let l = layout(8);
    assert_eq!(l.height, 338.0);
    assert_eq!(l.wagons.len(), 8);
    let slot = l.slot.unwrap();
    assert_eq!((slot.x, slot.y, slot.w), (494.0, 186.0, 96.0));
}

#[test]
fn more_than_the_capacity_is_laid_out_as_eight() {
    assert_eq!(layout(20), layout(8));
}

#[test]
fn every_page_fits_the_surface_and_its_content_stays_inside() {
    for count in 0..=8 {
        let l = layout(count);
        assert!(l.height <= crate::shapes::PAGE_MAX_HEIGHT, "{count}");
        for w in &l.wagons {
            assert!(w.rect.x + w.rect.w <= RIGHT);
            assert!(w.rect.y + w.rect.h < w.track_y);
            assert!(w.track_y < l.footer_y - 10.0, "{count}: track meets footer");
        }
        if let Some(slot) = l.slot {
            assert!(slot.x + slot.w <= RIGHT + 0.001);
        }
    }
}

#[test]
fn the_remove_cross_sits_in_the_top_right_corner_of_its_wagon() {
    for count in [1, 5, 8] {
        for w in layout(count).wagons {
            let (cx, cy) = w.cross;
            assert!(cx > w.rect.x + w.rect.w - 20.0 && cx < w.rect.x + w.rect.w);
            assert!(cy > w.rect.y && cy < w.rect.y + 20.0);
            assert_eq!((w.remove.w, w.remove.h), (24.0, 24.0));
            // The target is centred on the cross and stays inside the wagon.
            assert_eq!(w.remove.x + 12.0, cx);
            assert_eq!(w.remove.y + 12.0, cy);
            assert!(w.remove.x + w.remove.w <= w.rect.x + w.rect.w);
            assert!(w.remove.y >= w.rect.y);
        }
    }
}

#[test]
fn the_clear_word_sits_at_the_right_end_with_the_undo_word_before_it() {
    let l = layout(3);
    assert_eq!(l.clear.x + l.clear.w, RIGHT);
    assert!(l.undo.x + l.undo.w < l.clear.x);
    assert_eq!(l.undo.x, UNDO_X);
}
