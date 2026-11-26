//! The manifest reader: the line rules of `settings::parse`, an ordered key list with duplicate
//! detection, the table of known keys and typed accessors that name the key in every error.

use super::PackError;
use crate::theme::colour::Rgb;

/// The languages a blurb may be written in.
pub(super) const LANGS: [&str; 8] = ["tr", "en", "de", "es", "fr", "it", "pl", "pt"];

/// The ink roles of a face, as key suffixes after `ink.`.
const INK_KEYS: [&str; 6] = [
    "primary",
    "muted",
    "faint",
    "track",
    "faint_track",
    "border",
];

/// The floor keys of a face, as key suffixes after `floor.`.
pub(super) const FLOOR_KEYS: [&str; 5] = ["kind", "color", "color_high", "lum_min", "lum_max"];

const SINGLE_KEYS: [&str; 13] = [
    "format",
    "id",
    "name",
    "floor.adjustable",
    "material.tintable",
    "text_zone",
    "flow.color",
    "flow.style",
    "flow.adjustable",
    "flow.swatches",
    "motion.open",
    "motion.overlays",
    "dark",
];

fn is_known(key: &str) -> bool {
    if SINGLE_KEYS.contains(&key) {
        return true;
    }
    if let Some(lang) = key.strip_prefix("blurb.") {
        return LANGS.contains(&lang);
    }
    let rest = key.strip_prefix("collapsed.").unwrap_or(key);
    if let Some(floor) = rest.strip_prefix("floor.") {
        return FLOOR_KEYS.contains(&floor);
    }
    rest.strip_prefix("ink.")
        .is_some_and(|ink| INK_KEYS.contains(&ink))
}

/// The key/value pairs of a manifest, in file order.
pub(super) struct Fields {
    entries: Vec<(String, String)>,
}

/// Reads a manifest into fields. Blank lines and `#` comments are skipped, a leading BOM and
/// CRLF are tolerated, and unknown keys, duplicates and lines without `=` refuse the pack.
pub(super) fn read(text: &str) -> Result<Fields, PackError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries: Vec<(String, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(PackError::new(format!("line {}", index + 1), "bad line"));
        };
        let (key, value) = (key.trim(), value.trim());
        if !is_known(key) {
            return Err(PackError::new(key, "unknown key"));
        }
        if entries.iter().any(|(k, _)| k == key) {
            return Err(PackError::new(key, "duplicate key"));
        }
        entries.push((key.to_owned(), value.to_owned()));
    }
    Ok(Fields { entries })
}

impl Fields {
    /// The raw value of a key, when present.
    pub(super) fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Whether any key starts with the prefix.
    pub(super) fn has_prefix(&self, prefix: &str) -> bool {
        self.entries.iter().any(|(k, _)| k.starts_with(prefix))
    }

    /// The keys that start with a prefix (prefix removed) with their values, in file order.
    pub(super) fn with_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.entries
            .iter()
            .filter_map(move |(k, v)| Some((k.strip_prefix(prefix)?, v.as_str())))
    }

    /// A required key.
    pub(super) fn required(&self, key: &str) -> Result<&str, PackError> {
        self.get(key)
            .ok_or_else(|| PackError::new(key, "missing required key"))
    }

    /// An optional colour.
    pub(super) fn colour(&self, key: &str) -> Result<Option<Rgb>, PackError> {
        self.get(key)
            .map(|v| Rgb::parse(v).ok_or_else(|| PackError::new(key, "bad colour")))
            .transpose()
    }

    /// A required colour.
    pub(super) fn colour_required(&self, key: &str) -> Result<Rgb, PackError> {
        self.colour(key)?
            .ok_or_else(|| PackError::new(key, "missing required key"))
    }

    /// An optional finite number.
    pub(super) fn number(&self, key: &str) -> Result<Option<f64>, PackError> {
        self.get(key)
            .map(|v| number(v).ok_or_else(|| PackError::new(key, "bad number")))
            .transpose()
    }

    /// An optional `true` or `false`.
    pub(super) fn boolean(&self, key: &str) -> Result<Option<bool>, PackError> {
        match self.get(key) {
            None => Ok(None),
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            Some(_) => Err(PackError::new(key, "bad value")),
        }
    }

    /// An optional comma separated list; every item is non-empty.
    pub(super) fn list(&self, key: &str) -> Result<Option<Vec<&str>>, PackError> {
        let Some(value) = self.get(key) else {
            return Ok(None);
        };
        let items: Vec<&str> = value.split(',').map(str::trim).collect();
        if items.iter().any(|item| item.is_empty()) {
            return Err(PackError::new(key, "bad list"));
        }
        Ok(Some(items))
    }
}

fn number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The four numbers of a `text_zone` list, or `None` when the list is not four numbers.
pub(super) fn rect(items: &[&str]) -> Option<[f32; 4]> {
    if items.len() != 4 {
        return None;
    }
    let mut out = [0.0_f32; 4];
    for (slot, item) in out.iter_mut().zip(items) {
        // DIP values are small; narrowing to f32 loses nothing that matters.
        #[allow(clippy::cast_possible_truncation)]
        let n = number(item)? as f32;
        *slot = n;
    }
    Some(out)
}
