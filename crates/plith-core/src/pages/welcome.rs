//! The welcome view: the island's opening page, the Karot run.
//!
//! Positions come from `welcome()` in `docs/design/r7/karot.html` (620 x 232 DIP). Text rects
//! follow the mock's baselines: the top of a rect is the baseline minus the font size.

use super::tabbar::tabbar;
use super::{PAGE_W, PageBuild};
use crate::draw::DisplayList;
use crate::lang::Lang;
use crate::plays::Play;
use crate::settings::{HeadSlot, RunSpan, TempUnit};
use crate::shapes;
use crate::weather::{Day, Forecast};

mod axis;
mod hatch;
mod head;
mod run;

use axis::{Axis, Weather};

/// The run band that answers the pointer: top and bottom y.
const BAND_TOP: f64 = 126.0;
const BAND_BOTTOM: f64 = 172.0;

#[cfg(test)]
mod tests;

/// What the sound head slot shows.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundHead {
    /// The output name, for example `G733`.
    pub output: String,
    /// The kind of device, for example `KULAKLIK`.
    pub kind: String,
    /// The volume percentage, when known.
    pub value: Option<u8>,
    /// Whether the output is muted.
    pub muted: bool,
    /// Whether sound flows to the output now.
    pub flowing: bool,
}

/// Everything the welcome view needs.
#[derive(Debug, Clone, PartialEq)]
pub struct WelcomeFacts {
    /// Today and the minute of the day.
    pub now: (Day, u32),
    /// The short date, for example `SAL 17 KAS`.
    pub date: String,
    /// Today's plays; the last one is the one playing now when `playing`.
    pub plays: Vec<Play>,
    /// Whether a track is playing.
    pub playing: bool,
    /// The forecast, when there is one.
    pub forecast: Option<Forecast>,
    /// Whether weather is on.
    pub weather_on: bool,
    /// How much of the day the run shows.
    pub span: RunSpan,
    /// The temperature unit.
    pub unit: TempUnit,
    /// What fills the top right.
    pub head: HeadSlot,
    /// The default output, for the sound head.
    pub sound: Option<SoundHead>,
    /// Elapsed and total seconds of the playing track.
    pub elapsed: Option<(u32, u32)>,
    /// The interface language.
    pub lang: Lang,
}

/// The welcome page. `probe` is the hour cell under the pointer, from `probe_hour`.
#[must_use]
pub fn welcome_page(f: &WelcomeFacts, probe: Option<i32>) -> PageBuild {
    let weather = Weather::of(f);
    let hide_label = head::slot(f, weather.as_ref()) == HeadSlot::Playing;
    let (mut nodes, targets) = tabbar(None, "");
    nodes.extend(head::head(f, weather.as_ref(), probe));
    nodes.extend(run::run(f, weather.as_ref(), probe, hide_label));
    PageBuild {
        list: DisplayList {
            nodes,
            size: (PAGE_W, shapes::WELCOME.height),
        },
        targets,
        shape: shapes::WELCOME,
    }
}

/// The hour cell under the pointer when it is inside the run band, else `None`. The hour counts
/// from today's midnight and may be below 0 or past 23 when the run crosses midnight.
#[must_use]
pub fn probe_hour(f: &WelcomeFacts, x: f64, y: f64) -> Option<i32> {
    if !(BAND_TOP..=BAND_BOTTOM).contains(&y) || !(axis::X0..axis::X1).contains(&x) {
        return None;
    }
    Some(axis::floor_i32(Axis::new(f).minute_at(x) / 60.0))
}
