//! Small decisions the app makes when it turns service readings into page facts: which name is
//! short, which bus the microphone feeds, what the pill shows and how the media source reads.

use std::time::SystemTime;

use crate::draw::SeekTimeline;
use crate::pages::audio::{AudioFacts, VmFacts};
use crate::pages::pill::{PillFacts, PillState};

/// Peak level above which a strip counts as carrying signal (the audio model's threshold).
pub const SIGNAL_PEAK: f32 = 0.01;

/// The name shown for an endpoint: the device description when there is one, else the
/// endpoint name. Trimming to fit is the renderer's job.
#[must_use]
pub fn short_name(device_name: &str, friendly_name: &str) -> String {
    let device = device_name.trim();
    if device.is_empty() {
        friendly_name.trim().to_string()
    } else {
        device.to_string()
    }
}

/// The tag of bus `bus`: `A1..` for hardware buses, then `B1..`.
#[must_use]
pub fn bus_tag(vm: &VmFacts, bus: usize) -> String {
    if bus < vm.hardware_buses {
        format!("A{}", bus + 1)
    } else {
        format!("B{}", bus - vm.hardware_buses + 1)
    }
}

/// The bus the microphone feeds: the tag of the first bus the first hardware strip routes to.
/// The first hardware strip is taken to be the microphone, as in the mock.
#[must_use]
pub fn mic_bus(vm: &VmFacts) -> Option<String> {
    let strip = vm.strips.first()?;
    let bus = strip.routes.iter().position(|routed| *routed)?;
    Some(bus_tag(vm, bus))
}

/// The caption under the microphone row: `HW 1 · <name>` with Voicemeeter, else the name.
#[must_use]
pub fn mic_caption(name: &str, voicemeeter: bool) -> String {
    if voicemeeter {
        format!("HW 1 \u{b7} {name}")
    } else {
        name.to_string()
    }
}

/// Whether any strip peaks above [`SIGNAL_PEAK`] on either side.
#[must_use]
pub fn any_signal(strips: &[[f32; 2]]) -> bool {
    strips.iter().any(|[l, r]| l.max(*r) > SIGNAL_PEAK)
}

/// The pill route: muted wins, then sound (media playing or a strip with signal), else idle.
#[must_use]
pub fn pill_state(muted: bool, playing: bool, signal: bool) -> PillState {
    if muted {
        PillState::Muted
    } else if playing || signal {
        PillState::Live
    } else {
        PillState::Idle
    }
}

/// The pill: the default endpoint's name, its volume in percent and the route state. Sound
/// is media `playing` or a strip holding signal in the mixer's debounced flags.
#[must_use]
pub fn pill_facts(audio: &AudioFacts, playing: bool) -> PillFacts {
    let signal = audio
        .vm
        .as_ref()
        .is_some_and(|vm| vm.signal.iter().any(|s| *s));
    let output = audio
        .default_id
        .as_ref()
        .and_then(|id| audio.endpoints.iter().find(|e| &e.id == id))
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let muted = matches!(audio.attached, Some((_, true)));
    PillFacts {
        output,
        value: audio.attached.map(|(volume, _)| percent(volume)),
        state: pill_state(muted, playing, signal),
    }
}

/// A 0..1 volume as a whole percent, clamped to 0..=100.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(v: f32) -> u8 {
    // The value is clamped to 0..=100 first, so the cast cannot truncate or lose a sign.
    (v * 100.0).round().clamp(0.0, 100.0) as u8
}

/// A readable media source name from an app user model id: the part after `!`, without a
/// path or an `.exe`, and its last dotted segment (`SpotifyAB.SpotifyMusic_x!Spotify` and
/// `Spotify.exe` both read `Spotify`).
#[must_use]
pub fn source_label(source_id: &str) -> String {
    let app = source_id.rsplit('!').next().unwrap_or_default();
    let file = app.rsplit(['\\', '/']).next().unwrap_or_default();
    let stem = match file.len().checked_sub(4) {
        Some(cut) if file.is_char_boundary(cut) && file[cut..].eq_ignore_ascii_case(".exe") => {
            &file[..cut]
        }
        _ => file,
    };
    stem.rsplit('.').next().unwrap_or_default().to_string()
}

/// A playing timeline moved forward to `now`, so a rebuilt page shows the current elapsed
/// time; a paused one is returned unchanged.
#[must_use]
pub fn rebase(timeline: SeekTimeline, now: SystemTime) -> SeekTimeline {
    if !timeline.playing {
        return timeline;
    }
    let elapsed = now
        .duration_since(timeline.stamp)
        .map_or(0.0, |gap| gap.as_secs_f64());
    let end = if timeline.duration_s.is_finite() && timeline.duration_s > 0.0 {
        timeline.duration_s
    } else {
        f64::INFINITY
    };
    SeekTimeline {
        position_s: (timeline.position_s + elapsed).min(end),
        stamp: now,
        ..timeline
    }
}

/// A playing timeline moved forward to the last whole second of the track before `now` (or to
/// its end), with the stamp moved by the same amount. It describes the same playback, so the
/// painter's extrapolation does not jump, but it changes only once a second: page facts built
/// from it compare equal between second boundaries, and the elapsed text it feeds is whole
/// seconds anyway. A paused timeline, or one that has not reached its next second, is returned
/// unchanged.
#[must_use]
pub fn rebase_to_second(timeline: SeekTimeline, now: SystemTime) -> SeekTimeline {
    if !timeline.playing {
        return timeline;
    }
    let exact = rebase(timeline, now).position_s;
    let at_end = timeline.duration_s.is_finite() && exact >= timeline.duration_s;
    let target = if at_end { exact } else { exact.floor() };
    let shift = target - timeline.position_s;
    if shift <= 0.0 {
        return timeline;
    }
    SeekTimeline {
        position_s: target,
        stamp: timeline.stamp + std::time::Duration::from_secs_f64(shift),
        ..timeline
    }
}

#[cfg(test)]
mod tests;
