use super::is_new_size;

const NOW: (u32, u32, f64) = (840, 640, 1.0);

#[test]
fn new_size_needs_pixels_and_a_usable_scale() {
    assert!(is_new_size((1260, 960, 1.5), NOW));
    assert!(is_new_size((840, 640, 1.25), NOW));
    assert!(!is_new_size(NOW, NOW));
    assert!(!is_new_size((0, 640, 1.0), NOW));
    assert!(!is_new_size((840, 0, 1.0), NOW));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(!is_new_size((1260, 960, scale), NOW), "{scale}");
    }
}
