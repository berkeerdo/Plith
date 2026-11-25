//! The tray icon's commands as the app takes them: the settings window and a clean quit.

use super::App;
use plith_core::lang::Lang;
use plith_core::tray::menu::{TrayItem, menu};
use plith_shell::{QuitHandle, TrayHandle};

/// Closes the app the way `WM_CLOSE` does. A trait so tests need no window.
pub trait Quit {
    /// Asks the UI thread to close the window and leave the message loop.
    fn quit(&self);
}

impl Quit for QuitHandle {
    fn quit(&self) {
        QuitHandle::quit(self);
    }
}

/// Shows the update balloon. A trait so tests need no window.
pub trait Balloon {
    /// Shows `title` and `body` over the tray icon.
    fn show(&self, title: &str, body: &str);
}

impl Balloon for TrayHandle {
    fn show(&self, title: &str, body: &str) {
        self.notice(title, body);
    }
}

/// Names the reason the user must not be disturbed now, or `None`.
type Busy = Box<dyn Fn() -> Option<&'static str>>;

/// What the app keeps of the tray: the menu captions' handle, the balloon and the way out.
#[derive(Default)]
pub struct TrayHost {
    handle: Option<TrayHandle>,
    quit: Option<Box<dyn Quit>>,
    balloon: Option<Box<dyn Balloon>>,
    busy: Option<Busy>,
}

impl TrayHost {
    /// Why the user is busy, or `None` (also when no check is wired).
    pub(super) fn busy(&self) -> Option<&'static str> {
        self.busy.as_ref().and_then(|busy| busy())
    }

    /// Shows the balloon; false when none is wired.
    pub(super) fn show_notice(&self, title: &str, body: &str) -> bool {
        self.balloon.as_ref().is_some_and(|balloon| {
            balloon.show(title, body);
            true
        })
    }
}

impl App {
    /// Wires the tray: `handle` replaces the menu captions on a language change, `quit` is
    /// what the menu's last item does.
    pub fn attach_tray(&mut self, handle: Option<TrayHandle>, quit: impl Quit + 'static) {
        self.tray = TrayHost {
            handle,
            quit: Some(Box::new(quit)),
            ..std::mem::take(&mut self.tray)
        };
    }

    /// Wires the update balloon: `balloon` shows it, `busy` says whether the user must not be
    /// disturbed right now (a game, a presentation).
    pub fn attach_notice(
        &mut self,
        balloon: impl Balloon + 'static,
        busy: impl Fn() -> Option<&'static str> + 'static,
    ) {
        self.tray.balloon = Some(Box::new(balloon));
        self.tray.busy = Some(Box::new(busy));
    }

    /// A tray command: a left click and the first menu item open the settings window (the path
    /// of the quick page's `Tüm ayarlar`), the last item closes the app like `WM_CLOSE`.
    pub(super) fn on_tray(&mut self, item: TrayItem) {
        match item {
            TrayItem::Settings => {
                self.log.line("tray: open settings");
                self.open_sheet();
            }
            TrayItem::Quit => {
                self.log.line("tray: quit");
                if let Some(quit) = &self.tray.quit {
                    quit.quit();
                }
            }
        }
    }

    /// Gives the menu its captions in `lang`.
    pub(super) fn sync_tray(&self, lang: Lang) {
        if let Some(handle) = &self.tray.handle {
            handle.set_menu(&menu(lang));
        }
    }
}

#[cfg(test)]
mod tests;
