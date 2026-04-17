//! Sub-pixel pass: transverse chromatic aberration (TCA) correction.
//!
//! Port of `libs/lensfun/mod-subpix.cpp`. Per-channel distortion: red and blue
//! planes get independent radial corrections relative to green (which is the
//! reference and stays put).
//!
//! The kernels are pure per-pixel functions: input is a normalized coordinate
//! `(x, y)` in lens-relative space (already centered, scaled by the unit-circle
//! norm). Output is `(x_red, y_red, x_blue, y_blue)`. Buffer iteration and
//! pixel/normalized conversion live in [`crate::modifier`].
//!
//! # Models
//!
//! - **Linear**: `Rd = k * Ru` per channel. Pure radial scale. Reverse is just
//!   `1/k`, which the caller (modifier) handles by inverting the term before
//!   passing it in. So [`tca_linear`] serves both forward and reverse.
//! - **Poly3 forward**: `Rd = Ru * (b·Ru² + c·Ru + v)` per channel. Closed form.
//!   Optimized path when `c == 0` (skips a square root per pixel).
//! - **Poly3 reverse**: solve `b·Ru³ + c·Ru² + v·Ru - Rd = 0` per channel by
//!   Newton iteration (≤6 steps). On non-convergence or negative root, the
//!   channel coordinate is left unchanged.
//!
//! # Float discipline
//!
//! Mirrors upstream exactly. Linear uses `f32` end-to-end. Poly3 forward stays
//! in `f32`. Poly3 reverse runs the Newton loop in `f64` (upstream `double`).
//! Don't refactor the algebra — bit-exact match against upstream tests matters.
//!
//! ACM (Adobe camera model) is intentionally not yet ported here; v0.3 covers
//! the linear and poly3 paths, which together account for every TCA calibration
//! in the bundled XML database.

/// Newton-iteration epsilon. Matches `NEWTON_EPS` in `lensfunprv.h:21`.
const NEWTON_EPS: f64 = 0.00001;

/// Linear TCA per pixel.
///
/// Red and blue planes scale radially relative to green: `red = (x·kr, y·kr)`,
/// `blue = (x·kb, y·kb)`. Green stays at `(x, y)` and is not returned.
///
/// Used for both forward and reverse: for reverse, callers pass `1/kr` and
/// `1/kb` (mirroring upstream's `rescale_polynomial_coefficients`).
///
/// Returns `(x_red, y_red, x_blue, y_blue)`.
// Port of mod-subpix.cpp:199-215.
pub fn tca_linear(x: f32, y: f32, kr: f32, kb: f32) -> (f32, f32, f32, f32) {
    (x * kr, y * kr, x * kb, y * kb)
}

/// Forward poly3 TCA per pixel.
///
/// `Rd = Ru * (b·Ru² + c·Ru + v)` per channel. Coefficients are passed as
/// `[v, c, b]` for each channel, mirroring upstream's `Terms` layout
/// `[vr, vb, cr, cb, br, bb]` once you ungroup it per channel.
///
/// Returns `(x_red, y_red, x_blue, y_blue)`.
// Port of mod-subpix.cpp:296-343.
pub fn tca_poly3_forward(
    x: f32,
    y: f32,
    red_coeffs: [f32; 3],
    blue_coeffs: [f32; 3],
) -> (f32, f32, f32, f32) {
    let [vr, cr, br] = red_coeffs;
    let [vb, cb, bb] = blue_coeffs;

    // Optimize for the case when c == 0 (avoid one square root per channel).
    // Upstream gates the optimization on BOTH cr and cb being zero, so we do
    // the same — the per-pixel branch must be identical for red and blue.
    if cr == 0.0 && cb == 0.0 {
        let ru2_r = x * x + y * y;
        let poly2_r = br * ru2_r + vr;
        let xr = x * poly2_r;
        let yr = y * poly2_r;

        let ru2_b = x * x + y * y;
        let poly2_b = bb * ru2_b + vb;
        let xb = x * poly2_b;
        let yb = y * poly2_b;

        (xr, yr, xb, yb)
    } else {
        let ru2_r = x * x + y * y;
        let poly2_r = br * ru2_r + cr * ru2_r.sqrt() + vr;
        let xr = x * poly2_r;
        let yr = y * poly2_r;

        let ru2_b = x * x + y * y;
        let poly2_b = bb * ru2_b + cb * ru2_b.sqrt() + vb;
        let xb = x * poly2_b;
        let yb = y * poly2_b;

        (xr, yr, xb, yb)
    }
}

/// Reverse poly3 TCA per pixel.
///
/// Solve `b·Ru³ + c·Ru² + v·Ru - Rd = 0` per channel by Newton iteration
/// (≤6 steps). On non-convergence or negative root, the channel coordinate is
/// left unchanged.
///
/// Coefficients are `[v, c, b]` per channel. Returns `(x_red, y_red, x_blue,
/// y_blue)`.
// Port of mod-subpix.cpp:217-294.
pub fn tca_poly3_reverse(
    x: f32,
    y: f32,
    red_coeffs: [f32; 3],
    blue_coeffs: [f32; 3],
) -> (f32, f32, f32, f32) {
    let [vr, cr, br] = red_coeffs;
    let [vb, cb, bb] = blue_coeffs;

    let (xr, yr) = invert_one_channel(x, y, vr, cr, br);
    let (xb, yb) = invert_one_channel(x, y, vb, cb, bb);

    (xr, yr, xb, yb)
}

/// Single-channel poly3 inversion. Mirrors the per-channel block in
/// `ModifyCoord_UnTCA_Poly3` (mod-subpix.cpp:230-263, 266-291) — same formula
/// runs twice in upstream, once for red and once for blue.
fn invert_one_channel(x: f32, y: f32, v: f32, c: f32, b: f32) -> (f32, f32) {
    let v_ = v as f64;
    let c_ = c as f64;
    let b_ = b as f64;

    let rd = ((x * x + y * y) as f64).sqrt();
    if rd == 0.0 {
        return (x, y);
    }

    let mut ru = rd;
    let mut step = 0;
    let converged = loop {
        let ru2 = ru * ru;
        let fru = b_ * ru2 * ru + c_ * ru2 + v_ * ru - rd;
        if (-NEWTON_EPS..NEWTON_EPS).contains(&fru) {
            break true;
        }
        if step > 5 {
            // Does not converge — leave input unchanged.
            break false;
        }
        ru -= fru / (3.0 * b_ * ru2 + 2.0 * c_ * ru + v_);
        step += 1;
    };

    // Negative radius does not make sense at all (upstream comment).
    if !converged || ru <= 0.0 {
        return (x, y);
    }

    let scale = (ru / rd) as f32;
    (x * scale, y * scale)
}
