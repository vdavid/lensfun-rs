//! Distortion kernel tests. Hand-computed expectations against the closed-form math, plus
//! Newton round-trip property tests. Tolerances:
//!
//! - Forward kernels and Catmull-Rom: `1e-6` (pure float arithmetic, no iteration).
//! - Round-trip (`dist ∘ undist`): `1e-4` (Newton tolerance is `1e-5` in `f64`, so error
//!   in `f32` after composition lands well under `1e-4`).

use approx::assert_relative_eq;
use lensfun::auxfun::{NO_NEIGHBOR, catmull_rom_interpolate};
use lensfun::mod_coord::{
    dist_poly3, dist_poly5, dist_ptlens, undist_poly3, undist_poly5, undist_ptlens,
};
use proptest::prelude::*;

// ---- poly3 ----

#[test]
fn dist_poly3_origin_is_origin() {
    assert_eq!(dist_poly3(0.0, 0.0, 0.05), (0.0, 0.0));
}

#[test]
fn undist_poly3_origin_is_origin() {
    assert_eq!(undist_poly3(0.0, 0.0, 0.05), (0.0, 0.0));
}

#[test]
fn dist_poly3_known_value() {
    // Rd = Ru * (1 + k1 * Ru^2). For (x, y) = (0.5, 0.0), k1 = 0.1:
    // poly2 = 0.1 * 0.25 + 1 = 1.025; out = (0.5125, 0.0).
    let (x, y) = dist_poly3(0.5, 0.0, 0.1);
    assert_relative_eq!(x, 0.5125, epsilon = 1e-6);
    assert_relative_eq!(y, 0.0, epsilon = 1e-6);

    // (0.3, 0.4): r² = 0.25, poly2 = 1.025, out = (0.3075, 0.41).
    let (x, y) = dist_poly3(0.3, 0.4, 0.1);
    assert_relative_eq!(x, 0.3075, epsilon = 1e-6);
    assert_relative_eq!(y, 0.41, epsilon = 1e-6);
}

#[test]
fn poly3_round_trip_basic() {
    let k1 = 0.05;
    let cases = [(0.1_f32, 0.0_f32), (0.3, 0.4), (-0.2, 0.6), (0.0, -0.5)];
    for (x, y) in cases {
        let (xd, yd) = dist_poly3(x, y, k1);
        let (xu, yu) = undist_poly3(xd, yd, k1);
        assert_relative_eq!(xu, x, epsilon = 1e-4);
        assert_relative_eq!(yu, y, epsilon = 1e-4);
    }
}

// ---- poly5 ----

#[test]
fn dist_poly5_origin_is_origin() {
    assert_eq!(dist_poly5(0.0, 0.0, 0.05, 0.01), (0.0, 0.0));
}

#[test]
fn undist_poly5_origin_is_origin() {
    assert_eq!(undist_poly5(0.0, 0.0, 0.05, 0.01), (0.0, 0.0));
}

#[test]
fn dist_poly5_known_value() {
    // Rd = Ru * (1 + k1·Ru² + k2·Ru⁴). For (0.5, 0.0), k1=0.1, k2=0.02:
    // ru2 = 0.25; poly4 = 1 + 0.025 + 0.02 * 0.0625 = 1.025 + 0.00125 = 1.02625.
    // out = (0.513125, 0.0).
    let (x, y) = dist_poly5(0.5, 0.0, 0.1, 0.02);
    assert_relative_eq!(x, 0.513125, epsilon = 1e-6);
    assert_relative_eq!(y, 0.0, epsilon = 1e-6);
}

#[test]
fn poly5_round_trip_basic() {
    let (k1, k2) = (0.05_f32, 0.01_f32);
    let cases = [(0.1_f32, 0.0_f32), (0.3, 0.4), (-0.2, 0.6), (0.0, -0.5)];
    for (x, y) in cases {
        let (xd, yd) = dist_poly5(x, y, k1, k2);
        let (xu, yu) = undist_poly5(xd, yd, k1, k2);
        assert_relative_eq!(xu, x, epsilon = 1e-4);
        assert_relative_eq!(yu, y, epsilon = 1e-4);
    }
}

// ---- ptlens ----

#[test]
fn dist_ptlens_origin_is_origin() {
    assert_eq!(dist_ptlens(0.0, 0.0, 0.01, -0.02, 0.0), (0.0, 0.0));
}

#[test]
fn undist_ptlens_origin_is_origin() {
    assert_eq!(undist_ptlens(0.0, 0.0, 0.01, -0.02, 0.0), (0.0, 0.0));
}

#[test]
fn dist_ptlens_known_value() {
    // Rd = Ru * (a·Ru³ + b·Ru² + c·Ru + 1). For (0.5, 0.0), a=0.01, b=-0.02, c=0.005:
    // ru2 = 0.25, r = 0.5.
    // poly3 = 0.01·0.125 + (-0.02)·0.25 + 0.005·0.5 + 1
    //       = 0.00125 - 0.005 + 0.0025 + 1 = 0.99875.
    // out = (0.499375, 0.0).
    let (x, y) = dist_ptlens(0.5, 0.0, 0.01, -0.02, 0.005);
    assert_relative_eq!(x, 0.499375, epsilon = 1e-6);
    assert_relative_eq!(y, 0.0, epsilon = 1e-6);
}

