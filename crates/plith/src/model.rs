//! Service readings to core facts: field copies only. Every decision (names, the pill, the
//! microphone bus, the media source) is a `plith_core` function; the edition counts come from
//! `VmKind`.

use crate::services::{Facts, Readings};
use plith_audio::endpoints::EndpointState;
use plith_audio::microphone::MicSnapshot;
use plith_audio::voicemeeter::{ChannelSnapshot, VmSnapshot};
use plith_core::draw::{ArtId, ArtPixels, SeekTimeline};
use plith_core::pages::audio::{
    AudioFacts, ChannelFacts, EndpointFacts, MicFacts, VmFacts, device_kind,
};
use plith_core::pages::derive;
use plith_core::pages::media::MediaFacts;
use plith_core::pages::system::{self, SystemFacts};
use plith_media::MediaSnapshot;
use plith_system::DeviceLevel;
use std::sync::Arc;

/// One strip or bus.
fn channel(c: &ChannelSnapshot) -> ChannelFacts {
    ChannelFacts {
        label: c.label.clone(),
        gain_db: c.gain_db,
        muted: c.muted,
        routes: c.routes.clone(),
    }
}

/// The mixer as audio facts.
#[must_use]
pub fn vm_facts(vm: &VmSnapshot) -> VmFacts {
    VmFacts {
        strips: vm.strips.iter().map(channel).collect(),
        buses: vm.buses.iter().map(channel).collect(),
        hardware_strips: vm.kind.hardware_strips(),
        hardware_buses: vm.kind.hardware_buses(),
        signal: vm.signal.clone(),
    }
}

/// The audio page facts from the mixer, the endpoints and the microphone.
#[must_use]
pub fn audio_facts(
    vm: Option<&VmSnapshot>,
    endpoints: &EndpointState,
    mic: Option<&MicSnapshot>,
) -> AudioFacts {
    AudioFacts {
        vm: vm.map(vm_facts),
        endpoints: endpoints
            .active
            .iter()
            .map(|e| EndpointFacts {
                id: e.id.clone(),
                name: derive::short_name(&e.device_name, &e.friendly_name),
                kind: device_kind(e.form_factor, &[&e.device_name, &e.friendly_name]),
            })
            .collect(),
        default_id: endpoints.default_id.clone(),
        attached: endpoints.attached.as_ref().map(|a| (a.volume, a.muted)),
        mic: mic.map(|m| MicFacts {
            name: m.name.clone(),
            muted: m.muted,
        }),
    }
}

/// The media page facts; `output` is the route text after the arrow. The timeline is kept as
/// the session reported it: the painter extrapolates the seek and the elapsed time from its
/// stamp, so the facts (and the page) stay equal for the whole track.
#[must_use]
pub fn media_facts(media: &MediaSnapshot, output: Option<String>) -> MediaFacts {
    if !media.has_session {
        return MediaFacts {
            output,
            ..MediaFacts::default()
        };
    }
    let timeline = media.timeline.map(|t| SeekTimeline {
        position_s: t.position.as_secs_f64(),
        duration_s: t.duration.as_secs_f64(),
        stamp: t.stamp,
        playing: media.playing,
    });
    MediaFacts {
        has_session: true,
        source_name: derive::source_label(&media.source_id),
        title: media.title.clone(),
        artist: media.artist.clone(),
        art: media.artwork.as_ref().map(|a| ArtId(a.id)),
        playing: media.playing,
        can_seek: media.can_seek,
        can_next: media.can_next,
        can_prev: media.can_prev,
        timeline,
        output,
    }
}

/// The decoded cover art as page artwork.
#[must_use]
pub fn art_pixels(media: &MediaSnapshot) -> Option<Arc<ArtPixels>> {
    let art = media.artwork.as_ref()?;
    Some(Arc::new(ArtPixels {
        id: ArtId(art.id),
        width: art.width,
        height: art.height,
        bgra: Arc::clone(&art.bgra),
    }))
}

/// The system page facts: the first display's brightness, the microphone and the bus it feeds.
#[must_use]
pub fn system_facts(
    brightness: &[DeviceLevel],
    mic: Option<&MicSnapshot>,
    vm: Option<&VmFacts>,
) -> SystemFacts {
    SystemFacts {
        brightness: brightness.first().map(|d| (d.name.clone(), d.percent)),
        mic: mic.map(|m| system::MicFacts {
            name: derive::mic_caption(&m.name, vm.is_some()),
            muted: m.muted,
        }),
        mic_bus: vm.and_then(derive::mic_bus),
    }
}

/// Every page's facts from the readings; the pill reads the mixer's debounced signal.
#[must_use]
pub fn facts(readings: &Readings) -> Facts {
    let audio = audio_facts(
        readings.vm.as_ref(),
        &readings.endpoints,
        readings.mic.as_ref(),
    );
    let pill = derive::pill_facts(&audio, readings.media.playing);
    let output = (!pill.output.is_empty()).then(|| pill.output.clone());
    let system = system_facts(
        &readings.brightness,
        readings.mic.as_ref(),
        audio.vm.as_ref(),
    );
    Facts {
        media: media_facts(&readings.media, output),
        art: art_pixels(&readings.media),
        audio,
        system,
        pill,
    }
}

#[cfg(test)]
mod tests;
