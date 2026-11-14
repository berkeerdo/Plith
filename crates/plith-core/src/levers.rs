//! Load levers: opt-in launch settings that trade our own frame pacing against load on the
//! machine. The defaults are the measured R1b picks and never raise a priority.

/// The GPU scheduling class of this process (`D3DKMTSetProcessSchedulingPriorityClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchedClass {
    /// Leave the class alone.
    #[default]
    Normal,
    /// Above normal.
    AboveNormal,
    /// High.
    High,
}

impl SchedClass {
    /// The flag spelling of the class.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::AboveNormal => "above",
            Self::High => "high",
        }
    }
}

/// The MMCSS priority of the render thread inside its task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MmcssPriority {
    /// Leave the priority alone.
    #[default]
    Normal,
    /// High.
    High,
    /// Critical.
    Critical,
}

impl MmcssPriority {
    /// The flag spelling of the priority.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// The five load levers; `Default` is the R1b pick (latency 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levers {
    /// DXGI GPU thread priority, -7 to 7.
    pub gpu_priority: i8,
    /// GPU scheduling class of the process.
    pub sched_class: SchedClass,
    /// Maximum frame latency of the DXGI device, 1 to 3.
    pub latency: u8,
    /// Present sync interval, 0 or 1.
    pub sync: u8,
    /// MMCSS thread priority.
    pub mmcss: MmcssPriority,
}

impl Default for Levers {
    fn default() -> Self {
        Self {
            gpu_priority: 0,
            sched_class: SchedClass::Normal,
            latency: 2,
            sync: 1,
            mmcss: MmcssPriority::Normal,
        }
    }
}

impl Levers {
    /// All five settings for the start line.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "gpu-priority {}, sched-class {}, latency {}, sync {}, mmcss {}",
            self.gpu_priority,
            self.sched_class.as_str(),
            self.latency,
            self.sync,
            self.mmcss.as_str()
        )
    }
}

/// What the OS said to one lever.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeverOutcome {
    /// The lever's name, for example `gpu-priority`.
    pub name: &'static str,
    /// The requested setting, for example `7`.
    pub setting: String,
    /// `Ok` when applied; the error code as hex text when refused.
    pub result: Result<(), String>,
}

/// The log line of an outcome: `lever gpu-priority 7: ok` or `... : refused 0x80070005`.
#[must_use]
pub fn outcome_line(outcome: &LeverOutcome) -> String {
    let LeverOutcome {
        name,
        setting,
        result,
    } = outcome;
    match result {
        Ok(()) => format!("lever {name} {setting}: ok"),
        Err(code) => format!("lever {name} {setting}: refused {code}"),
    }
}

#[cfg(test)]
mod tests;
