use super::*;

fn chan(label: &str, muted: bool, routes: &[bool]) -> ChannelFacts {
    ChannelFacts {
        label: label.to_string(),
        gain_db: -3.0,
        muted,
        routes: routes.to_vec(),
    }
}

fn banana(strips: Vec<ChannelFacts>) -> AudioFacts {
    let buses = vec![
        chan("A1", false, &[]),
        chan("A2", false, &[]),
        chan("A3", false, &[]),
        chan("B1", false, &[]),
        chan("B2", false, &[]),
    ];
    AudioFacts {
        vm: Some(VmFacts {
            strips,
            buses,
            hardware_strips: 3,
            hardware_buses: 3,
        }),
        ..AudioFacts::default()
    }
}

fn five_strips(first_routes: &[bool]) -> Vec<ChannelFacts> {
    let mut v = vec![chan("Mic", false, first_routes)];
    for n in 2..=5 {
        v.push(chan(&format!("S{n}"), false, &[false; 5]));
    }
    v
}

fn active(strips: usize, hot: usize, now: f64) -> Activity {
    let mut levels = vec![[0.0_f32; 2]; strips];
    levels[hot] = [0.5, 0.1];
    let mut a = Activity::default();
    a.observe(&levels, now);
    a
}

#[test]
fn routed_with_signal_is_live() {
    let facts = banana(five_strips(&[true, false, false, false, false]));
    let view = audio_view(&facts, &active(5, 0, 10.0), PickMode::None, 10.5);
    assert_eq!(
        view.routes,
        vec![Route {
            source: 0,
            output: 0,
            state: RouteState::Live
        }]
    );
    assert!(view.outputs[0].live);
    assert!(!view.outputs[1].live);
    assert_eq!(view.footer, Footer::Normal);
}

#[test]
fn routed_without_signal_is_idle() {
    let facts = banana(five_strips(&[true, false, false, false, false]));
    let silent = Activity::default();
    let view = audio_view(&facts, &silent, PickMode::None, 10.0);
    assert!(
        view.routes.is_empty(),
        "idle routes are dropped unless picked"
    );
    let view = audio_view(&facts, &silent, PickMode::Picked(0), 10.0);
    assert!(view.routes.contains(&Route {
        source: 0,
        output: 0,
        state: RouteState::Idle
    }));
}

#[test]
fn muted_source_is_cut() {
    let mut strips = five_strips(&[true, false, false, false, false]);
    strips[0].muted = true;
    let facts = banana(strips);
    let view = audio_view(&facts, &active(5, 0, 10.0), PickMode::None, 10.0);
    assert_eq!(view.routes[0].state, RouteState::Cut);
    assert!(view.sources[0].muted);
}

#[test]
fn muted_bus_is_not_live() {
    let mut facts = banana(five_strips(&[true, false, false, false, false]));
    facts.vm.as_mut().unwrap().buses[0].muted = true;
    let view = audio_view(&facts, &active(5, 0, 10.0), PickMode::Picked(0), 10.0);
    assert_eq!(view.routes[0].state, RouteState::Cut);
}

#[test]
fn pick_mode_marks_unrouted_outputs_as_candidates() {
    let facts = banana(five_strips(&[true, false, false, false, false]));
    let view = audio_view(&facts, &Activity::default(), PickMode::Picked(0), 0.0);
    let cands: Vec<usize> = view
        .routes
        .iter()
        .filter(|r| r.state == RouteState::Candidate)
        .map(|r| r.output)
        .collect();
    assert_eq!(cands, vec![1, 2, 3, 4]);
    assert!(view.routes.iter().all(|r| r.source == 0));
    assert!(view.sources[0].picked);
    assert_eq!(view.mode, PickMode::Picked(0));
    assert_eq!(view.footer, Footer::Picking);
}

#[test]
fn no_voicemeeter_shows_windows_outputs() {
    let facts = AudioFacts {
        vm: None,
        endpoints: vec![
            EndpointFacts {
                id: "a".into(),
                name: "Speakers".into(),
            },
            EndpointFacts {
                id: "b".into(),
                name: "Headset".into(),
            },
        ],
        default_id: Some("b".into()),
        attached: Some((0.456, false)),
        mic: None,
    };
    let view = audio_view(&facts, &Activity::default(), PickMode::None, 0.0);
    assert_eq!(view.sources.len(), 1);
    assert_eq!(view.sources[0].name, "Windows");
    assert_eq!(view.sources[0].sub, "VARSAYILAN ÇIKIŞ");
    assert_eq!(view.outputs.len(), 2);
    assert_eq!(view.outputs[0].value, OutputValue::None);
    assert_eq!(view.outputs[1].value, OutputValue::Percent(46));
    assert_eq!(view.outputs[1].tag, None);
    assert_eq!(view.outputs[0].id.as_deref(), Some("a"));
    assert_eq!(view.outputs[1].id.as_deref(), Some("b"));
    assert_eq!(
        view.routes,
        vec![Route {
            source: 0,
            output: 1,
            state: RouteState::Live
        }]
    );
    assert_eq!(view.footer, Footer::NoVoicemeeter);
    assert_eq!(
        Footer::NoVoicemeeter.text(),
        "VOICEMEETER ÇALIŞMIYOR \u{B7} ÇIKIŞA TIKLA: VARSAYILAN OLUR"
    );
}

