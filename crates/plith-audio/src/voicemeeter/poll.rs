//! The service-owned poll thread: dirty checks, snapshot rebuilds and level reads.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::api::Api;
use super::service::{Shared, Status};
use super::{
    ChannelSnapshot, DirtyOutcome, LoginSession, Rail, Readiness, VmKind, VmLevels, VmSnapshot,
    fallback_label, level_channel, level_type, param_name, read_routes,
};

/// Dirty check cadence.
const POLL: Duration = Duration::from_millis(50);
/// Levels are read every this many polls (so every 100 ms): they change constantly and cost
/// two API calls per strip and bus, unlike the dirty flag.
const LEVEL_DIVISOR: u32 = 2;
/// Polls between reconnect attempts after the engine went away (3 s).
const RECONNECT_EVERY: u32 = 60;

/// Reads one strip or bus. `None` when the gain cannot be read yet (engine still starting).
fn read_channel(api: &Api, kind: VmKind, rail: Rail, index: usize) -> Option<ChannelSnapshot> {
    let gain_db = api.get_float(&param_name(rail, index, "Gain"))?;
    let muted = api
        .get_float(&param_name(rail, index, "Mute"))
        .is_some_and(|v| v >= 0.5);
    let label = api
        .get_string(&param_name(rail, index, "Label"))
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| fallback_label(kind, rail, index));
    let routes = match rail {
        Rail::Strip => read_routes(kind, index, |name| api.get_float(name)),
        Rail::Bus => Vec::new(),
    };
    Some(ChannelSnapshot {
        rail,
        index,
        label,
        gain_db,
        muted,
        routes,
    })
}

/// Builds a full snapshot, or `None` when any part is not readable yet.
pub(super) fn build_snapshot(api: &Api) -> Option<VmSnapshot> {
    let kind = VmKind::from_api(api.voicemeeter_type()?)?;
    let strips = (0..kind.strip_count())
        .map(|i| read_channel(api, kind, Rail::Strip, i))
        .collect::<Option<Vec<_>>>()?;
    let buses = (0..kind.bus_count())
        .map(|i| read_channel(api, kind, Rail::Bus, i))
        .collect::<Option<Vec<_>>>()?;
    Some(VmSnapshot {
        kind,
        strips,
        buses,
    })
}

/// Peak of the left/right pair of one strip or bus.
fn read_pair(api: &Api, kind: VmKind, rail: Rail, index: usize) -> [f32; 2] {
    let Some(first) = level_channel(kind, rail, index).and_then(|c| i32::try_from(c).ok()) else {
        return [0.0, 0.0];
    };
    let ty = level_type(rail);
    [
        api.level(ty, first).unwrap_or(0.0),
        api.level(ty, first + 1).unwrap_or(0.0),
    ]
}

/// Reads the peak levels of every strip and bus.
pub(super) fn read_levels(api: &Api, kind: VmKind) -> VmLevels {
    VmLevels {
        strips: (0..kind.strip_count())
            .map(|i| read_pair(api, kind, Rail::Strip, i))
            .collect(),
        buses: (0..kind.bus_count())
            .map(|i| read_pair(api, kind, Rail::Bus, i))
            .collect(),
    }
}

/// The status that goes with a readiness verdict.
pub(super) fn status_for(readiness: Readiness) -> Status {
    match readiness {
        Readiness::Ready => Status::Running,
        Readiness::AppNotRunning => Status::Unavailable("Voicemeeter is not running".into()),
    }
}

/// Mutable state of the loop.
struct Poller {
    api: Arc<Api>,
    shared: Arc<Shared>,
    session: LoginSession,
    connected: bool,
    need_snapshot: bool,
    /// Logged in but the application is not launched; cleared by the first built snapshot.
    awaiting_app: bool,
    kind: Option<VmKind>,
    tick: u32,
}

impl Poller {
    /// Engine went away: clear what consumers see and start reconnecting.
    fn on_gone(&mut self) {
        self.connected = false;
        self.kind = None;
        self.tick = 0;
        self.shared
            .set_status(Status::Unavailable("the Voicemeeter engine stopped".into()));
        self.shared.publish(None);
        self.shared.set_levels(VmLevels::default());
    }

    /// Defensive Logout then Login. This is the only place Logout precedes Login.
    fn try_reconnect(&mut self) {
        self.api.logout();
        let rc = self.api.login();
        if rc >= 0 {
            self.session = LoginSession::on_login(rc, Instant::now());
            self.connected = true;
            self.need_snapshot = true;
            let readiness = Readiness::from_login(rc, false);
            self.awaiting_app = readiness.keeps_session_on_gone();
            self.shared.set_status(status_for(readiness));
        }
    }

    fn step_connected(&mut self) {
        let outcome = self.session.on_dirty(self.api.is_dirty(), Instant::now());
        match outcome {
            DirtyOutcome::Dirty => self.need_snapshot = true,
            DirtyOutcome::EngineGone if self.awaiting_app => {
                // The application was never launched: the API attaches when it starts, so
                // re-arm the grace instead of a Logout/Login cycle.
                self.session = LoginSession::on_login(1, Instant::now());
            }
            DirtyOutcome::EngineGone => return self.on_gone(),
            DirtyOutcome::Clean | DirtyOutcome::Transient => {}
        }
        if self.need_snapshot
            && let Some(snapshot) = build_snapshot(&self.api)
        {
            self.need_snapshot = false;
            self.kind = Some(snapshot.kind);
            if self.awaiting_app {
                self.awaiting_app = false;
                self.shared.set_status(Status::Running);
            }
            self.shared.publish(Some(&snapshot));
        }
        if let Some(kind) = self.kind
            && self.tick.is_multiple_of(LEVEL_DIVISOR)
        {
            self.shared.set_levels(read_levels(&self.api, kind));
        }
    }

    fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.connected {
            self.step_connected();
        } else if self.tick.is_multiple_of(RECONNECT_EVERY) {
            self.try_reconnect();
        }
    }
}

/// Thread body: polls until the stop flag is set. Never calls Logout (see the service).
pub(super) fn run(api: Arc<Api>, shared: Arc<Shared>, session: LoginSession, readiness: Readiness) {
    let mut poller = Poller {
        api,
        shared,
        session,
        connected: true,
        need_snapshot: true,
        awaiting_app: readiness.keeps_session_on_gone(),
        kind: None,
        tick: 0,
    };
    while !poller.shared.stop.load(Ordering::Acquire) {
        poller.step();
        thread::sleep(POLL);
    }
}
