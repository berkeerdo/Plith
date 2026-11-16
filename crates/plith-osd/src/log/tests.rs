use super::*;

#[test]
fn a_log_without_a_file_ignores_lines() {
    let log = Log::closed();
    log.line("dropped");
}

#[test]
fn the_known_folder_is_a_real_absolute_path() {
    let folder = local_app_data().expect("LocalAppData");
    assert!(folder.is_absolute(), "{}", folder.display());
    assert!(!is_reparse_point(&folder));
}

#[test]
fn a_temp_file_reports_its_own_final_path() {
    let path = std::env::temp_dir().join(format!("plith-osd-log-test-{}.txt", std::process::id()));
    let file = File::create(&path).expect("create");
    let resolved = final_path(&file).expect("final path");
    let expected = fs::canonicalize(&path).expect("canonical");
    assert!(
        same_final_path(&resolved, &expected.to_string_lossy()),
        "{resolved} vs {}",
        expected.display()
    );
    drop(file);
    let _ = fs::remove_file(&path);
}
