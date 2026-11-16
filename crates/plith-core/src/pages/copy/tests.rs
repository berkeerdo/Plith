use super::*;

#[test]
fn turkish_letters_follow_turkish_rules() {
    assert_eq!(tr_upper("parlaklık"), "PARLAKLIK");
    assert_eq!(tr_upper("sistem"), "S\u{130}STEM");
    assert_eq!(
        tr_upper("çıkış iğne öğe ü"),
        "\u{c7}IKI\u{15e} \u{130}\u{11e}NE \u{d6}\u{11e}E \u{dc}"
    );
    assert_eq!(tr_upper("DDC/CI 4:03"), "DDC/CI 4:03");
}

#[test]
fn own_labels_are_authored_upper_cased() {
    for label in OWN_LABELS {
        assert_eq!(*label, tr_upper(label), "{label}");
    }
    assert!(OWN_LABELS.contains(&"S\u{130}STEM"));
    assert!(OWN_LABELS.contains(&KAPALI));
}
