//! The DirectComposition swap chain bound to the notch window.

use plith_core::layout::SurfaceLayout;
use plith_core::levers::{LeverOutcome, Levers};
use windows::Win32::Foundation::{E_INVALIDARG, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    ID2D1Image,
};
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGISurface, IDXGISwapChain1,
};
use windows::core::{Error, Result};

use crate::device::Gpu;
use crate::levers::apply_latency;

/// A flip-model swap chain shown through DirectComposition on one window.
pub struct CompositionTarget {
    swap_chain: IDXGISwapChain1,
    sync: u32,
    latency_outcome: Option<LeverOutcome>,
    // Kept alive: dropping the target or the visual detaches the content from the window.
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

/// Wraps the swap chain back buffer as the Direct2D target of the context.
fn bind_back_buffer(gpu: &Gpu, swap_chain: &IDXGISwapChain1, scale: f64) -> Result<()> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: std::mem::ManuallyDrop::new(None),
    };
    // SAFETY: buffer 0 exists on a swap chain with two buffers; `props` is live for the call.
    unsafe {
        let surface: IDXGISurface = swap_chain.GetBuffer(0)?;
        let bitmap = gpu
            .context()
            .CreateBitmapFromDxgiSurface(&surface, Some(&raw const props))?;
        gpu.context().SetTarget(&bitmap);
        #[allow(clippy::cast_possible_truncation)]
        let dpi = (96.0 * scale) as f32;
        gpu.context().SetDpi(dpi, dpi);
    }
    Ok(())
}

impl CompositionTarget {
    /// Creates the swap chain sized to `layout.surface`, attaches it to `hwnd` and makes the
    /// back buffer the Direct2D target of `gpu.context()` at `96 * scale` DPI. `levers.latency`
    /// sets the maximum frame latency and `levers.sync` the present interval.
    ///
    /// # Errors
    /// Returns the first failing Windows call.
    pub fn new(gpu: &Gpu, hwnd: HWND, layout: &SurfaceLayout, levers: Levers) -> Result<Self> {
        let (Ok(width @ 1..), Ok(height @ 1..)) = (
            u32::try_from(layout.surface.width),
            u32::try_from(layout.surface.height),
        ) else {
            return Err(Error::from(E_INVALIDARG));
        };
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            ..Default::default()
        };
        // SAFETY: plain COM calls on live interfaces; `desc` outlives the call that reads it.
        let (swap_chain, target, visual, latency_outcome) = unsafe {
            let swap_chain = gpu.dxgi_factory().CreateSwapChainForComposition(
                gpu.d3d(),
                &raw const desc,
                None,
            )?;
            let latency_outcome = apply_latency(gpu.dxgi_device(), levers.latency)?;
            let dcomp = gpu.dcomp();
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            (swap_chain, target, visual, latency_outcome)
        };
        bind_back_buffer(gpu, &swap_chain, layout.scale)?;
        Ok(Self {
            swap_chain,
            sync: u32::from(levers.sync),
            latency_outcome,
            _target: target,
            _visual: visual,
        })
    }

    /// Drops the context's reference to the back buffer. Call it before `ResizeBuffers` or
    /// before recreating the device: a swap chain cannot resize while a target still holds it.
    pub fn release_target(&self, gpu: &Gpu) {
        // SAFETY: clearing the target of a live context is always valid.
        unsafe { gpu.context().SetTarget(None::<&ID2D1Image>) };
    }

    /// The outcome of the latency lever, once (`None` after it was taken).
    pub fn take_outcome(&mut self) -> Option<LeverOutcome> {
        self.latency_outcome.take()
    }

    /// Presents the back buffer with the configured sync interval. With frame latency 2 it
    /// returns at once unless two frames are already queued. With 2 flip-sequential buffers the
    /// second queued frame waits on the GPU for a back buffer, a wait that neither `draw_ms`
    /// nor `present_ms` shows.
    ///
    /// # Errors
    /// Returns the `Present` HRESULT when it is a failure (for example device removed).
    pub fn present(&mut self) -> Result<()> {
        // SAFETY: `swap_chain` is a live swap chain; the flags value 0 is valid.
        unsafe { self.swap_chain.Present(self.sync, DXGI_PRESENT(0)).ok() }
    }
}
