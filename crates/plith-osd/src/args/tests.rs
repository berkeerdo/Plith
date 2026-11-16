use super::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn spike_defaults_to_inband_20s() {
    let args = parse(&strings(&["--spike"])).unwrap();
    assert_eq!(args.spike, Some(Creation::InBand));
    assert_eq!(args.seconds, 20);
    assert_eq!(args.pipe, None);
}

#[test]
fn no_flags_means_no_spike() {
    let args = parse(&[]).unwrap();
    assert_eq!(args.spike, None);
    assert_eq!(args.seconds, 20);
}

#[test]
fn spike_mode_seconds_and_pipe_are_read() {
    let args = parse(&strings(&[
        "--spike",
        "setband",
        "--seconds",
        "3",
        "--pipe",
        "plith-osd",
    ]))
    .unwrap();
    assert_eq!(args.spike, Some(Creation::SetBand));
    assert_eq!(args.seconds, 3);
    assert_eq!(args.pipe.as_deref(), Some("plith-osd"));
    let plain = parse(&strings(&["--seconds", "120", "--spike", "plain"])).unwrap();
    assert_eq!(plain.spike, Some(Creation::Plain));
    assert_eq!(plain.seconds, 120);
}

#[test]
fn seconds_out_of_range_is_rejected() {
    assert!(parse(&strings(&["--seconds", "0"])).is_err());
    assert!(parse(&strings(&["--seconds", "121"])).is_err());
    assert!(parse(&strings(&["--seconds", "x"])).is_err());
    assert!(parse(&strings(&["--seconds"])).is_err());
}

#[test]
fn unknown_flag_is_rejected() {
    assert!(parse(&strings(&["--bogus"])).is_err());
    assert!(parse(&strings(&["--spike", "sideways"])).is_err());
    assert!(parse(&strings(&["--pipe"])).is_err());
}
