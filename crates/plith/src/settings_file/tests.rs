use super::*;
use plith_core::settings::{IdleLook, serialize};
use plith_core::theme::registry::ThemeId;

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
    assert_eq!(text.lines().count(), 17);
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

#[test]
fn a_failed_rename_leaves_no_temp_file() {
    let dir = folder("rename");
    // A folder where the file should be: the rename onto it fails.
    let path = dir.join("settings.ini");
    std::fs::create_dir_all(&path).unwrap();
    assert!(save_at(&path, &Settings::default()).is_err());
    assert!(!dir.join("settings.ini.tmp").exists());
}

#[test]
fn import_runs_only_without_settings_ini() {
    let dir = folder("import-skip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.ini");
    std::fs::write(dir.join("config.ini"), "[General]\nTheme = Light\n").unwrap();
    std::fs::write(&path, "look=clock\n").unwrap();
    let settings = load_at(&path, &log_in(&dir));
    assert_eq!(settings.look, IdleLook::Clock);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "look=clock\n");
}

#[test]
fn a_retired_theme_rewrites_the_file_once_with_the_new_keys() {
    let dir = folder("migrate");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.ini");
    std::fs::write(&path, "theme=sodyum\nlook=clock\n").unwrap();
    let log = log_in(&dir);
    let settings = load_at(&path, &log);
    assert_eq!(settings.theme, ThemeId::gece());
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("theme=gece\n"), "{written}");
    assert!(written.contains("accent.gece=#FFB224\n"), "{written}");
    assert!(!written.contains("sodyum"), "{written}");
    assert!(!dir.join("settings.ini.tmp").exists());
    // The second start finds nothing to move and leaves the file as it is.
    assert_eq!(load_at(&path, &log), settings);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
    log.flush();
    let text = std::fs::read_to_string(dir.join("plith.log")).unwrap();
    assert!(
        text.contains("settings: theme sodyum moved to gece"),
        "{text}"
    );
}

#[test]
fn another_problem_does_not_rewrite_the_file() {
    let dir = folder("no-migrate");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.ini");
    std::fs::write(&path, "look=clock\nnonsense\n").unwrap();
    let _ = load_at(&path, &log_in(&dir));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "look=clock\nnonsense\n"
    );
}

#[test]
fn import_runs_when_only_config_ini_exists() {
    let dir = folder("import-run");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.ini");
    let config = "[General]\nTheme = Light\n[Osd]\nShowWeather = False\n";
    std::fs::write(dir.join("config.ini"), config).unwrap();
    let log = log_in(&dir);
    let settings = load_at(&path, &log);
    assert_eq!(settings.theme, ThemeId::gece());
    assert_eq!(settings.floors[&ThemeId::gece()].to_hex(), "#F3F2ED");
    assert!(!settings.weather);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("floor.gece=#F3F2ED\n"), "{written}");
    assert_eq!(
        std::fs::read_to_string(dir.join("config.ini")).unwrap(),
        config
    );
    log.flush();
    let text = std::fs::read_to_string(dir.join("plith.log")).unwrap();
    assert!(text.contains("import: wpf config.ini, 2 keys"), "{text}");
}
