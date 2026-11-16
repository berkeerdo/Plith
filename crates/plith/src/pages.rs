//! Facts to page content: builds the four pages and the pill, keeps their hit targets and
//! shapes, and rebuilds only the pages a change touches.

use crate::services::{Changes, Facts};
use plith_core::draw::{ArtPixels, DisplayList, PageContent};
use plith_core::geometry::IslandShape;
use plith_core::pages::audio::{Activity, PickMode, audio_page, audio_view};
use plith_core::pages::media::media_page;
use plith_core::pages::pill::pill;
use plith_core::pages::shelf::shelf_page;
use plith_core::pages::system::system_page;
use plith_core::pages::{HitTarget, PageBuild, PageId};
use std::sync::Arc;

/// What a build reads besides the page id.
pub struct Inputs<'a> {
    /// The facts.
    pub facts: &'a Facts,
    /// Signal activity per strip.
    pub activity: &'a Activity,
    /// The audio page's pick mode.
    pub pick: PickMode,
    /// App time in seconds, for the activity hold.
    pub now_s: f64,
    /// The tab bar clock, `HH:MM`.
    pub clock: &'a str,
}

/// Which lists need a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dirty {
    /// One flag per page, in tab order.
    pub pages: [bool; 4],
    /// The pill.
    pub pill: bool,
}

impl Dirty {
    /// The lists a change touches: audio facts reach every page but the shelf (the media route
    /// names the output, the system page the microphone) and the pill; media reaches its page
    /// and the pill; brightness only the system page.
    #[must_use]
    pub fn from_changes(changes: Changes) -> Dirty {
        let audio = changes.has(Changes::AUDIO);
        let media = changes.has(Changes::MEDIA);
        let system = changes.has(Changes::SYSTEM);
        Dirty {
            pages: [audio, audio || media, false, audio || system],
            pill: audio || media,
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
            let view = audio_view(&facts.audio, inputs.activity, inputs.pick, inputs.now_s);
            audio_page(&view, inputs.clock)
        }
        PageId::Media => media_page(&facts.media, inputs.clock),
        PageId::Shelf => shelf_page(inputs.clock),
        PageId::System => system_page(&facts.system, inputs.clock),
    }
}

/// The built pages and pill.
pub struct Pages {
    lists: [Arc<DisplayList>; 4],
    targets: [Vec<HitTarget>; 4],
    shapes: [IslandShape; 4],
    pill: Arc<DisplayList>,
    art: Vec<Arc<ArtPixels>>,
}

impl Pages {
    /// Every page and the pill, built from `inputs`.
    #[must_use]
    pub fn new(inputs: &Inputs) -> Self {
        let [a, b, c, d] = PageId::ALL.map(|page| build_page(page, inputs));
        Self {
            shapes: [a.shape, b.shape, c.shape, d.shape],
            lists: [a.list, b.list, c.list, d.list].map(Arc::new),
            targets: [a.targets, b.targets, c.targets, d.targets],
            pill: Arc::new(pill(&inputs.facts.pill)),
            art: art(inputs.facts),
        }
    }

    /// Rebuilds the lists `dirty` names; the others keep their `Arc`.
    pub fn rebuild(&mut self, dirty: Dirty, inputs: &Inputs) {
        for page in PageId::ALL {
            let i = page.index();
            if !dirty.pages[i] {
                continue;
            }
            let build = build_page(page, inputs);
            self.shapes[i] = build.shape;
            self.targets[i] = build.targets;
            self.lists[i] = Arc::new(build.list);
        }
        if dirty.pill {
            self.pill = Arc::new(pill(&inputs.facts.pill));
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
    pub fn shapes(&self) -> [IslandShape; 4] {
        self.shapes
    }

    /// The hit targets of every page, in tab order.
    #[must_use]
    pub fn targets(&self) -> &[Vec<HitTarget>; 4] {
        &self.targets
    }
}

/// The artwork the media page references.
fn art(facts: &Facts) -> Vec<Arc<ArtPixels>> {
    facts.art.iter().cloned().collect()
}

#[cfg(test)]
mod tests;