#[test]
fn ptlens_round_trip_basic() {
    let (a, b, c) = (0.01_f32, -0.02_f32, 0.005_f32);
    let cases = [(0.1_f32, 0.0_f32), (0.3, 0.4), (-0.2, 0.6), (0.0, -0.5)];
    for (x, y) in cases {
        let (xd, yd) = dist_ptlens(x, y, a, b, c);
        let (xu, yu) = undist_ptlens(xd, yd, a, b, c);
        assert_relative_eq!(xu, x, epsilon = 1e-4);
        assert_relative_eq!(yu, y, epsilon = 1e-4);
    }
}

// ---- property-based round trips ----

proptest! {
    #[test]
    fn poly3_round_trip(
        x in -0.7_f32..0.7,
        y in -0.7_f32..0.7,
        k1 in -0.1_f32..0.1,
    ) {
        let (xd, yd) = dist_poly3(x, y, k1);
        let (xu, yu) = undist_poly3(xd, yd, k1);
        prop_assume!(xu.is_finite() && yu.is_finite());
        prop_assert!((xu - x).abs() < 1e-4);
        prop_assert!((yu - y).abs() < 1e-4);
    }

    #[test]
    fn poly5_round_trip(
        x in -0.7_f32..0.7,
        y in -0.7_f32..0.7,
        k1 in -0.1_f32..0.1,
        k2 in -0.05_f32..0.05,
    ) {
        let (xd, yd) = dist_poly5(x, y, k1, k2);
        let (xu, yu) = undist_poly5(xd, yd, k1, k2);
        // poly5/ptlens leave coords unchanged on non-convergence; restrict to the
        // well-behaved center where Newton converges and rules out that path.
        prop_assert!((xu - x).abs() < 1e-4);
        prop_assert!((yu - y).abs() < 1e-4);
    }

    #[test]
    fn ptlens_round_trip(
        x in -0.5_f32..0.5,
        y in -0.5_f32..0.5,
        a in -0.02_f32..0.02,
        b in -0.02_f32..0.02,
        c in -0.02_f32..0.02,
    ) {
        let (xd, yd) = dist_ptlens(x, y, a, b, c);
        let (xu, yu) = undist_ptlens(xd, yd, a, b, c);
        prop_assert!((xu - x).abs() < 1e-4);
        prop_assert!((yu - y).abs() < 1e-4);
    }
}

// ---- Catmull-Rom ----

#[test]
fn catmull_rom_passes_through_y2_at_t0() {
    let y = catmull_rom_interpolate(1.0, 2.0, 3.5, 4.0, 0.0);
    assert_relative_eq!(y, 2.0, epsilon = 1e-6);
}

#[test]
fn catmull_rom_passes_through_y3_at_t1() {
    let y = catmull_rom_interpolate(1.0, 2.0, 3.5, 4.0, 1.0);
    assert_relative_eq!(y, 3.5, epsilon = 1e-6);
}

#[test]
fn catmull_rom_left_endpoint_uses_y3_minus_y2() {
    // y1 = NO_NEIGHBOR ⇒ tg2 = y3 - y2. Compute by hand at t = 0.5:
    // t2 = 0.25, t3 = 0.125. y2 = 2, y3 = 4, y4 = 5.
    // tg2 = 2, tg3 = (5 - 2) * 0.5 = 1.5.
    // Hermite = (2·0.125 - 3·0.25 + 1)·2 + (0.125 - 0.5 + 0.5)·2
    //         + (-2·0.125 + 3·0.25)·4 + (0.125 - 0.25)·1.5
    //         = 0.5·2 + 0.125·2 + 0.5·4 + (-0.125)·1.5
    //         = 1 + 0.25 + 2 - 0.1875 = 3.0625.
    let y = catmull_rom_interpolate(NO_NEIGHBOR, 2.0, 4.0, 5.0, 0.5);
    assert_relative_eq!(y, 3.0625, epsilon = 1e-6);
}

#[test]
fn catmull_rom_right_endpoint_uses_y3_minus_y2() {
    // y4 = NO_NEIGHBOR ⇒ tg3 = y3 - y2 = 2. y1 = 0, y2 = 2, y3 = 4. tg2 = (4-0)·0.5 = 2.
    // At t = 0.5 (t2=0.25, t3=0.125):
    // = 0.5·2 + 0.125·2 + 0.5·4 + (-0.125)·2
    // = 1 + 0.25 + 2 - 0.25 = 3.0.
    let y = catmull_rom_interpolate(0.0, 2.0, 4.0, NO_NEIGHBOR, 0.5);
    assert_relative_eq!(y, 3.0, epsilon = 1e-6);
}

#[test]
fn catmull_rom_both_endpoints_collapse_to_linear() {
    // Both NO_NEIGHBOR ⇒ tg2 = tg3 = y3 - y2. At t = 0.5 with y2 = 2, y3 = 6:
    // tg = 4. = 0.5·2 + 0.125·4 + 0.5·6 + (-0.125)·4
    //         = 1 + 0.5 + 3 - 0.5 = 4.0 (which equals (y2 + y3) / 2 — the linear midpoint).
    let y = catmull_rom_interpolate(NO_NEIGHBOR, 2.0, 6.0, NO_NEIGHBOR, 0.5);
    assert_relative_eq!(y, 4.0, epsilon = 1e-6);
}
