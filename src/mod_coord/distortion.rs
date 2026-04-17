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

// Implementation lands here in v0.2 work.

/// Stub. Replaced in v0.2 by the `Dist_Poly3` port (mod-coord.cpp:615-632).
pub fn dist_poly3(_x: f32, _y: f32, _k1: f32) -> (f32, f32) {
    unimplemented!("dist_poly3 — pending v0.2 work")
}

/// Stub. Replaced in v0.2 by the `UnDist_Poly3` port (mod-coord.cpp:560-613).
pub fn undist_poly3(_x: f32, _y: f32, _k1: f32) -> (f32, f32) {
    unimplemented!("undist_poly3 — pending v0.2 work")
}

/// Stub. Replaced in v0.2 by the `Dist_Poly5` port (mod-coord.cpp:675-692).
pub fn dist_poly5(_x: f32, _y: f32, _k1: f32, _k2: f32) -> (f32, f32) {
    unimplemented!("dist_poly5 — pending v0.2 work")
}

/// Stub. Replaced in v0.2 by the `UnDist_Poly5` port (mod-coord.cpp:634-673).
pub fn undist_poly5(_x: f32, _y: f32, _k1: f32, _k2: f32) -> (f32, f32) {
    unimplemented!("undist_poly5 — pending v0.2 work")
}

/// Stub. Replaced in v0.2 by the `Dist_PTLens` port (mod-coord.cpp:736-757).
pub fn dist_ptlens(_x: f32, _y: f32, _a: f32, _b: f32, _c: f32) -> (f32, f32) {
    unimplemented!("dist_ptlens — pending v0.2 work")
}

/// Stub. Replaced in v0.2 by the `UnDist_PTLens` port (mod-coord.cpp:694-734).
pub fn undist_ptlens(_x: f32, _y: f32, _a: f32, _b: f32, _c: f32) -> (f32, f32) {
    unimplemented!("undist_ptlens — pending v0.2 work")
}
