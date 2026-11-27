//! The pictures a material theme draws with and the paint the app sends to the render thread.
//!
//! Pure data: the app decodes the pack's PNGs once per scale and shares them by `Arc`; the
//! render thread turns them into bitmaps on its own device. The pixel format is that of
//! [`ArtPixels`]: premultiplied BGRA, `width * 4` bytes per row, the format the renderer already
//! uses for album art, so bitmap creation needs no conversion.

use crate::draw::ArtPixels;
use crate::theme::Palette;
use crate::theme::registry::ThemeId;
use std::sync::Arc;

/// The decoded rasters of a material pack (spec `2026-11-26-halka-island-design.md`, section 3).
/// Each picture keeps its own pixel size.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialArt {
    /// The fabric floor (opaque).
    pub fabric: Arc<ArtPixels>,
    /// The three ripple rings, innermost first.
    pub rings: [Arc<ArtPixels>; 3],
    /// The source disc.
    pub disc: Arc<ArtPixels>,
}

/// What the render thread paints a theme with: the two palettes and, for a material theme, its
/// pictures.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemePaint {
    /// The pack drawn (the motion log line names it).
    pub drawn: ThemeId,
    /// The open island's colours.
    pub open: Palette,
    /// The collapsed pill's colours.
    pub collapsed: Palette,
    /// The material's pictures; `None` for a plain colour theme.
    pub material: Option<Arc<MaterialArt>>,
}

impl ThemePaint {
    /// A plain theme drawn as Gece: one palette for both faces, no material.
    #[must_use]
    pub fn plain(palette: Palette) -> Self {
        Self {
            drawn: ThemeId::gece(),
            open: palette,
            collapsed: palette,
            material: None,
        }
    }

    /// The pack `drawn`, the two palettes of a resolved theme and its pictures (`None` for a
    /// plain theme).
    #[must_use]
    pub fn new(
        drawn: ThemeId,
        open: Palette,
        collapsed: Palette,
        material: Option<Arc<MaterialArt>>,
    ) -> Self {
        Self {
            drawn,
            open,
            collapsed,
            material,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::ArtId;

    fn pixels(id: u64) -> Arc<ArtPixels> {
        Arc::new(ArtPixels {
            id: ArtId(id),
            width: 1,
            height: 1,
            bgra: Arc::from([0u8, 0, 0, 255]),
        })
    }

    #[test]
    fn plain_paint_has_one_palette_and_no_material() {
        let paint = ThemePaint::plain(Palette::default());
        assert_eq!(paint.open, paint.collapsed);
        assert!(paint.material.is_none());
        assert_eq!(paint.drawn, ThemeId::gece());
    }

    #[test]
    fn material_paint_keeps_its_art() {
        let art = Arc::new(MaterialArt {
            fabric: pixels(1),
            rings: [pixels(2), pixels(3), pixels(4)],
            disc: pixels(5),
        });
        let halka = ThemeId::parse("halka").expect("id");
        let paint = ThemePaint::new(
            halka.clone(),
            Palette::default(),
            Palette::setup(),
            Some(Arc::clone(&art)),
        );
        assert_eq!(paint.drawn, halka);
        assert!(Arc::ptr_eq(paint.material.as_ref().unwrap(), &art));
        assert_ne!(paint.open, paint.collapsed);
    }
}
