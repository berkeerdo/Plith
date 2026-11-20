//! `Node::Inset`: a list drawn scaled inside another, such as a true-size pill preview.

use super::{ListEnv, ListKit, draw_list};
use crate::paint::f;
use plith_core::draw::{ArtId, DisplayList};
use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
use windows::core::Result;
use windows_numerics::Matrix3x2;

/// `a` then `b` (row-vector convention, as Direct2D): a point goes through `a` first.
fn then(a: &Matrix3x2, b: &Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: a.M11 * b.M11 + a.M12 * b.M21,
        M12: a.M11 * b.M12 + a.M12 * b.M22,
        M21: a.M21 * b.M11 + a.M22 * b.M21,
        M22: a.M21 * b.M12 + a.M22 * b.M22,
        M31: a.M31 * b.M11 + a.M32 * b.M21 + b.M31,
        M32: a.M31 * b.M12 + a.M32 * b.M22 + b.M32,
    }
}

/// Draws `list` with its origin at `(x, y)` scaled by `scale`, on top of the current transform,
/// and restores that transform. Returns the artwork that was referenced but missing.
///
/// # Errors
/// Returns the first failing call of the inner list.
pub(super) fn draw(
    ctx: &ID2D1DeviceContext,
    kit: &mut ListKit,
    env: &mut ListEnv<'_>,
    list: &DisplayList,
    (x, y, scale): (f64, f64, f64),
) -> Result<Vec<ArtId>> {
    let mut current = Matrix3x2::identity();
    // SAFETY: a state read inside the frame's BeginDraw/EndDraw into a live matrix.
    unsafe { ctx.GetTransform(&raw mut current) };
    let local = Matrix3x2 {
        M11: f(scale),
        M12: 0.0,
        M21: 0.0,
        M22: f(scale),
        M31: f(x),
        M32: f(y),
    };
    let composed = then(&local, &current);
    // SAFETY: plain state changes inside the frame's BeginDraw/EndDraw; restored below.
    unsafe { ctx.SetTransform(&raw const composed) };
    let missing = draw_list(ctx, kit, env, list);
    // SAFETY: as above.
    unsafe { ctx.SetTransform(&raw const current) };
    missing
}
