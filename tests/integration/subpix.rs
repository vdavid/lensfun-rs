//! Port of upstream `tests/test_modifier_subpix.cpp`.
//!
//! Upstream's test is a smoke test on the high-level `lfModifier` API: it
//! constructs a lens with a TCA calibration, calls `ApplySubpixelDistortion`
//! row-by-row over a 300×300 buffer, and asserts the call returns `true`. It
//! doesn't check pixel values. Once we wire `Modifier::apply_subpixel_*`, that
//! shape ports cleanly — but for v0.3 it's blocked.
//!
//! Until then, we exercise the kernels directly with the same calibration
//! coefficients upstream uses for its smoke tests:
//!
//! - **Linear**: `kr = kb = 1.0003` (Nikon AF-S DX Nikkor 35mm f/1.8G)
//! - **Poly3**:  `[1.0002104, 1.0000529, 0, 0, -0.0000220, 0]`
//!   (Canon EOS 5D Mark III + Canon EF 24-70mm f/2.8L II USM)
//!
//! These match the calibration data in upstream test fixtures verbatim
//! (test_modifier_subpix.cpp:160-168).

use approx::assert_relative_eq;
use lensfun::mod_subpix::{tca_linear, tca_poly3_forward, tca_poly3_reverse};
use proptest::prelude::*;

// Upstream calibration constants — see file header.
const NIKON_KR: f32 = 1.0003;
const NIKON_KB: f32 = 1.0003;

// Upstream Terms layout for poly3 is `[vr, vb, cr, cb, br, bb]`. We pass
// per-channel as `[v, c, b]`.
const CANON_RED: [f32; 3] = [1.0002104, 0.0, -0.0000220];
const CANON_BLUE: [f32; 3] = [1.0000529, 0.0, -0.0000000];

// -----------------------------// Linear //-----------------------------//

