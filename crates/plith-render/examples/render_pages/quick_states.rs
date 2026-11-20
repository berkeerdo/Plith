//! The quick page and every page's tab row (with the gear mark), for the gear review of R7: the
//! quick page with two looks picked, then the audio, media, shelf, system and welcome pages.

use super::Content;
use super::idle_states::{idle_facts, rest};
use plith_core::lang::Lang;
use plith_core::pages::PageId;
use plith_core::pages::audio::{PickMode, audio_page, audio_view};
use plith_core::pages::fixtures;
use plith_core::pages::idle_pill::idle_pill;
use plith_core::pages::media::media_page;
use plith_core::pages::quick::{QuickFacts, quick_page};
use plith_core::pages::shelf::shelf_page;
use plith_core::pages::system::system_page;
use plith_core::pages::welcome::welcome_page;
use plith_core::settings::IdleLook;
use std::sync::Arc;

fn quick(look: IdleLook, lang: Lang) -> Vec<Content> {
    let facts = idle_facts(lang);
    let quick = QuickFacts {
        look,
        previews: IdleLook::ALL.map(|each| Arc::new(idle_pill(each, &facts))),
        lang,
    };
    vec![Content::Page(PageId::Quick, quick_page(&quick))]
}

/// The states to render in `lang`, by file name stem.
pub fn states(lang: Lang) -> Vec<(&'static str, Vec<Content>)> {
    let clock = fixtures::CLOCK;
    let view = audio_view(&fixtures::lit(&fixtures::banana()), PickMode::None, lang);
    vec![
        ("quick-karot", quick(IdleLook::Karot, lang)),
        ("quick-seal", quick(IdleLook::Seal, lang)),
        (
            "audio",
            vec![Content::Page(PageId::Audio, audio_page(&view, clock, lang))],
        ),
        (
            "media",
            vec![Content::Page(
                PageId::Media,
                media_page(&fixtures::media_playing(), clock, lang),
            )],
        ),
        (
            "shelf",
            vec![Content::Page(PageId::Shelf, shelf_page(clock, lang))],
        ),
        (
            "system",
            vec![Content::Page(
                PageId::System,
                system_page(&fixtures::system(), clock, lang),
            )],
        ),
        (
            "welcome",
            vec![Content::Page(
                PageId::Welcome,
                welcome_page(&rest(lang), None),
            )],
        ),
    ]
}
