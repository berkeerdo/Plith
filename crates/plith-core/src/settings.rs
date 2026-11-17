//! The idle look settings and their text file format: one `key=value` per line, `#` comments,
//! blank lines skipped. Parsing never fails; a bad line is reported and the default stays.

/// What the collapsed island shows while nothing happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleLook {
    /// The Karot look.
    #[default]
    Karot,
    /// A plain clock.
    Clock,
    /// The playing track.
    Playing,
    /// The seal look.
    Seal,
    /// A plain pill.
    Plain,
    /// No pill at rest.
    Hidden,
}

impl IdleLook {
    /// The key shared by the settings file and `--idle-look`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Karot => "karot",
            Self::Clock => "clock",
            Self::Playing => "playing",
            Self::Seal => "seal",
            Self::Plain => "plain",
            Self::Hidden => "hidden",
        }
    }

    /// The look for a key, or `None` when the key is unknown.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "karot" => Self::Karot,
            "clock" => Self::Clock,
            "playing" => Self::Playing,
            "seal" => Self::Seal,
            "plain" => Self::Plain,
            "hidden" => Self::Hidden,
            _ => return None,
        })
    }
}

/// The span the run chart covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunSpan {
    /// Three hours.
    Three,
    /// Six hours.
    #[default]
    Six,
    /// A whole day.
    Day,
}

/// Where the weather location comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationMode {
    /// The Windows location service.
    #[default]
    Windows,
    /// A lookup by IP address.
    Ip,
    /// The city typed in the settings.
    City,
}

/// The temperature unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TempUnit {
    /// Celsius.
    #[default]
    C,
    /// Fahrenheit.
    F,
}

/// The page the island opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpenOn {
    /// The welcome view.
    #[default]
    Welcome,
    /// The page that was open last.
    LastPage,
}

/// What fills the top right of the welcome view. `Auto` is Weather while weather is on and
/// Sound otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadSlot {
    /// Weather while weather is on, Sound otherwise.
    #[default]
    Auto,
    /// The weather station.
    Weather,
    /// The sound station.
    Sound,
    /// The playing station.
    Playing,
    /// Nothing.
    Empty,
}

/// The interface language: follow the system, or a lower-case primary language code. An unknown
/// code is kept as written and resolves to English.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LangChoice {
    /// Follow the system language.
    #[default]
    Auto,
    /// A language code such as `tr` or `en`.
    Code(String),
}

/// Every idle look and welcome view setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The look of the collapsed island.
    pub look: IdleLook,
    /// The span of the run chart.
    pub span: RunSpan,
    /// Whether the plays chart is shown.
    pub plays: bool,
    /// Whether weather is on.
    pub weather: bool,
    /// Where the weather location comes from.
    pub location: LocationMode,
    /// The city for `LocationMode::City`, free text.
    pub city: String,
    /// The temperature unit.
    pub unit: TempUnit,
    /// Whether the clock is 24 hour.
    pub clock24: bool,
    /// The page the island opens on.
    pub open_on: OpenOn,
    /// The interface language.
    pub lang: LangChoice,
    /// What fills the top right of the welcome view.
    pub head: HeadSlot,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            look: IdleLook::default(),
            span: RunSpan::default(),
            plays: true,
            weather: false,
            location: LocationMode::default(),
            city: String::new(),
            unit: TempUnit::default(),
            clock24: true,
            open_on: OpenOn::default(),
            lang: LangChoice::default(),
            head: HeadSlot::default(),
        }
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

fn parse_span(value: &str) -> Option<RunSpan> {
    match value {
        "3h" => Some(RunSpan::Three),
        "6h" => Some(RunSpan::Six),
        "day" => Some(RunSpan::Day),
        _ => None,
    }
}

fn parse_location(value: &str) -> Option<LocationMode> {
    match value {
        "windows" => Some(LocationMode::Windows),
        "ip" => Some(LocationMode::Ip),
        "city" => Some(LocationMode::City),
        _ => None,
    }
}

fn parse_unit(value: &str) -> Option<TempUnit> {
    match value {
        "c" => Some(TempUnit::C),
        "f" => Some(TempUnit::F),
        _ => None,
    }
}

