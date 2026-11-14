//! WQL text for the built-in panel, and the ids derived from instance names. Pure.

/// The WMI namespace that holds the brightness classes.
pub const NAMESPACE: &str = r"root\wmi";

/// Escapes a value for a single-quoted WQL string literal.
///
/// Instance names are full of backslashes (`DISPLAY\AUS27FD\5&efad&0&UID4353_0`) and a
/// backslash escapes the next character in WQL, so an unescaped name silently matches nothing
/// and the panel looks absent. Backslashes are doubled first so the escape of a quote survives.
#[must_use]
pub fn escape_wql(value: &str) -> String {
    value.replace('\\', r"\\").replace('\'', r"\'")
}

/// The device id of a built-in panel with this WMI instance name.
#[must_use]
pub fn panel_id(instance: &str) -> String {
    format!("Internal panel {instance}")
}

/// The display name of a built-in panel: "Built-in display", plus the short model id when the
/// instance name has one (`DISPLAY\AUS27FD\5&efad&0&UID4353_0` gives `AUS27FD`).
#[must_use]
pub fn panel_name(instance: &str) -> String {
    let model = instance
        .split('\\')
        .nth(1)
        .map(str::trim)
        .filter(|segment| !segment.is_empty());
    match model {
        Some(model) => format!("Built-in display ({model})"),
        None => "Built-in display".to_owned(),
    }
}

/// Query for the current level of one panel.
#[must_use]
pub fn read_query(instance: &str) -> String {
    format!(
        "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE InstanceName = '{}'",
        escape_wql(instance)
    )
}

/// Query for the method object (and its path) of one panel.
#[must_use]
pub fn methods_query(instance: &str) -> String {
    format!(
        "SELECT __PATH FROM WmiMonitorBrightnessMethods WHERE InstanceName = '{}'",
        escape_wql(instance)
    )
}
