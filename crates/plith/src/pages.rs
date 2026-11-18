//! Facts to page content: builds the five pages and the pill, keeps their hit targets and
//! shapes, and rebuilds only the pages a change touches.

use crate::services::Facts;
use plith_core::draw::{ArtPixels, DisplayList, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::lang::Lang;
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::idle_pill::{IdleFacts, idle_pill};
use plith_core::pages::media::media_page;
use plith_core::pages::pill::pill;
use plith_core::pages::shelf::shelf_page;
use plith_core::pages::system::system_page;
use plith_core::pages::tabbar::tabbar;
use plith_core::pages::welcome::{WelcomeFacts, welcome_page};
use plith_core::pages::{HitTarget, PAGE_COUNT, PAGE_W, PageBuild, PageId};
use plith_core::settings::IdleLook;
use plith_core::shapes::WELCOME;
use std::sync::Arc;

/// What a build reads besides the page id.
pub struct Inputs<'a> {
    /// The facts.
    pub facts: &'a Facts,
    /// The audio page's pick mode.
    pub pick: PickMode,
    /// The tab bar clock, `HH:MM`.
    pub clock: &'a str,
    /// The welcome view's facts; `None` builds the placeholder (set by the app wiring).
    pub welcome: Option<&'a WelcomeFacts>,
    /// The hour cell under the pointer on the welcome view.
    pub probe: Option<i32>,
    /// The resting look and its facts; `None` draws the seal (before the app wiring sets them).
    pub idle: Option<(IdleLook, &'a IdleFacts)>,
    /// The interface language, resolved once at start.
    pub lang: Lang,
}

/// Which lists need a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dirty {
    /// One flag per page, in page order (the welcome view last).
    pub pages: [bool; PAGE_COUNT],
    /// The pill.
    pub pill: bool,
}

impl Dirty {
    /// The lists whose facts differ between `old` and `new`. Each page's facts already hold
    /// what it reads from other services (the media route names the output, the system page
    /// the microphone and its bus), so an audio change reaches the system page only when the
    /// microphone facts changed. The shelf reads no service.
    #[must_use]
    pub fn between(old: &Facts, new: &Facts) -> Dirty {
        let art = |facts: &Facts| facts.art.as_ref().map(|a| a.id);
        Dirty {
            pages: [
                old.audio != new.audio,
                old.media != new.media || art(old) != art(new),
                false,
                old.system != new.system,
                // The welcome view's facts mix services, the play log and the forecast, so the
                // feed compares them itself and joins the result with `union`.
                false,
            ],
            pill: old.pill != new.pill,
        }
    }

    /// Every list either side names.
    #[must_use]
    pub fn union(self, other: Dirty) -> Dirty {
        let mut pages = self.pages;
        for (own, theirs) in pages.iter_mut().zip(other.pages) {
            *own |= theirs;
        }
        Dirty {
            pages,
            pill: self.pill || other.pill,
        }
    }

    /// Only `page`.
    #[must_use]
    pub fn page(page: PageId) -> Dirty {
        let mut dirty = Dirty::default();
        dirty.pages[page.index()] = true;
        dirty
    }

    /// True when anything needs a rebuild.
    #[must_use]
    pub fn any(self) -> bool {
        self.pill || self.pages.iter().any(|p| *p)
    }
}

/// Builds one page.
#[must_use]
pub fn build_page(page: PageId, inputs: &Inputs) -> PageBuild {
    let facts = inputs.facts;
    match page {
        PageId::Audio => {
            let view = audio_view(&facts.audio, inputs.pick, inputs.lang);
            audio_page(&view, inputs.clock, inputs.lang)
        }
        PageId::Media => media_page(&facts.media, inputs.clock, inputs.lang),
        PageId::Shelf => shelf_page(inputs.clock, inputs.lang),
        PageId::System => system_page(&facts.system, inputs.clock, inputs.lang),
        PageId::Welcome => inputs.welcome.map_or_else(
            || welcome_placeholder(inputs.lang),
            |f| welcome_page(f, inputs.probe),
        ),
    }
}

/// The pill: the resting look when the app wiring set one, else the seal.
fn pill_of(inputs: &Inputs) -> DisplayList {
    match inputs.idle {
        Some((look, facts)) => idle_pill(look, facts),
        None => pill(&inputs.facts.pill, inputs.lang),
    }
}

/// The welcome slot until the app wiring builds the real view: the bare tab bar (no station
/// filled, no clock) at the welcome size, so a click still reaches every tab.
fn welcome_placeholder(lang: Lang) -> PageBuild {
    let (nodes, targets) = tabbar(None, "", lang);
    PageBuild {
        list: DisplayList {
            nodes,
            size: (PAGE_W, WELCOME.height),
        },
        targets,
        shape: WELCOME,
    }
}

/// The built pages and pill.
pub struct Pages {
    lists: [Arc<DisplayList>; PAGE_COUNT],
    targets: [Vec<HitTarget>; PAGE_COUNT],
    shapes: [IslandShape; PAGE_COUNT],
    pill: Arc<DisplayList>,
    art: Vec<Arc<ArtPixels>>,
}

impl Pages {
    /// Every page and the pill, built from `inputs`.
    #[must_use]
    pub fn new(inputs: &Inputs) -> Self {
        let [audio, media, shelf, system, welcome] =
            PageId::ALL.map(|page| build_page(page, inputs));
        Self {
            shapes: [
                audio.shape,
                media.shape,
                shelf.shape,
                system.shape,
                welcome.shape,
            ],
            lists: [
                audio.list,
                media.list,
                shelf.list,
                system.list,
                welcome.list,
            ]
            .map(Arc::new),
            targets: [
                audio.targets,
                media.targets,
                shelf.targets,
                system.targets,
                welcome.targets,
            ],
            pill: Arc::new(pill_of(inputs)),
            art: art(inputs.facts),
        }
    }

    /// Rebuilds the lists `dirty` names. The others, and a rebuilt list equal to the old one,
    /// keep their `Arc`, so the render cache (keyed on it) keeps its bitmap.
    pub fn rebuild(&mut self, dirty: Dirty, inputs: &Inputs) {
        for page in PageId::ALL {
            let i = page.index();
            if !dirty.pages[i] {
                continue;
            }
            let build = build_page(page, inputs);
            self.shapes[i] = build.shape;
            self.targets[i] = build.targets;
            keep_or_replace(&mut self.lists[i], build.list);
        }
        if dirty.pill {
            keep_or_replace(&mut self.pill, pill_of(inputs));
        }
        if dirty.pages[PageId::Media.index()] {
            self.art = art(inputs.facts);
        }
    }

    /// The content to publish.
    #[must_use]
    pub fn content(&self) -> PageContent {
        PageContent {
            pages: self.lists.clone(),
            pill: Arc::clone(&self.pill),
            art: self.art.clone(),
        }
    }

    /// The island shape of each page.
    #[must_use]
    pub fn shapes(&self) -> [IslandShape; PAGE_COUNT] {
        self.shapes
    }

    /// The hit targets of every page, in page order.
    #[must_use]
    pub fn targets(&self) -> &[Vec<HitTarget>; PAGE_COUNT] {
        &self.targets
    }
}

/// Replaces `slot` with `list` unless they are equal.
fn keep_or_replace(slot: &mut Arc<DisplayList>, list: DisplayList) {
    if **slot != list {
        *slot = Arc::new(list);
    }
}

/// The artwork the media page references.
fn art(facts: &Facts) -> Vec<Arc<ArtPixels>> {
    facts.art.iter().cloned().collect()
}

#[cfg(test)]
mod tests;
