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
    let bare = LATEST.replace("v0.5.0", "0.5.0");
    let bare = parse_release(&bare).unwrap();
    assert_eq!(bare.version, release.version);
    assert_eq!(
        bare.page_url,
        "https://github.com/berkeerdo/Plith/releases/tag/0.5.0"
    );
}

#[test]
fn hostile_release_urls_are_rejected() {
    let good = format!("{BASE}Plith-Setup-0.5.0.exe");
    let page = "https://github.com/berkeerdo/Plith/releases/tag/v0.5.0";
    let download_cases = [
        format!("{BASE}../../evil/Plith-Setup-0.5.0.exe"),
        format!("{BASE}%2e%2e/Plith-Setup-0.5.0.exe"),
        format!("{BASE}..\\Plith-Setup-0.5.0.exe"),
        format!("{good}?x=1"),
        format!("{good}#x"),
        format!("{good}/"),
        good.replace("berkeerdo/Plith", "berkeerdo/Other"),
        good.replace("berkeerdo", "Berkeerdo"),
        good.replace("Plith-Setup-0.5.0.exe", "Plith-Setup-0.4.0.exe"),
        good.replace("https://github.com", "https://github.com@evil.com"),
    ];
    for url in download_cases {
        let json = LATEST.replace(&good, &url);
        assert!(parse_release(&json).is_none(), "{url}");
    }
    let page_cases = [
        format!("{page}?x"),
        format!("{page}#x"),
        format!("{page}/../x"),
        page.replace("Plith", "Other"),
        "https://github.com/berkeerdo/Plith/releases/".to_string(),
    ];
    for url in page_cases {
        let json = LATEST.replace(page, &url);
        assert!(parse_release(&json).is_none(), "{url}");
    }
    for tag in ["v0.5.0/x", "v0.5.0%2e", "..", "v0.5.0\\\\x", "v0.5.0 "] {
        let json = LATEST.replace(
            "\"tag_name\": \"v0.5.0\"",
            &format!("\"tag_name\": \"{tag}\""),
        );
        assert!(parse_release(&json).is_none(), "{tag}");
    }
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
    // The clock went backwards: the record is ignored.
    assert_eq!(next_check_in(now, started, now + 5_000, None), 0);
    assert_eq!(next_check_in(now, started, 0, Some(now + 5_000)), 0);
}

#[test]
fn future_record_still_waits_two_minutes_after_start() {
    assert_eq!(next_check_in(1_000, 1_000, 5_000, None), 120);
    assert_eq!(next_check_in(1_000, 1_000, 0, Some(5_000)), 120);
    assert_eq!(next_check_in(1_000, 1_000, 5_000, Some(6_000)), 120);
    // A valid failure survives a future success record.
    assert_eq!(next_check_in(1_100, 1_000, 5_000, Some(1_050)), 21_550);
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
