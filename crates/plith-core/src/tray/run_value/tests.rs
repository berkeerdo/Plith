use super::*;

#[test]
fn the_value_is_the_quoted_full_path() {
    assert_eq!(
        run_value(r"C:\Program Files\Plith\plith.exe"),
        r#""C:\Program Files\Plith\plith.exe""#
    );
}

#[test]
fn what_we_write_is_ours() {
    for exe in [
        r"C:\Program Files\Plith\plith.exe",
        r"D:\tools\plith.exe",
        r"C:\Users\a b\AppData\Local\Plith\plith.exe",
    ] {
        assert!(is_ours(&run_value(exe)), "{exe}");
        assert_eq!(program_of(&run_value(exe)), exe);
    }
}

#[test]
fn the_wpf_value_is_ours_whatever_its_case() {
    assert!(is_ours(r#""C:\Program Files\Plith\Plith.exe""#));
    assert!(is_ours(r#""C:\PROGRAM FILES\PLITH\PLITH.EXE""#));
}

#[test]
fn an_unquoted_value_and_arguments_are_read() {
    assert!(is_ours(r"C:\Plith\plith.exe"));
    assert_eq!(
        program_of(r"C:\Plith\plith.exe --osd"),
        r"C:\Plith\plith.exe"
    );
    assert_eq!(
        program_of(r#""C:\a b\plith.exe" --osd"#),
        r"C:\a b\plith.exe"
    );
    assert!(is_ours(r#""C:\a b\plith.exe" --osd"#));
}

#[test]
fn another_program_is_not_ours() {
    assert!(!is_ours(r#""C:\Program Files\Other\other.exe""#));
    assert!(!is_ours(r#""C:\Plith\plith-osd.exe""#));
    assert!(!is_ours(r#""C:\Plith\notplith.exe""#));
    assert!(!is_ours(r"C:\Plith\plith.exe.bak"));
}

#[test]
fn empty_and_broken_values_are_not_ours() {
    assert!(!is_ours(""));
    assert!(!is_ours("   "));
    assert!(!is_ours("\""));
    assert!(!is_ours("\"\""));
}

#[test]
fn forward_slashes_and_a_bare_name_count() {
    assert!(is_ours("C:/Plith/plith.exe"));
    assert!(is_ours("plith.exe"));
}

#[test]
fn the_key_and_the_name_are_the_ones_wpf_used() {
    assert_eq!(RUN_KEY, r"Software\Microsoft\Windows\CurrentVersion\Run");
    assert_eq!(RUN_VALUE, "Plith");
}
