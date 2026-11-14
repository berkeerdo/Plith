use super::*;

#[test]
fn default_is_the_r1b_pick() {
    let levers = Levers::default();
    assert_eq!(levers.gpu_priority, 0);
    assert_eq!(levers.sched_class, SchedClass::Normal);
    assert_eq!(levers.latency, 2);
    assert_eq!(levers.sync, 1);
    assert_eq!(levers.mmcss, MmcssPriority::Normal);
}

#[test]
fn describe_lists_all_five() {
    assert_eq!(
        Levers::default().describe(),
        "gpu-priority 0, sched-class normal, latency 2, sync 1, mmcss normal"
    );
    let levers = Levers {
        gpu_priority: -3,
        sched_class: SchedClass::AboveNormal,
        latency: 3,
        sync: 0,
        mmcss: MmcssPriority::Critical,
    };
    assert_eq!(
        levers.describe(),
        "gpu-priority -3, sched-class above, latency 3, sync 0, mmcss critical"
    );
}

#[test]
fn refused_lever_is_logged_not_fatal() {
    let refused = LeverOutcome {
        name: "gpu-priority",
        setting: "7".to_string(),
        result: Err("0x80070005".to_string()),
        fallback: None,
    };
    assert_eq!(
        outcome_line(&refused),
        "lever gpu-priority 7: refused 0x80070005"
    );
    let ok = LeverOutcome {
        result: Ok(()),
        ..refused
    };
    assert_eq!(outcome_line(&ok), "lever gpu-priority 7: ok");
}

#[test]
fn refused_latency_names_the_fallback() {
    let mut refused = LeverOutcome {
        name: "latency",
        setting: "3".to_string(),
        result: Err("0x887A0001".to_string()),
        fallback: Some(LeverFallback {
            setting: "2".to_string(),
            result: Ok(()),
        }),
    };
    assert_eq!(
        outcome_line(&refused),
        "lever latency 3: refused 0x887A0001, using 2"
    );
    refused.fallback = Some(LeverFallback {
        setting: "2".to_string(),
        result: Err("0x80070057".to_string()),
    });
    assert_eq!(
        outcome_line(&refused),
        "lever latency 3: refused 0x887A0001, fallback failed 0x80070057"
    );
    // A fallback after an applied lever is never reported.
    refused.result = Ok(());
    assert_eq!(outcome_line(&refused), "lever latency 3: ok");
}
