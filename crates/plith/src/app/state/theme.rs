//! The theme paint the render thread is sent: the resolved palettes and, for a material theme,
//! its decoded art.

use super::App;
use plith_core::material::art::ThemePaint;
use plith_core::theme::Palette;
use plith_core::theme::registry::ResolvedTheme;

impl App {
    /// The paint for `resolved`. A material theme decodes its art here (once per scale, kept
    /// across palette-only switches); a plain theme costs nothing. A decode error is logged and
    /// the island falls back to the plain default paint.
    pub(super) fn theme_paint(&mut self, resolved: &ResolvedTheme) -> ThemePaint {
        if !resolved.material {
            return ThemePaint::new(resolved.open, resolved.collapsed, None);
        }
        match self.art.get(&resolved.drawn, self.scale) {
            Ok(art) => ThemePaint::new(resolved.open, resolved.collapsed, Some(art)),
            Err(error) => {
                self.log.line(&error);
                ThemePaint::plain(Palette::default())
            }
        }
    }

    /// Sends the current theme to the render thread again, when it is a material theme: the
    /// art depends on the display scale, and the render thread starts on a plain palette.
    pub fn resend_material(&mut self) {
        let resolved = self.resolved();
        if !resolved.material || self.relayout.is_none() {
            return;
        }
        let paint = self.theme_paint(&resolved);
        if let Some(relayout) = &self.relayout {
            relayout.set_theme(paint);
        }
    }

    /// Records the scale of the display the art is decoded for.
    pub fn set_display_scale(&mut self, scale: f64) {
        self.scale = scale;
    }
}
