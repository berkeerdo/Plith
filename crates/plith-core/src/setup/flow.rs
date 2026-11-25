//! The installer's screen state and what it asks the app to do.
//!
//! The flow is pure: the app feeds it clicks, keys and worker reports, draws [`Flow::screen`]
//! and carries out the returned [`Effect`]s. Nothing here touches Windows.

use super::{Job, Mode, Report, Screen, SetupAction, Situation, Version};

#[cfg(test)]
mod tests;

/// Something the app must do for the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Start the elevated worker for this job.
    Elevate(Job),
    /// Start the installed `plith.exe` from the medium-integrity setup process.
    StartPlith,
    /// Turn the start with Windows value on or off.
    SetAutostart(bool),
    /// Delete the user's data folder.
    DeleteUserData,
    /// Write a line to `setup.log`.
    Log(String),
    /// Close the window and end the process.
    Quit,
}

/// The screens and their transitions.
#[derive(Debug)]
pub struct Flow {
    own: Version,
    screen: Screen,
    job: Job,
    delete_data: bool,
    /// The start with Windows tick the done screen starts with.
    autostart: bool,
    /// Started by the update check: a finished install starts the new Plith by itself.
    update: bool,
}

/// The step an error is attributed to when the worker said nothing: the first of an install,
/// the only one of an uninstall.
fn silent_step(job: Job) -> u8 {
    match job {
        Job::Install => 1,
        Job::Uninstall => 3,
    }
}

/// An error code from a worker reason: its decimal digits, else 0 (none).
fn code_of(reason: &str) -> u32 {
    reason.parse().unwrap_or(0)
}

impl Flow {
    /// The first screen for how this setup was started and what is on the machine. `own` is the
    /// version this setup carries. `run_value_ours` is the Run value `Plith` found at start:
    /// `None` no value, `Some(true)` a Plith one, `Some(false)` another program's. A fresh or
    /// WPF install ticks start with Windows; an update or repair keeps the user's choice, so it
    /// is ticked only when a Plith value is there.
    #[must_use]
    pub fn new(
        mode: &Mode,
        situation: Situation,
        own: Version,
        run_value_ours: Option<bool>,
    ) -> Self {
        let autostart = match (mode, situation) {
            (Mode::Install, Situation::Fresh | Situation::OverWpf { .. }) => true,
            _ => run_value_ours == Some(true),
        };
        let update = |from| Screen::Update { from, to: own };
        let (screen, job) = match (mode, situation) {
            (_, Situation::NewerInstalled { installed }) => {
                (Screen::Newer { installed }, Job::Install)
            }
            (Mode::Uninstall, _) => (
                Screen::UninstallConfirm { delete_data: false },
                Job::Uninstall,
            ),
            (Mode::Update { from }, _) => (update(*from), Job::Install),
            (_, Situation::Upgrade { from }) => (update(from), Job::Install),
            _ => (Screen::Start, Job::Install),
        };
        Self {
            own,
            screen,
            job,
            delete_data: false,
            autostart,
            update: matches!(mode, Mode::Update { .. }),
        }
    }

