//! Cross-thread handoff: the UI thread publishes the wanted scene, the render thread parks on
//! it, and publishes the drawn shape back for hit testing.

use crate::geometry::IslandShape;
use crate::shapes::{PAGES, REST};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// Reasons the island must not be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Suppression {
    /// The display is off.
    pub display_off: bool,
    /// The session is locked.
    pub locked: bool,
    /// Another window is fullscreen.
    pub fullscreen: bool,
}

impl Suppression {
    /// True when any reason applies.
    #[must_use]
    pub fn any(&self) -> bool {
        self.display_off || self.locked || self.fullscreen
    }
}

/// What the UI thread wants on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Scene {
    /// Whether the island is expanded.
    pub expanded: bool,
    /// The wanted page index when expanded.
    pub page: usize,
    /// Why the island is hidden, if it is.
    pub suppression: Suppression,
}

impl Scene {
    /// The shape the island should morph to.
    #[must_use]
    pub fn target(&self) -> IslandShape {
        if !self.expanded {
            return REST;
        }
        debug_assert!(self.page < PAGES.len(), "page {} out of range", self.page);
        PAGES[self.page.min(PAGES.len() - 1)]
    }
}

#[derive(Debug)]
struct State {
    scene: Scene,
    generation: u64,
    stopped: bool,
}

/// Latest-wins mailbox from the UI thread to the render thread.
#[derive(Debug)]
pub struct SceneMailbox {
    state: Mutex<State>,
    changed: Condvar,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl SceneMailbox {
    /// A mailbox holding `scene` at generation 0.
    #[must_use]
    pub fn new(scene: Scene) -> Self {
        Self {
            state: Mutex::new(State {
                scene,
                generation: 0,
                stopped: false,
            }),
            changed: Condvar::new(),
        }
    }

    /// Replaces the scene, wakes the waiter and returns the new generation.
    pub fn publish(&self, scene: Scene) -> u64 {
        let mut state = lock(&self.state);
        state.scene = scene;
        state.generation += 1;
        let generation = state.generation;
        drop(state);
        self.changed.notify_all();
        generation
    }

    /// Reads the scene, applies `change` and publishes the result under one lock, so no other
    /// publisher can slip in between. Bumps the generation once and returns it.
    pub fn update(&self, change: impl FnOnce(Scene) -> Scene) -> u64 {
        let mut state = lock(&self.state);
        state.scene = change(state.scene);
        state.generation += 1;
        let generation = state.generation;
        drop(state);
        self.changed.notify_all();
        generation
    }

    /// The current scene and its generation, without blocking.
    #[must_use]
    pub fn latest(&self) -> (Scene, u64) {
        let state = lock(&self.state);
        (state.scene, state.generation)
    }

    /// Blocks until the generation is newer than `seen`; `None` once stopped.
    #[must_use]
    pub fn wait_newer(&self, seen: u64) -> Option<(Scene, u64)> {
        let mut state = lock(&self.state);
        loop {
            if state.stopped {
                return None;
            }
            if state.generation > seen {
                return Some((state.scene, state.generation));
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Releases every waiter; later waits return `None`.
    pub fn stop(&self) {
        lock(&self.state).stopped = true;
        self.changed.notify_all();
    }
}

/// The shape the render thread drew, for hit testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeFrame {
    /// The drawn shape.
    pub shape: IslandShape,
    /// The scene generation this frame was drawn for.
    pub generation: u64,
}

/// Holds the latest drawn shape.
#[derive(Debug, Default)]
pub struct ShapeCell {
    frame: Mutex<Option<ShapeFrame>>,
}

impl ShapeCell {
    /// Stores the latest frame.
    pub fn publish(&self, frame: ShapeFrame) {
        *lock(&self.frame) = Some(frame);
    }

    /// The latest frame, if any was published.
    #[must_use]
    pub fn latest(&self) -> Option<ShapeFrame> {
        *lock(&self.frame)
    }
}

#[cfg(test)]
mod tests;
