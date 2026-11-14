//! The `mask` candidate: a layered window whose pixels are alpha 1 inside the island and alpha 0
//! outside, so only the island receives input.

use super::region_px;
use plith_core::geometry::IslandShape;
use plith_core::layout::SurfaceLayout;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};

/// Premultiplied BGRA pixel with alpha 1 and black colour.
const INSIDE: u32 = 0x0100_0000;

/// A surface-sized top-down 32-bit DIB selected into a memory DC.
#[derive(Debug)]
pub(crate) struct MaskBuffer {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl MaskBuffer {
    /// Creates the buffer for a surface of `width` x `height` pixels.
    pub(crate) fn new(width: i32, height: i32) -> windows::core::Result<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).unwrap_or(0),
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: a null source DC asks for a memory DC compatible with the screen.
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err(windows::core::Error::from_thread());
        }
        // SAFETY: `info` and `bits` are valid for the call; with no file mapping the system
        // allocates the pixel memory and returns it through `bits`.
        let bitmap = unsafe {
            CreateDIBSection(
                Some(dc),
                &raw const info,
                DIB_RGB_COLORS,
                &raw mut bits,
                None,
                0,
            )
        };
        let bitmap = match bitmap {
            Ok(bitmap) => bitmap,
            Err(error) => {
                // SAFETY: `dc` was created above and is not selected into anything.
                let _ = unsafe { DeleteDC(dc) };
                return Err(error);
            }
        };
        // SAFETY: `dc` and `bitmap` are live and the bitmap is not selected elsewhere.
        let previous = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
        Ok(Self {
            dc,
            bitmap,
            previous,
            bits: bits.cast::<u32>(),
            width,
            height,
        })
    }

    fn fill(&mut self, shape: IslandShape, layout: &SurfaceLayout) {
        let len = usize::try_from(self.width * self.height).unwrap_or(0);
        // SAFETY: `bits` points at `width * height` u32 pixels owned by the DIB section, which
        // lives as long as `self`; `&mut self` makes this the only access.
        let pixels = unsafe { core::slice::from_raw_parts_mut(self.bits, len) };
        pixels.fill(0);
        let (ox, oy) = layout.island_origin(shape);
        let bounds = region_px(shape, layout, 0);
        for y in bounds.top.max(0)..bounds.bottom.min(self.height) {
            let dy = (f64::from(y) + 0.5) / layout.scale - oy;
            for x in bounds.left.max(0)..bounds.right.min(self.width) {
                let dx = (f64::from(x) + 0.5) / layout.scale - ox;
                if shape.contains(dx, dy) {
                    let index = usize::try_from(y * self.width + x).unwrap_or(0);
                    pixels[index] = INSIDE;
                }
            }
        }
    }

    /// Redraws the mask for `shape` and pushes it to the layered window `hwnd`.
    pub(crate) fn apply(
        &mut self,
        hwnd: isize,
        shape: IslandShape,
        layout: &SurfaceLayout,
    ) -> windows::core::Result<()> {
        self.fill(shape, layout);
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: u8::try_from(AC_SRC_OVER).unwrap_or(0),
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: u8::try_from(AC_SRC_ALPHA).unwrap_or(1),
        };
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // SAFETY: every pointer argument refers to a local that outlives the call, `self.dc`
        // holds a live 32-bit DIB and `hwnd` is a layered window of this thread. A null
        // destination position keeps the window where it is.
        unsafe {
            UpdateLayeredWindow(
                hwnd,
                None,
                None,
                Some(&raw const size),
                Some(self.dc),
                Some(&raw const origin),
                COLORREF(0),
                Some(&raw const blend),
                ULW_ALPHA,
            )
        }
    }
}

impl Drop for MaskBuffer {
    fn drop(&mut self) {
        // SAFETY: the DC and bitmap were created in `new` and are released exactly once; the
        // original bitmap is selected back before the DIB is deleted.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