fn parse_open_on(value: &str) -> Option<OpenOn> {
    match value {
        "welcome" => Some(OpenOn::Welcome),
        "last" => Some(OpenOn::LastPage),
        _ => None,
    }
}

fn parse_head(value: &str) -> Option<HeadSlot> {
    match value {
        "auto" => Some(HeadSlot::Auto),
        "weather" => Some(HeadSlot::Weather),
        "sound" => Some(HeadSlot::Sound),
        "playing" => Some(HeadSlot::Playing),
        "empty" => Some(HeadSlot::Empty),
        _ => None,
    }
}

fn parse_lang(value: &str) -> Option<LangChoice> {
    match value {
        "" => None,
        "auto" => Some(LangChoice::Auto),
        code => Some(LangChoice::Code(code.to_string())),
    }
}

/// Applies one value to the field named by `key`. Returns `false` when the value is bad.
fn apply(s: &mut Settings, key: &str, value: &str) -> bool {
    match key {
        "look" => IdleLook::from_key(value).map(|v| s.look = v).is_some(),
        "span" => parse_span(value).map(|v| s.span = v).is_some(),
        "plays" => parse_bool(value).map(|v| s.plays = v).is_some(),
        "weather" => parse_bool(value).map(|v| s.weather = v).is_some(),
        "location" => parse_location(value).map(|v| s.location = v).is_some(),
        "city" => {
            s.city = value.to_string();
            true
        }
        "unit" => parse_unit(value).map(|v| s.unit = v).is_some(),
        "clock24" => parse_bool(value).map(|v| s.clock24 = v).is_some(),
        "open_on" => parse_open_on(value).map(|v| s.open_on = v).is_some(),
        "lang" => parse_lang(value).map(|v| s.lang = v).is_some(),
        "head" => parse_head(value).map(|v| s.head = v).is_some(),
        _ => false,
    }
}

const KEYS: [&str; 11] = [
    "look", "span", "plays", "weather", "location", "city", "unit", "clock24", "open_on", "lang",
    "head",
];

/// Parses the settings text. Bad lines keep the default and add one entry to the problem list.
#[must_use]
pub fn parse(text: &str) -> (Settings, Vec<String>) {
    let mut settings = Settings::default();
    let mut problems = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!("line {number}: expected key=value"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if !KEYS.contains(&key) {
            problems.push(format!("line {number}: unknown key `{key}`"));
        } else if !apply(&mut settings, key, value) {
            problems.push(format!("line {number}: bad value `{value}` for `{key}`"));
        }
    }
    (settings, problems)
}

/// Writes every key in a fixed order, one `key=value` per line, ending with a newline.
#[must_use]
pub fn serialize(s: &Settings) -> String {
    let span = match s.span {
        RunSpan::Three => "3h",
        RunSpan::Six => "6h",
        RunSpan::Day => "day",
    };
    let location = match s.location {
        LocationMode::Windows => "windows",
        LocationMode::Ip => "ip",
        LocationMode::City => "city",
    };
    let unit = match s.unit {
        TempUnit::C => "c",
        TempUnit::F => "f",
    };
    let open_on = match s.open_on {
        OpenOn::Welcome => "welcome",
        OpenOn::LastPage => "last",
    };
    let lang = match &s.lang {
        LangChoice::Auto => "auto",
        LangChoice::Code(code) => code.as_str(),
    };
    let head = match s.head {
        HeadSlot::Auto => "auto",
        HeadSlot::Weather => "weather",
        HeadSlot::Sound => "sound",
        HeadSlot::Playing => "playing",
        HeadSlot::Empty => "empty",
    };
    format!(
        "look={}\nspan={span}\nplays={}\nweather={}\nlocation={location}\ncity={}\nunit={unit}\n\
         clock24={}\nopen_on={open_on}\nlang={lang}\nhead={head}\n",
        s.look.key(),
        on_off(s.plays),
        on_off(s.weather),
        s.city,
        on_off(s.clock24),
    )
}

#[cfg(test)]
mod tests;
