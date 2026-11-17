use super::*;

fn unique(tag: &str) -> String {
    format!("Local\\Plith.Test.{tag}.{}", std::process::id())
}

#[test]
fn second_acquire_in_process_sees_the_first() {
    let name = unique("second");
    let first = acquire(&name).unwrap();
    assert!(first.is_some());
    assert!(acquire(&name).unwrap().is_none());
}

#[test]
fn drop_releases() {
    let name = unique("drop");
    let first = acquire(&name).unwrap();
    assert!(first.is_some());
    drop(first);
    assert!(acquire(&name).unwrap().is_some());
}