    /// The screen to draw.
    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen.clone()
    }

    /// The version this setup carries.
    #[must_use]
    pub fn own(&self) -> Version {
        self.own
    }

    fn running(&self) -> bool {
        matches!(
            self.screen,
            Screen::Running { .. } | Screen::Uninstalling { .. }
        )
    }

    /// What Enter does: the action of the screen's pill; `None` while work is going on.
    #[must_use]
    pub fn primary_action(&self) -> Option<SetupAction> {
        match self.screen {
            Screen::Start | Screen::UacCancelled | Screen::Update { .. } => {
                Some(SetupAction::Install)
            }
            Screen::Done { .. } => Some(SetupAction::Open),
            Screen::Error { .. } => Some(SetupAction::Retry),
            Screen::UninstallConfirm { .. } => Some(SetupAction::Uninstall),
            Screen::Newer { .. } | Screen::Uninstalled => Some(SetupAction::Close),
            Screen::Running { .. } | Screen::Uninstalling { .. } => None,
        }
    }

    /// Starts the worker for `job` and shows its first running screen.
    fn begin(&mut self, job: Job) -> Vec<Effect> {
        self.job = job;
        self.screen = match job {
            Job::Install => Screen::Running { done: 0 },
            Job::Uninstall => Screen::Uninstalling { done: 0 },
        };
        vec![Effect::Elevate(job)]
    }

    /// A click or key on an action of the current screen; an action the screen does not offer
    /// does nothing.
    pub fn on_action(&mut self, action: SetupAction) -> Vec<Effect> {
        match (action, &self.screen) {
            (
                SetupAction::Install,
                Screen::Start | Screen::UacCancelled | Screen::Update { .. },
            ) => self.begin(Job::Install),
            (SetupAction::Uninstall, Screen::UninstallConfirm { .. }) => self.begin(Job::Uninstall),
            (SetupAction::Retry, Screen::Error { .. }) => self.begin(self.job),
            (SetupAction::Open, Screen::Done { .. }) => vec![Effect::StartPlith, Effect::Quit],
            (SetupAction::Close, _) if !self.running() => vec![Effect::Quit],
            (SetupAction::ToggleAutostart, Screen::Done { autostart }) => {
                let on = !autostart;
                self.screen = Screen::Done { autostart: on };
                vec![Effect::SetAutostart(on)]
            }
            (SetupAction::ToggleDeleteData, Screen::UninstallConfirm { delete_data }) => {
                self.delete_data = !delete_data;
                self.screen = Screen::UninstallConfirm {
                    delete_data: self.delete_data,
                };
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A report from the worker. Only logs matter once the work has ended.
    pub fn on_report(&mut self, report: Report) -> Vec<Effect> {
        if let Report::Log(text) = report {
            return vec![Effect::Log(text)];
        }
        if !self.running() {
            return Vec::new();
        }
        match report {
            Report::Step(done) => {
                self.screen = match self.job {
                    Job::Install => Screen::Running { done },
                    Job::Uninstall => Screen::Uninstalling { done },
                };
                Vec::new()
            }
            Report::Failed { step, reason } => {
                let code = code_of(&reason);
                self.screen = Screen::Error {
                    step,
                    code,
                    busy: false,
                };
                vec![Effect::Log(format!("step {step} failed, code {code}"))]
            }
            Report::Finished => match self.job {
                Job::Install => {
                    let on = self.autostart;
                    self.screen = Screen::Done { autostart: on };
                    let mut effects = vec![Effect::SetAutostart(on)];
                    // An update was asked for from Plith: it comes back without a click.
                    if self.update {
                        effects.push(Effect::StartPlith);
                    }
                    effects
                }
                Job::Uninstall => {
                    self.screen = Screen::Uninstalled;
                    let mut effects = vec![Effect::SetAutostart(false)];
                    if self.delete_data {
                        effects.push(Effect::DeleteUserData);
                    }
                    effects
                }
            },
            Report::Log(_) => Vec::new(),
        }
    }

    /// The worker could not be started: `cancelled` is the user declining the Windows prompt.
    /// The app logs the cause itself.
    pub fn on_elevate_failed(&mut self, cancelled: bool) {
        self.screen = match (cancelled, self.job) {
            (true, Job::Install) => Screen::UacCancelled,
            (true, Job::Uninstall) => Screen::UninstallConfirm {
                delete_data: self.delete_data,
            },
            (false, job) => Screen::Error {
                step: silent_step(job),
                code: 0,
                busy: false,
            },
        };
    }

    /// The worker process ended. After its `Finished` or `Failed` that changes nothing; without
    /// one it is an error: exit 2 is a refusal, exit 3 another worker holding the lock.
    pub fn on_worker_exit(&mut self, exit: u32) -> Vec<Effect> {
        if !self.running() {
            return Vec::new();
        }
        self.screen = Screen::Error {
            step: silent_step(self.job),
            code: 0,
            busy: exit == 3,
        };
        let text = match exit {
            2 => "worker refused".to_string(),
            3 => "another worker is running".to_string(),
            other => format!("worker ended without a report, exit code {other}"),
        };
        vec![Effect::Log(text)]
    }
}