#[test]
fn linear_origin_is_fixed_point() {
    // `(0, 0)` is the optical center; all channels collapse there. Mirrors
    // upstream's implicit handling — the multiply just yields zero.
    let (xr, yr, xb, yb) = tca_linear(0.0, 0.0, NIKON_KR, NIKON_KB);
    assert_eq!((xr, yr, xb, yb), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn linear_scales_each_channel_by_its_k() {
    let (xr, yr, xb, yb) = tca_linear(1.0, 0.5, 1.001, 0.999);
    assert_eq!(xr, 1.001);
    assert_eq!(yr, 0.5005);
    assert_eq!(xb, 0.999);
    assert_eq!(yb, 0.4995);
}

#[test]
fn linear_identity_when_k_is_one() {
    let (xr, yr, xb, yb) = tca_linear(0.7, -0.3, 1.0, 1.0);
    assert_eq!((xr, yr, xb, yb), (0.7, -0.3, 0.7, -0.3));
}

#[test]
fn linear_reverse_via_inverted_k_round_trips() {
    // Upstream achieves linear reverse by passing 1/k as the term. Verify
    // round-trip is well-behaved for typical calibration values.
    let (xr, yr, xb, yb) = tca_linear(2.0, 3.0, NIKON_KR, NIKON_KB);
    let (xr2, yr2, xb2, yb2) = tca_linear(xr, yr, 1.0 / NIKON_KR, 1.0 / NIKON_KR);
    assert_relative_eq!(xr2, 2.0, max_relative = 1e-6);
    assert_relative_eq!(yr2, 3.0, max_relative = 1e-6);
    let _ = (xb, yb, xb2, yb2);
}

// -----------------------------// Poly3 forward //-----------------------------//

#[test]
fn poly3_forward_origin_is_fixed_point() {
    let (xr, yr, xb, yb) = tca_poly3_forward(0.0, 0.0, CANON_RED, CANON_BLUE);
    assert_eq!((xr, yr, xb, yb), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn poly3_forward_matches_closed_form_at_unit_radius() {
    // At (1, 0): ru² = 1, sqrt(ru²) = 1, so per-channel scale is `b + c + v`.
    let (xr, yr, xb, yb) =
        tca_poly3_forward(1.0, 0.0, [1.0, 0.001, -0.0001], [1.0, -0.001, 0.0001]);
    assert_relative_eq!(xr, 1.0 + 0.001 - 0.0001, max_relative = 1e-6);
    assert_eq!(yr, 0.0);
    assert_relative_eq!(xb, 1.0 - 0.001 + 0.0001, max_relative = 1e-6);
    assert_eq!(yb, 0.0);
}

#[test]
fn poly3_forward_takes_optimized_path_when_c_is_zero() {
    // Both c values are zero (Canon calibration above). The kernel picks the
    // optimized branch that skips the sqrt. Result should be `b·ru² + v` scale.
    let x = 0.5_f32;
    let y = 0.25_f32;
    let ru2 = x * x + y * y;

    let (xr, yr, xb, yb) = tca_poly3_forward(x, y, CANON_RED, CANON_BLUE);

    let expected_red_scale = CANON_RED[2] * ru2 + CANON_RED[0];
    let expected_blue_scale = CANON_BLUE[2] * ru2 + CANON_BLUE[0];

    assert_eq!(xr, x * expected_red_scale);
    assert_eq!(yr, y * expected_red_scale);
    assert_eq!(xb, x * expected_blue_scale);
    assert_eq!(yb, y * expected_blue_scale);
}

#[test]
fn poly3_forward_takes_general_path_when_c_is_nonzero() {
    // When either cr or cb is nonzero, upstream takes the general branch with
    // an extra sqrt per channel. Verify the formula end-to-end.
    let x = 0.5_f32;
    let y = 0.25_f32;
    let ru2 = x * x + y * y;
    let r = ru2.sqrt();

    let red = [1.0002, 0.0001, -0.00002];
    let blue = [1.0001, 0.0, -0.00001];

    let (xr, yr, xb, yb) = tca_poly3_forward(x, y, red, blue);

    let expected_red_scale = red[2] * ru2 + red[1] * r + red[0];
    let expected_blue_scale = blue[2] * ru2 + blue[1] * r + blue[0];

    assert_eq!(xr, x * expected_red_scale);
    assert_eq!(yr, y * expected_red_scale);
    assert_eq!(xb, x * expected_blue_scale);
    assert_eq!(yb, y * expected_blue_scale);
}

// -----------------------------// Poly3 reverse //-----------------------------//

#[test]
fn poly3_reverse_origin_is_fixed_point() {
    let (xr, yr, xb, yb) = tca_poly3_reverse(0.0, 0.0, CANON_RED, CANON_BLUE);
    assert_eq!((xr, yr, xb, yb), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn poly3_reverse_round_trips_typical_calibration() {
    // Forward then reverse should recover the input within Newton tolerance.
    let x = 0.4_f32;
    let y = 0.2_f32;

    let (xr, yr, xb, yb) = tca_poly3_forward(x, y, CANON_RED, CANON_BLUE);
    let (xr2, yr2, _, _) = tca_poly3_reverse(xr, yr, CANON_RED, CANON_RED);
    let (_, _, xb2, yb2) = tca_poly3_reverse(xb, yb, CANON_BLUE, CANON_BLUE);

    assert_relative_eq!(xr2, x, max_relative = 1e-4);
    assert_relative_eq!(yr2, y, max_relative = 1e-4);
    assert_relative_eq!(xb2, x, max_relative = 1e-4);
    assert_relative_eq!(yb2, y, max_relative = 1e-4);
}

#[test]
fn poly3_reverse_with_identity_coeffs_is_identity() {
    // v=1, c=0, b=0 means Rd = Ru. Newton converges in one step.
    let (xr, yr, xb, yb) = tca_poly3_reverse(0.7, -0.3, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    assert_relative_eq!(xr, 0.7, max_relative = 1e-6);
    assert_relative_eq!(yr, -0.3, max_relative = 1e-6);
    assert_relative_eq!(xb, 0.7, max_relative = 1e-6);
    assert_relative_eq!(yb, -0.3, max_relative = 1e-6);
}

// -----------------------------// Channel independence //-----------------------------//

proptest! {
    /// Changing red coefficients must not affect the blue output, and vice
    /// versa. This is a structural property: red and blue are processed
    /// independently by upstream.
    #[test]
    fn poly3_forward_channels_independent(
        x in -1.0_f32..1.0,
        y in -1.0_f32..1.0,
        vr1 in 0.99_f32..1.01, cr1 in -0.001_f32..0.001, br1 in -0.001_f32..0.001,
        vr2 in 0.99_f32..1.01, cr2 in -0.001_f32..0.001, br2 in -0.001_f32..0.001,
        vb in 0.99_f32..1.01, cb in -0.001_f32..0.001, bb in -0.001_f32..0.001,
    ) {
        let (_, _, xb1, yb1) = tca_poly3_forward(x, y, [vr1, cr1, br1], [vb, cb, bb]);
        let (_, _, xb2, yb2) = tca_poly3_forward(x, y, [vr2, cr2, br2], [vb, cb, bb]);
        prop_assert_eq!(xb1, xb2);
        prop_assert_eq!(yb1, yb2);
    }

    #[test]
    fn poly3_reverse_channels_independent(
        x in -1.0_f32..1.0,
        y in -1.0_f32..1.0,
        vb1 in 0.99_f32..1.01, cb1 in -0.001_f32..0.001, bb1 in -0.001_f32..0.001,
        vb2 in 0.99_f32..1.01, cb2 in -0.001_f32..0.001, bb2 in -0.001_f32..0.001,
        vr in 0.99_f32..1.01, cr in -0.001_f32..0.001, br in -0.001_f32..0.001,
    ) {
        let (xr1, yr1, _, _) = tca_poly3_reverse(x, y, [vr, cr, br], [vb1, cb1, bb1]);
        let (xr2, yr2, _, _) = tca_poly3_reverse(x, y, [vr, cr, br], [vb2, cb2, bb2]);
        prop_assert_eq!(xr1, xr2);
        prop_assert_eq!(yr1, yr2);
    }

    #[test]
    fn linear_channels_independent(
        x in -1.0_f32..1.0,
        y in -1.0_f32..1.0,
        kr1 in 0.99_f32..1.01,
        kr2 in 0.99_f32..1.01,
        kb in 0.99_f32..1.01,
    ) {
        let (_, _, xb1, yb1) = tca_linear(x, y, kr1, kb);
        let (_, _, xb2, yb2) = tca_linear(x, y, kr2, kb);
        prop_assert_eq!(xb1, xb2);
        prop_assert_eq!(yb1, yb2);
    }
}

// -----------------------------// High-level API (blocked) //-----------------------------//

/// Direct port of `test_mod_subpix` (test_modifier_subpix.cpp:74-85): build a
/// 300×300 modifier, call `ApplySubpixelDistortion` row-by-row, assert each
/// returns true. Blocked until `Modifier::apply_subpixel_distortion` exists.
#[test]
#[ignore = "blocked on Modifier::apply_subpixel_distortion wiring"]
fn upstream_smoke_test_apply_subpixel_distortion() {
    // When wired:
    //   let lens = Lens::new_rectilinear_with_tca(...);
    //   let modifier = Modifier::for_lens(&lens, 24.0, 1.0, 300, 300, false);
    //   for y in 0..300 { assert!(modifier.apply_subpixel_distortion(0.0, y, 300, 1, &mut buf)); }
}
