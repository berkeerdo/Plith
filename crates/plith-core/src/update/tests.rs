use super::*;

const BASE: &str = "https://github.com/berkeerdo/Plith/releases/download/v0.5.0/";

const LATEST: &str = r#"{
  "url": "https://api.github.com/repos/berkeerdo/Plith/releases/1",
  "html_url": "https://github.com/berkeerdo/Plith/releases/tag/v0.5.0",
  "tag_name": "v0.5.0",
  "draft": false,
  "prerelease": false,
  "assets": [
    {"name": "Plith-Setup-0.4.0.exe", "size": 9,
     "browser_download_url": "https://github.com/berkeerdo/Plith/releases/download/v0.5.0/Plith-Setup-0.4.0.exe"},
    {"name": "Plith-Setup-0.5.0.exe", "size": 4521984,
     "browser_download_url": "https://github.com/berkeerdo/Plith/releases/download/v0.5.0/Plith-Setup-0.5.0.exe"}
  ]
}"#;

#[test]
fn parses_github_latest_with_setup_asset() {
    let release = parse_release(LATEST).unwrap();
    assert_eq!(release.version, Version::new(0, 5, 0));
    assert_eq!(release.setup_url, format!("{BASE}Plith-Setup-0.5.0.exe"));
    assert_eq!(
        release.page_url,
        "https://github.com/berkeerdo/Plith/releases/tag/v0.5.0"
    );
    let bare = LATEST.replace("\"v0.5.0\"", "\"0.5.0\"");
    assert_eq!(parse_release(&bare).unwrap().version, release.version);
}

#[test]
fn release_without_asset_is_none() {
    assert!(parse_release("not json").is_none());
    assert!(parse_release(r#"{"tag_name":"v0.5.0"}"#).is_none());
    let renamed = LATEST.replace(
        "\"Plith-Setup-0.5.0.exe\", \"size\"",
        "\"Plith-0.5.0.exe\", \"size\"",
    );
    assert!(parse_release(&renamed).is_none());
    let wrong_host = LATEST.replace(
        &format!("{BASE}Plith-Setup-0.5.0"),
        "https://example.com/Plith-Setup-0.5.0",
    );
    assert!(parse_release(&wrong_host).is_none());
    let wrong_page = LATEST.replace(
        "https://github.com/berkeerdo/Plith/releases/tag",
        "https://example.com/tag",
    );
    assert!(parse_release(&wrong_page).is_none());
}

#[test]
fn first_check_two_minutes_after_start() {
    assert_eq!(next_check_in(1_000, 1_000, 0, None), 120);
    assert_eq!(next_check_in(1_090, 1_000, 0, None), 30);
    assert_eq!(next_check_in(5_000, 1_000, 0, None), 0);
    // A check a day ago is due, but never before two minutes after start.
    assert_eq!(
        next_check_in(1_000_000, 999_990, 1_000_000 - 90_000, None),
        110
    );
}

#[test]
fn daily_after_success_six_hours_after_failure() {
    let now = 2_000_000;
    let started = now - 10_000;
    assert_eq!(next_check_in(now, started, now - 3_600, None), 82_800);
    assert_eq!(next_check_in(now, started, now - 90_000, None), 0);
    assert_eq!(next_check_in(now, started, 0, Some(now - 600)), 21_000);
    assert_eq!(
        next_check_in(now, started, now - 7_200, Some(now - 600)),
        21_000
    );
    // A failure older than the last success does not matter.
    assert_eq!(
        next_check_in(now, started, now - 600, Some(now - 7_200)),
        85_800
    );
    // The clock went backwards: check now.
    assert_eq!(next_check_in(now, started, now + 5_000, None), 0);
    assert_eq!(next_check_in(now, started, 0, Some(now + 5_000)), 0);
}

#[test]
fn notice_once_per_version() {
    let own = Version::new(0, 4, 0);
    let new = Version::new(0, 5, 0);
    assert!(should_notice(&new, own, ""));
    assert!(should_notice(&new, own, "0.4.1"));
    assert!(should_notice(&new, own, "garbage"));
    assert!(!should_notice(&new, own, "0.5.0"));
    assert!(!should_notice(&new, own, "v0.5.0"));
    assert!(!should_notice(&own, own, ""));
    assert!(!should_notice(&Version::new(0, 3, 0), own, ""));
}