#[test]
fn no_voicemeeter_muted_default_is_cut() {
    let facts = AudioFacts {
        endpoints: vec![EndpointFacts {
            id: "a".into(),
            name: "Speakers".into(),
        }],
        default_id: Some("a".into()),
        attached: Some((0.5, true)),
        ..AudioFacts::default()
    };
    let view = audio_view(&facts, &Activity::default(), PickMode::None, 0.0);
    assert_eq!(view.routes[0].state, RouteState::Cut);
}

#[test]
fn potato_rows_shrink_pitch() {
    let strips: Vec<ChannelFacts> = (1..=8)
        .map(|n| chan(&format!("S{n}"), false, &[]))
        .collect();
    let buses: Vec<ChannelFacts> = (1..=8)
        .map(|n| chan(&format!("X{n}"), false, &[]))
        .collect();
    let facts = AudioFacts {
        vm: Some(VmFacts {
            strips,
            buses,
            hardware_strips: 5,
            hardware_buses: 5,
        }),
        ..AudioFacts::default()
    };
    let view = audio_view(&facts, &Activity::default(), PickMode::None, 0.0);
    assert!((view.row_pitch - 40.0).abs() < 1e-9);
    assert_eq!(view.sources[4].sub, "HW 5");
    assert_eq!(view.sources[5].sub, "VAIO");
    assert_eq!(view.sources[6].sub, "AUX");
    assert_eq!(view.sources[7].sub, "VAIO3");
    assert_eq!(view.outputs[4].tag.as_deref(), Some("A5"));
    assert_eq!(view.outputs[5].tag.as_deref(), Some("B1"));
    let banana_view = audio_view(
        &banana(five_strips(&[])),
        &Activity::default(),
        PickMode::None,
        0.0,
    );
    assert!((banana_view.row_pitch - 57.0).abs() < 1e-9);
}

#[test]
fn bus_names() {
    let mut facts = banana(five_strips(&[true, false, false, false, false]));
    facts.vm.as_mut().unwrap().buses[1].label = "Headset".into();
    let view = audio_view(&facts, &Activity::default(), PickMode::None, 0.0);
    assert_eq!(view.outputs[1].name, "Headset");
    assert_eq!(view.outputs[2].name, "Kullanılmıyor");
    assert_eq!(view.outputs[0].name, "A1");
    assert_eq!(view.outputs[0].value, OutputValue::Db(-3.0));
}

#[test]
fn activity_holds_two_seconds() {
    let mut a = Activity::default();
    assert!(!a.has_signal(0, 0.0));
    a.observe(&[[0.2, 0.0]], 5.0);
    assert!(a.has_signal(0, 6.9));
    assert!(!a.has_signal(0, 7.1));
    a.observe(&[[0.005, 0.005]], 6.0);
    assert!(
        !a.has_signal(0, 7.1),
        "quiet frames do not refresh the hold"
    );
    assert!(!a.has_signal(3, 6.0));
}

#[test]
fn empty_label_uses_fallback() {
    let mut strips = five_strips(&[]);
    strips[2].label = String::new();
    let view = audio_view(&banana(strips), &Activity::default(), PickMode::None, 0.0);
    assert_eq!(view.sources[2].name, "Kaynak 3");
    assert_eq!(view.sources[2].sub, "HW 3");
    assert_eq!(view.sources[3].sub, "VAIO");
}

#[test]
fn footer_copy() {
    assert_eq!(Footer::Normal.text(), "KAYNAĞA, SONRA ÇIKIŞA TIKLA");
    assert_eq!(
        Footer::Picking.text(),
        "ÇIKIŞA TIKLA: YOL KURULUR \u{B7} KAYNAĞA TEKRAR TIKLA: VAZGEÇ"
    );
}

#[test]
fn muted_bus_route_is_cut_and_output_muted() {
    let mut facts = banana(five_strips(&[true, false, false, false, false]));
    facts.vm.as_mut().unwrap().buses[0].muted = true;
    let view = audio_view(&facts, &active(5, 0, 10.0), PickMode::None, 10.0);
    assert_eq!(
        view.routes,
        vec![Route {
            source: 0,
            output: 0,
            state: RouteState::Cut
        }]
    );
    assert!(view.outputs[0].muted);
    assert!(!view.outputs[1].muted);
}

#[test]
fn no_voicemeeter_ignores_pick_mode() {
    let facts = AudioFacts {
        endpoints: vec![EndpointFacts {
            id: "a".into(),
            name: "Speakers".into(),
        }],
        default_id: Some("a".into()),
        attached: Some((0.5, false)),
        ..AudioFacts::default()
    };
    let view = audio_view(&facts, &Activity::default(), PickMode::Picked(0), 0.0);
    assert_eq!(view.mode, PickMode::None);
    assert!(!view.sources[0].picked);
    assert!(view.routes.iter().all(|r| r.state != RouteState::Candidate));
    assert_eq!(view.footer, Footer::NoVoicemeeter);
}

#[test]
fn no_voicemeeter_muted_default_mutes_source_and_output() {
    let facts = AudioFacts {
        endpoints: vec![EndpointFacts {
            id: "a".into(),
            name: "Speakers".into(),
        }],
        default_id: Some("a".into()),
        attached: Some((0.5, true)),
        ..AudioFacts::default()
    };
    let view = audio_view(&facts, &Activity::default(), PickMode::None, 0.0);
    assert!(view.sources[0].muted);
    assert!(view.outputs[0].muted);
}

#[test]
fn out_of_range_pick_is_normalised() {
    let facts = banana(five_strips(&[]));
    let view = audio_view(&facts, &Activity::default(), PickMode::Picked(99), 0.0);
    assert_eq!(view.mode, PickMode::None);
    assert_eq!(view.footer, Footer::Normal);
}
