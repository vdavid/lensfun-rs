//! Distortion correction kernels (port of `mod-coord.cpp:560-758`).
//!
//! Six functions — forward (`Dist`) and inverse (`UnDist`) for each of the three models
//! supported by upstream: poly3, poly5, and PTLens. The forward kernels are closed-form
//! polynomials; the inverse kernels run a small Newton iteration (≤6 steps) because the
//! polynomial inverses don't have a clean closed form.
//!
//! These are pure per-pixel functions: input is a normalized coordinate `(x, y)` in
//! lens-relative space (already centered, scaled by the unit-circle norm), output is
//! the corrected coordinate. Buffer iteration and pixel/normalized conversion live in
//! the [`crate::modifier`] module.
//!
//! Float discipline: in/out are `f32` to match upstream `float`. Newton iteration runs in
//! `f64` to match upstream `double`. Forward kernels stay in `f32` end-to-end.
//!
//! Non-convergence: poly3 returns NaN (matches upstream). poly5 and ptlens leave the input
//! coordinate unchanged (matches upstream's `goto next_pixel` after `continue`-equivalent —
//! see mod-coord.cpp:670 and 731). Don't "fix" this asymmetry; it's real.

/// Newton-iteration epsilon. Matches `NEWTON_EPS` in `lensfunprv.h:21`.
const NEWTON_EPS: f64 = 0.00001;

/// Forward poly3: `Rd = Ru * (1 + k1 * Ru²)`.
// Port of mod-coord.cpp:615-632.
pub fn dist_poly3(x: f32, y: f32, k1: f32) -> (f32, f32) {
    let poly2 = k1 * (x * x + y * y) + 1.0;
    (x * poly2, y * poly2)
}

/// Inverse poly3: solve `k1·Ru³ + Ru - Rd = 0` by Newton iteration. Returns NaN on
/// non-convergence or negative root (matches upstream).
// Port of mod-coord.cpp:560-613.
pub fn undist_poly3(x: f32, y: f32, k1: f32) -> (f32, f32) {
    let inv_k1_ = 1.0_f32 / k1;

    let rd = ((x * x + y * y) as f64).sqrt();
    if rd == 0.0 {
        return (x, y);
    }

    // Upstream: `float rd_div_k1_ = rd * inv_k1_;` — double multiply, then truncated to float.
    let rd_div_k1_: f32 = (rd * inv_k1_ as f64) as f32;

    let mut ru = rd;
    let mut step = 0;
    loop {
        let fru = ru * ru * ru + ru * inv_k1_ as f64 - rd_div_k1_ as f64;
        if (-NEWTON_EPS..NEWTON_EPS).contains(&fru) {
            break;
        }
        if step > 5 {
            return (f32::NAN, f32::NAN);
        }
        ru -= fru / (3.0 * ru * ru + inv_k1_ as f64);
        step += 1;
    }
    if ru < 0.0 {
        return (f32::NAN, f32::NAN);
    }

    let scale = (ru / rd) as f32;
    (x * scale, y * scale)
}

/// Forward poly5: `Rd = Ru * (1 + k1·Ru² + k2·Ru⁴)`.
// Port of mod-coord.cpp:675-692.
pub fn dist_poly5(x: f32, y: f32, k1: f32, k2: f32) -> (f32, f32) {
    let ru2 = x * x + y * y;
    let poly4 = 1.0 + k1 * ru2 + k2 * ru2 * ru2;
    (x * poly4, y * poly4)
}

/// Inverse poly5: solve `Ru·(1 + k1·Ru² + k2·Ru⁴) - Rd = 0`. On non-convergence or
/// negative root, returns the input unchanged (matches upstream).
// Port of mod-coord.cpp:634-673.
pub fn undist_poly5(x: f32, y: f32, k1: f32, k2: f32) -> (f32, f32) {
    let rd = ((x * x + y * y) as f64).sqrt();
    if rd == 0.0 {
        return (x, y);
    }

    let mut ru = rd;
    let mut step = 0;
    let converged = loop {
        let ru2 = ru * ru;
        let fru = ru * (1.0 + k1 as f64 * ru2 + k2 as f64 * ru2 * ru2) - rd;
        if (-NEWTON_EPS..NEWTON_EPS).contains(&fru) {
            break true;
        }
        if step > 5 {
            // Does not converge — leave input unchanged.
            break false;
        }
        ru -= fru / (1.0 + 3.0 * k1 as f64 * ru2 + 5.0 * k2 as f64 * ru2 * ru2);
        step += 1;
    };
    if !converged || ru < 0.0 {
        return (x, y);
    }

    let scale = (ru / rd) as f32;
    (x * scale, y * scale)
}

/// Forward PTLens: `Rd = Ru * (a·Ru³ + b·Ru² + c·Ru + 1)`.
// Port of mod-coord.cpp:736-757.
pub fn dist_ptlens(x: f32, y: f32, a: f32, b: f32, c: f32) -> (f32, f32) {
    let ru2 = x * x + y * y;
    let r = ru2.sqrt();
    let poly3 = a * ru2 * r + b * ru2 + c * r + 1.0;
    (x * poly3, y * poly3)
}

/// Inverse PTLens: solve `Ru·(a·Ru³ + b·Ru² + c·Ru + 1) - Rd = 0`. On non-convergence or
/// negative root, returns the input unchanged (matches upstream).
// Port of mod-coord.cpp:694-734.
pub fn undist_ptlens(x: f32, y: f32, a: f32, b: f32, c: f32) -> (f32, f32) {
    let rd = ((x * x + y * y) as f64).sqrt();
    if rd == 0.0 {
        return (x, y);
    }

    let a_ = a as f64;
    let b_ = b as f64;
    let c_ = c as f64;

    let mut ru = rd;
    let mut step = 0;
    let converged = loop {
        let fru = ru * (a_ * ru * ru * ru + b_ * ru * ru + c_ * ru + 1.0) - rd;
        if (-NEWTON_EPS..NEWTON_EPS).contains(&fru) {
            break true;
        }
        if step > 5 {
            break false;
        }
        ru -= fru / (4.0 * a_ * ru * ru * ru + 3.0 * b_ * ru * ru + 2.0 * c_ * ru + 1.0);
        step += 1;
    };
    if !converged || ru < 0.0 {
        return (x, y);
    }

    let scale = (ru / rd) as f32;
    (x * scale, y * scale)
}
