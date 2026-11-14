//! Render endpoint sections: volume, mute and the default device switch.

use plith_audio::endpoints::{DefaultRoles, EndpointService, read_default_roles};

use crate::support::{Guard, Verdict, close, drain, event_arrived, judge, planned, yes_no};

const VOLUME_STEP: f32 = 0.02;
const VOLUME_TOLERANCE: f32 = 0.001;

pub fn endpoint_volume(service: &EndpointService, yes: bool) -> Verdict {
    let Some(before) = service.snapshot().attached else {
        return Verdict::Skipped("no attached render endpoint".into());
    };
    let original = before.volume;
    let target = if original + VOLUME_STEP <= 1.0 {
        original + VOLUME_STEP
    } else {
        original - VOLUME_STEP
    };
    println!("BEFORE: volume {original:.4} on {}", before.id);
    if !yes {
        return planned(&format!(
            "set volume {target:.4}, then restore exactly {original:.4}"
        ));
    }
    let events = service.subscribe();
    let mut guard = Guard::new(|| {
        let _ = service.set_volume(original);
    });
    drain(&events);
    let written = service.set_volume(target);
    let arrived = event_arrived(&events, |s| {
        s.attached
            .as_ref()
            .is_some_and(|a| close(a.volume, target, VOLUME_TOLERANCE))
    });
    let after = service.snapshot().attached.map(|a| a.volume);
    println!(
        "AFTER-CHANGE: write={written} volume {after:?} (wanted {target:.4}), event: {}",
        yes_no(arrived)
    );
    let changed = after.is_some_and(|v| close(v, target, VOLUME_TOLERANCE));
    drain(&events);
    guard.restore_now();
    let arrived = event_arrived(&events, |s| {
        s.attached
            .as_ref()
            .is_some_and(|a| close(a.volume, original, VOLUME_TOLERANCE))
    });
    let after = service.snapshot().attached.map(|a| a.volume);
    println!(
        "AFTER-RESTORE: volume {after:?} (original {original:.4}), event: {}",
        yes_no(arrived)
    );
    judge(
        changed,
        after.is_some_and(|v| close(v, original, VOLUME_TOLERANCE)),
    )
}

pub fn endpoint_mute(service: &EndpointService, yes: bool) -> Verdict {
    let Some(before) = service.snapshot().attached else {
        return Verdict::Skipped("no attached render endpoint".into());
    };
    let original = before.muted;
    println!("BEFORE: muted={original} on {}", before.id);
    if !yes {
        return planned(&format!("toggle mute twice, ending muted={original}"));
    }
    let events = service.subscribe();
    let muted_now = || service.snapshot().attached.map(|a| a.muted);
    let mut guard = Guard::new(|| {
        if muted_now().is_some_and(|m| m != original) {
            let _ = service.toggle_mute();
        }
    });
    drain(&events);
    let toggled = service.toggle_mute();
    let arrived = event_arrived(&events, |s| {
        s.attached.as_ref().is_some_and(|a| a.muted != original)
    });
    println!(
        "AFTER-CHANGE: toggle returned {toggled:?}, muted={:?} (wanted {}), event: {}",
        muted_now(),
        !original,
        yes_no(arrived)
    );
    let changed = muted_now() == Some(!original);
    drain(&events);
    guard.restore_now();
    let arrived = event_arrived(&events, |s| {
        s.attached.as_ref().is_some_and(|a| a.muted == original)
    });
    println!(
        "AFTER-RESTORE: muted={:?} (original {original}), event: {}",
        muted_now(),
        yes_no(arrived)
    );
    judge(changed, muted_now() == Some(original))
}

pub fn describe(roles: Option<&DefaultRoles>) -> String {
    roles.map_or_else(
        || "unreadable".to_owned(),
        |r| format!("console={:?} multimedia={:?}", r.console, r.multimedia),
    )
}

/// Writes `id` as the default and re-reads both roles, up to `attempts` times, until both
/// roles name it. Returns the last reading.
pub fn force_default(service: &EndpointService, id: &str, attempts: u32) -> Option<DefaultRoles> {
    let mut roles = read_default_roles();
    for _ in 0..attempts {
        if roles.as_ref().is_some_and(|r| r.all_are(id)) {
            break;
        }
        let _ = service.set_default_endpoint(id);
        roles = read_default_roles();
    }
    roles
}

pub fn default_endpoint(service: &EndpointService, to: &str, yes: bool) -> Verdict {
    let Some(roles) = read_default_roles() else {
        return Verdict::Skipped("default roles unreadable".into());
    };
    println!("BEFORE: {}", describe(Some(&roles)));
    let (true, Some(original)) = (roles.agree(), roles.multimedia.clone()) else {
        return Verdict::Skipped(
            "Console and Multimedia defaults differ; one switch cannot restore both".into(),
        );
    };
    let wanted = to.to_lowercase();
    let matches: Vec<_> = service
        .snapshot()
        .active
        .into_iter()
        .filter(|e| e.id.to_lowercase().contains(&wanted) && !e.id.eq_ignore_ascii_case(&original))
        .collect();
    let [target] = matches.as_slice() else {
        return Verdict::Skipped(format!(
            "--to {to:?} matched {} other active endpoints, need exactly one",
            matches.len()
        ));
    };
    let target = target.id.clone();
    println!("TARGET: {} ({target})", matches[0].friendly_name);
    if !yes {
        return planned(&format!(
            "set Console and Multimedia defaults to {target}, re-read, then set both back to {original}"
        ));
    }
    let events = service.subscribe();
    let mut guard = Guard::new(|| {
        let _ = force_default(service, &original, 3);
    });
    drain(&events);
    let done = service.set_default_endpoint(&target);
    let arrived = event_arrived(&events, |s| {
        s.default_id
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case(&target))
    });
    let after = read_default_roles();
    println!(
        "AFTER-CHANGE: write={done} {}, event: {}",
        describe(after.as_ref()),
        yes_no(arrived)
    );
    let changed = after.as_ref().is_some_and(|r| r.all_are(&target));
    if !changed {
        println!("PARTIAL or failed switch: restoring both roles below");
    }
    drain(&events);
    guard.restore_now();
    let arrived = event_arrived(&events, |s| {
        s.default_id
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case(&original))
    });
    let restored = read_default_roles();
    println!(
        "AFTER-RESTORE: {}, event: {}",
        describe(restored.as_ref()),
        yes_no(arrived)
    );
    judge(changed, restored.is_some_and(|r| r.all_are(&original)))
}
