use super::*;

#[test]
fn top_centre_scales_and_centres_on_the_monitor_top() {
    let monitor = RectPx {
        x: -2560,
        y: 0,
        width: 2560,
        height: 1440,
    };
    let rect = top_centre(monitor, 1.5, 360.0, 96.0);
    assert_eq!(
        rect,
        RectPx {
            x: -2560 + (2560 - 540) / 2,
            y: 0,
            width: 540,
            height: 144,
        }
    );
}

#[test]
fn creation_words_match_the_command_line() {
    assert_eq!(Creation::InBand.as_str(), "inband");
    assert_eq!(Creation::SetBand.as_str(), "setband");
    assert_eq!(Creation::Plain.as_str(), "plain");
}
