//! Cross-thread handoff: the UI thread publishes the wanted scene, the render thread parks on
//! it, and publishes the drawn shape back for hit testing.

use crate::draw::PageContent;
use crate::geometry::IslandShape;
use crate::shapes::{PAGES, REST};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scene {
    /// Whether the island is expanded.
    pub expanded: bool,
    /// The wanted page index when expanded.
    pub page: usize,
    /// Why the island is hidden, if it is.
    pub suppression: Suppression,
    /// The shape of each page; `target` reads the one at `page`.
    pub shapes: [IslandShape; 4],
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            expanded: false,
            page: 0,
            suppression: Suppression::default(),
            shapes: PAGES,
        }
    }
}

impl Scene {
    /// The shape the island should morph to.
    #[must_use]
    pub fn target(&self) -> IslandShape {
        if !self.expanded {
            return REST;
        }
        debug_assert!(
            self.page < self.shapes.len(),
            "page {} out of range",
            self.page
        );
        self.shapes[self.page.min(self.shapes.len() - 1)]
    }
}

/// The scene and the page content it is drawn with, at one generation.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The wanted scene.
    pub scene: Scene,
    /// The display lists and artwork.
    pub content: Arc<PageContent>,
    /// The mailbox generation this snapshot was taken at.
    pub generation: u64,
}

#[derive(Debug)]
struct State {
    scene: Scene,
    content: Arc<PageContent>,
    generation: u64,
    stopped: bool,
}

impl State {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            scene: self.scene,
            content: Arc::clone(&self.content),
            generation: self.generation,
        }
    }
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
    /// A mailbox holding `scene` and `content` at generation 0.
    #[must_use]
    pub fn new(scene: Scene, content: Arc<PageContent>) -> Self {
        Self {
            state: Mutex::new(State {
                scene,
                content,
                generation: 0,
                stopped: false,
            }),
            changed: Condvar::new(),
        }
    }

    /// Bumps the generation and wakes the waiter; the caller holds the lock.
    fn bump(&self, mut state: MutexGuard<'_, State>) -> u64 {
        state.generation += 1;
        let generation = state.generation;
        drop(state);
        self.changed.notify_all();
        generation
    }

    /// Replaces the scene, wakes the waiter and returns the new generation; `None` (no bump,
    /// no wake) when the scene is unchanged.
    pub fn publish(&self, scene: Scene) -> Option<u64> {
        self.update(|_| scene)
    }

    /// Reads the scene, applies `change` and publishes the result under one lock, so no other
    /// publisher can slip in between. Bumps the generation once and returns it; `None` (no
    /// bump, no wake) when `change` leaves the scene as it was.
    pub fn update(&self, change: impl FnOnce(Scene) -> Scene) -> Option<u64> {
        let mut state = lock(&self.state);
        let next = change(state.scene);
        if next == state.scene {
            return None;
        }
        state.scene = next;
        Some(self.bump(state))
    }

    /// Replaces the page content; `None` (no bump, no wake) when it equals the current one.
    ///
    /// The deep compare runs outside the lock, so the render thread never waits on it: the
    /// stored `Arc` is cloned under the lock, compared without it, and the new content is stored
    /// only if the stored `Arc` is still the one compared against. A lost race retries once;
    /// a second lost race compares under the lock.
    pub fn publish_content(&self, content: Arc<PageContent>) -> Option<u64> {
        for _ in 0..2 {
            let current = Arc::clone(&lock(&self.state).content);
            if Arc::ptr_eq(&current, &content) || *current == *content {
                return None;
            }
            let mut state = lock(&self.state);
            if Arc::ptr_eq(&state.content, &current) {
                state.content = content;
                return Some(self.bump(state));
            }
        }
        let mut state = lock(&self.state);
        if Arc::ptr_eq(&state.content, &content) || *state.content == *content {
            return None;
        }
        state.content = content;
        Some(self.bump(state))
    }

    /// The current scene, content and generation, without blocking.
    #[must_use]
    pub fn latest(&self) -> Snapshot {
        lock(&self.state).snapshot()
    }

    /// Blocks until the generation is newer than `seen`; `None` once stopped.
    #[must_use]
    pub fn wait_newer(&self, seen: u64) -> Option<Snapshot> {
        let mut state = lock(&self.state);
        loop {
            if state.stopped {
                return None;
            }
            if state.generation > seen {
                return Some(state.snapshot());
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
    /// The shape of the frame painted before this one; equal to `shape` for the first frame
    /// after the start or after a snap. With a frame queue the display may still show it.
    pub previous: IslandShape,
    /// The render thread's frame counter: 1 for its first painted frame, then increasing.
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
