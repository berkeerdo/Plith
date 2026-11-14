use super::*;
use std::fs;

#[test]
fn stamp_is_zero_padded() {
    let stamp = Stamp {
        year: 2026,
        month: 11,
        day: 4,
        hour: 9,
        minute: 5,
        second: 3,
        millis: 7,
    };
    assert_eq!(format_stamp(stamp), "2026-11-04 09:05:03.007");
}

fn is_timestamp(prefix: &str) -> bool {
    let bytes = prefix.as_bytes();
    let shape = "dddd-dd-dd dd:dd:dd.ddd";
    bytes.len() == shape.len()
        && shape.bytes().zip(bytes).all(|(want, got)| match want {
            b'd' => got.is_ascii_digit(),
            other => *got == other,
        })
}

#[test]
fn line_has_timestamp_and_appends() {
    let dir = std::env::temp_dir().join(format!("plith-log-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plith.log");
    {
        let log = LogFile::open_at(&path).unwrap();
        log.line("first");
        log.line("second");
    }
    {
        let log = LogFile::open_at(&path).unwrap();
        log.line("third");
    }
    let text = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "reopening must append, not truncate");
    for (line, want) in lines.iter().zip(["first", "second", "third"]) {
        let (prefix, rest) = line.split_at(23);
        assert!(is_timestamp(prefix), "bad prefix: {prefix:?}");
        assert_eq!(rest, format!(" {want}"));
    }
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&dir).unwrap();
}

#[test]
fn lines_keep_order_and_flush_on_drop() {
    let dir = std::env::temp_dir().join(format!("plith-log-order-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plith.log");
    {
        let log = std::sync::Arc::new(LogFile::open_at(&path).unwrap());
        let other = std::sync::Arc::clone(&log);
        let helper = std::thread::spawn(move || {
            for n in 0..200 {
                other.line(&format!("helper {n}"));
            }
        });
        for n in 0..200 {
            log.line(&format!("main {n}"));
        }
        helper.join().unwrap();
        log.line("exit 0");
    }
    let text = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().map(|line| &line[24..]).collect();
    assert_eq!(lines.len(), 401, "every line is on disk after the drop");
    assert_eq!(lines.last(), Some(&"exit 0"));
    for who in ["main", "helper"] {
        let own: Vec<&&str> = lines.iter().filter(|l| l.starts_with(who)).collect();
        for (n, line) in own.iter().enumerate() {
            assert_eq!(**line, format!("{who} {n}"), "order within one thread");
        }
    }
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&dir).unwrap();
}
