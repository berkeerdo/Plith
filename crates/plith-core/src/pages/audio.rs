//! The audio view model: plain audio facts in, source rows, output rows and routes out.
//!
//! Facts are copied from the service crates by the app; this module knows nothing about them.
//! Layout (positions, nodes) is a separate step that reads this view.

mod model;
#[cfg(test)]
mod tests;

pub use model::Activity;

/// Row pitch in DIP for up to five source rows.
pub const PITCH_NORMAL: f64 = 57.0;
/// Row pitch in DIP above five source rows.
pub const PITCH_DENSE: f64 = 40.0;
/// Source row count above which the pitch shrinks.
pub const DENSE_ABOVE: usize = 5;

/// One strip or bus as the mixer reports it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChannelFacts {
    /// The user label (may be empty).
    pub label: String,
    /// Gain in dB.
    pub gain_db: f32,
    /// Whether the channel is muted.
    pub muted: bool,
    /// For a strip: which buses it routes to, indexed by bus. Unused for buses.
    pub routes: Vec<bool>,
}

/// The mixer state for one Voicemeeter edition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VmFacts {
    /// Strips, hardware first, then virtual.
    pub strips: Vec<ChannelFacts>,
    /// Buses, hardware (A) first, then virtual (B).
    pub buses: Vec<ChannelFacts>,
    /// How many strips are hardware inputs.
    pub hardware_strips: usize,
    /// How many buses are hardware outputs (A buses).
    pub hardware_buses: usize,
}

/// One active Windows output endpoint.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EndpointFacts {
    /// The endpoint id.
    pub id: String,
    /// The display name.
    pub name: String,
}

/// The default microphone.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MicFacts {
    /// The display name.
    pub name: String,
    /// Whether it is muted.
    pub muted: bool,
}

/// Everything the audio page needs to know.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioFacts {
    /// The mixer, or `None` when Voicemeeter is not running.
    pub vm: Option<VmFacts>,
    /// Active output endpoints.
    pub endpoints: Vec<EndpointFacts>,
    /// The default endpoint id.
    pub default_id: Option<String>,
    /// Volume (0..1) and mute of the attached endpoint.
    pub attached: Option<(f32, bool)>,
    /// The default microphone.
    pub mic: Option<MicFacts>,
}

/// A source row.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRow {
    /// The main name.
    pub name: String,
    /// The small caption under it.
    pub sub: String,
    /// Whether the source is muted.
    pub muted: bool,
    /// Whether it is the picked source.
    pub picked: bool,
}

/// What an output row shows as its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputValue {
    /// A gain in dB.
    Db(f32),
    /// A volume percent.
    Percent(u8),
    /// No value.
    None,
}

/// An output row.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputRow {
    /// Bus tag such as `A1`; `None` for Windows endpoints.
    pub tag: Option<String>,
    /// The main name.
    pub name: String,
    /// The value shown.
    pub value: OutputValue,
    /// Whether any live route ends here.
    pub live: bool,
    /// Whether the bus or endpoint is muted.
    pub muted: bool,
}

/// How a route is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteState {
    /// Routed and carrying signal.
    Live,
    /// Routed, no signal.
    Idle,
    /// Routed but the source is muted.
    Cut,
    /// Not routed; offered while a source is picked.
    Candidate,
}

/// A line from a source row to an output row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// Source row index.
    pub source: usize,
    /// Output row index.
    pub output: usize,
    /// How it is drawn.
    pub state: RouteState,
}

/// Whether a source is picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickMode {
    /// Nothing picked.
    None,
    /// The source at this row index is picked.
    Picked(usize),
}

/// Which footer line shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Footer {
    /// Idle hint.
    Normal,
    /// A source is picked.
    Picking,
    /// Voicemeeter is not running.
    NoVoicemeeter,
}

impl Footer {
    /// The footer copy.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Footer::Normal => "KAYNAĞA, SONRA ÇIKIŞA TIKLA",
            Footer::Picking => "ÇIKIŞA TIKLA: YOL KURULUR \u{B7} KAYNAĞA TEKRAR TIKLA: VAZGEÇ",
            Footer::NoVoicemeeter => "VOICEMEETER ÇALIŞMIYOR \u{B7} ÇIKIŞA TIKLA: VARSAYILAN OLUR",
        }
    }
}

/// The audio page as data.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioView {
    /// Source rows, top to bottom.
    pub sources: Vec<SourceRow>,
    /// Output rows, top to bottom.
    pub outputs: Vec<OutputRow>,
    /// Routes to draw.
    pub routes: Vec<Route>,
    /// The pick mode the view was built for.
    pub mode: PickMode,
    /// The footer line.
    pub footer: Footer,
    /// Row pitch in DIP.
    pub row_pitch: f64,
}

/// Maps facts, signal activity and the pick mode to the view.
#[must_use]
pub fn audio_view(
    facts: &AudioFacts,
    activity: &Activity,
    mode: PickMode,
    now_s: f64,
) -> AudioView {
    model::build(facts, activity, mode, now_s)
}
