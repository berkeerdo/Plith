use super::*;
use plith_core::settings::{IdleLook, serialize};

fn folder(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("plith-settings-{name}-{}", std::process::id()))
}

fn log_in(dir: &Path) -> LogFile {
    LogFile::open_at(&dir.join("plith.log")).unwrap()
}

#[test]
fn missing_settings_file_gives_defaults_and_writes_one() {
    let dir = folder("missing");
    let path = dir.join("settings.ini");
    let log = log_in(&dir);
    assert_eq!(load_at(&path, &log), Settings::default());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        serialize(&Settings::default())
    );
}

#[test]
fn a_bad_line_is_skipped_and_the_rest_is_read() {
    let dir = folder("bad");
    let path = dir.join("settings.ini");
    let log = log_in(&dir);
    std::fs::write(&path, "look=clock\nnonsense\nspan=forever\n").unwrap();
    let settings = load_at(&path, &log);
    assert_eq!(settings.look, IdleLook::Clock);
    assert_eq!(settings.span, Settings::default().span);
    log.flush();
    let text = std::fs::read_to_string(dir.join("plith.log")).unwrap();
    assert!(text.contains("settings: line 2"), "{text}");
    assert!(text.contains("settings: line 3"), "{text}");
}

#[test]
fn saving_strips_line_breaks_from_the_city() {
    let dir = folder("city");
    let path = dir.join("settings.ini");
    let settings = Settings {
        city: "Ist\r\nanbul\n".into(),
        ..Settings::default()
    };
    save_at(&path, &settings).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("city=Istanbul\n"), "{text}");
    assert_eq!(text.lines().count(), 13);
}

#[test]
fn save_is_atomic() {
    let dir = folder("atomic");
    let path = dir.join("settings.ini");
    let log = log_in(&dir);
    let first = Settings {
        osd: false,
        ..Settings::default()
    };
    save_at(&path, &first).unwrap();
    assert!(!dir.join("settings.ini.tmp").exists());
    assert_eq!(load_at(&path, &log), first);
    let second = Settings {
        city: "Izmir".to_string(),
        ..first
    };
    save_at(&path, &second).unwrap();
    assert!(!dir.join("settings.ini.tmp").exists());
    assert_eq!(load_at(&path, &log), second);
}
