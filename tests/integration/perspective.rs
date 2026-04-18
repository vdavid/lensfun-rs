//! Port of `tests/test_modifier_coord_perspective_correction.cpp`.
//!
//! Six scenarios from upstream — 4-point landscape, 4-point portrait, 8-point,
//! 5-point ellipse, 7-point ellipse + horizontal, and the "no points" rejection.
//! All use a freshly-constructed lens with no calibration (rectilinear default)
//! so only the perspective callback runs.
//!
//! Float tolerance: matches upstream (`5e3 * f32::EPSILON ≈ 6e-4`, `2e3 *
//! f32::EPSILON ≈ 2.4e-4`).

#![allow(clippy::excessive_precision)]

use approx::assert_abs_diff_eq;
use lensfun::Lens;
use lensfun::lens::LensType;
use lensfun::mod_pc::{
    Direction, SvdNoConvergence, apply_correction_kernel, apply_distortion_kernel,
    build_perspective_state, svd,
};
use lensfun::modifier::Modifier;

const FOCAL: f32 = 50.89;
const CROP: f32 = 1.534;
const TOL_5K: f32 = f32::EPSILON * 5e3;
const TOL_2K: f32 = f32::EPSILON * 2e3;

fn rectilinear_lens() -> Lens {
    // Match upstream `lfLens()` constructor defaults (lens.cpp:22-45).
    Lens {
        lens_type: LensType::Rectilinear,
        crop_factor: 1.0,
        aspect_ratio: 1.5,
        ..Lens::default()
    }
}

fn modifier_landscape() -> Modifier {
    Modifier::new(&rectilinear_lens(), FOCAL, CROP, 1500, 1000, false)
}

fn modifier_portrait() -> Modifier {
    Modifier::new(&rectilinear_lens(), FOCAL, CROP, 1000, 1500, false)
}

// ---------------------- SVD ----------------------

/// Port of `test_mod_coord_pc_svd` (cpp:70-98).
#[test]
fn svd_matches_upstream() {
    let x = [1.0_f64, 2.0, 3.0, 2.0, 1.0];
    let y = [1.0_f64, 2.0, 2.0, 0.0, 1.5];
    let mut m = Vec::with_capacity(5);
    for i in 0..5 {
        m.push(vec![x[i] * x[i], x[i] * y[i], y[i] * y[i], x[i], y[i], 1.0]);
    }
    let result = svd(m).expect("converges");
    let eps = f64::EPSILON * 5.0;
    let expected = [
        0.04756514941544937,
        0.09513029883089875,
        0.1902605976617977,
        -0.4280863447390447,
        -0.5707817929853928,
        0.6659120918162917,
    ];
    for i in 0..6 {
        assert!(
            (result[i] - expected[i]).abs() <= eps,
            "i={i}: got {}, want {}",
            result[i],
            expected[i],
        );
    }
}

// ---------------------- 4 control points ----------------------

/// Port of `test_mod_coord_pc_4_points` (cpp:100-121).
#[test]
fn pc_4_points_landscape() {
    let mut m = modifier_landscape();
    let x = [503.0_f32, 1063.0, 509.0, 1066.0];
    let y = [150.0_f32, 197.0, 860.0, 759.0];
    assert!(m.enable_perspective_correction(&x, &y, 0.0));

    let expected_x = [
        194.93747_f32,
        283.76633,
        367.27982,
        445.94156,
        520.16211,
        590.3075,
        656.70416,
        719.64496,
        779.39276,
        836.18463,
    ];
    let expected_y = [
        -88.539207_f32,
        45.526031,
        171.56934,
        290.28986,
        402.30765,
        508.1748,
        608.38434,
        703.37805,
        793.55273,
        879.26605,
    ];
    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 * i as f32, 100.0 * i as f32, 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], expected_x[i], epsilon = TOL_5K);
        assert_abs_diff_eq!(coords[1], expected_y[i], epsilon = TOL_5K);
    }
}

/// Port of `test_mod_coord_pc_4_points_portrait` (cpp:123-144).
#[test]
fn pc_4_points_portrait() {
    let mut m = modifier_portrait();
    let x = [145.0_f32, 208.0, 748.0, 850.0];
    let y = [1060.0_f32, 666.0, 668.0, 1060.0];
    assert!(m.enable_perspective_correction(&x, &y, 0.0));

    let expected_x = [
        71.087723_f32,
        147.83899,
        228.25151,
        312.59366,
        401.16068,
        494.27817,
        592.30609,
        695.64337,
        804.73334,
        920.07019,
    ];
    let expected_y = [
        508.74167_f32,
        536.03046,
        564.62103,
        594.60876,
        626.09875,
        659.20654,
        694.06024,
        730.80176,
        769.58856,
        810.5965,
    ];
    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 * i as f32, 100.0 * i as f32, 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], expected_x[i], epsilon = TOL_2K);
        assert_abs_diff_eq!(coords[1], expected_y[i], epsilon = TOL_2K);
    }
}

// ---------------------- 8 control points ----------------------

/// Port of `test_mod_coord_pc_8_points` (cpp:146-167).
#[test]
fn pc_8_points() {
    let mut m = modifier_landscape();
    let x = [615.0_f32, 264.0, 1280.0, 813.0, 615.0, 1280.0, 264.0, 813.0];
    let y = [755.0_f32, 292.0, 622.0, 220.0, 755.0, 622.0, 292.0, 220.0];
    assert!(m.enable_perspective_correction(&x, &y, 0.0));

    let expected_x = [
        -111.952522_f32,
        7.50310612,
        129.942596,
        255.479279,
        384.231903,
        516.325867,
        651.893066,
        791.071838,
        934.008728,
        1080.85803,
    ];
    let expected_y = [
        395.10733_f32,
        422.476837,
        450.529816,
        479.292572,
        508.792175,
        539.057251,
        570.118042,
        602.006531,
        634.755859,
        668.401611,
    ];
    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 * i as f32, 100.0 * i as f32, 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], expected_x[i], epsilon = TOL_5K);
        assert_abs_diff_eq!(coords[1], expected_y[i], epsilon = TOL_5K);
    }
}

// ---------------------- 0 / out-of-range points ----------------------

/// Port of `test_mod_coord_pc_0_points` (cpp:169-181).
#[test]
fn pc_zero_points_rejected() {
    let mut m = modifier_landscape();
    assert!(!m.enable_perspective_correction(&[], &[], 0.0));

    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(!m.apply_geometry_distortion(
            100.0 * i as f32,
            100.0 * i as f32,
            1,
            1,
            &mut coords
        ));
    }
}

// ---------------------- 5 control points (ellipse) ----------------------

/// Port of `test_mod_coord_pc_5_points` (cpp:183-204).
#[test]
fn pc_5_points_ellipse() {
    let mut m = modifier_landscape();
    let x = [661.0_f32, 594.0, 461.0, 426.0, 530.0];
    let y = [501.0_f32, 440.0, 442.0, 534.0, 562.0];
    assert!(m.enable_perspective_correction(&x, &y, 0.0));

    let expected_x = [
        -115.54961_f32,
        22.151754,
        151.93915,
        274.47577,
        390.35281,
        500.09869,
        604.18756,
        703.04572,
        797.05792,
        886.5719,
    ];
    let expected_y = [
        209.91034_f32,
        274.73886,
        335.84152,
        393.53049,
        448.0842,
        499.75153,
        548.75549,
        595.297,
        639.55695,
        681.69928,
    ];
    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 * i as f32, 100.0 * i as f32, 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], expected_x[i], epsilon = TOL_5K);
        assert_abs_diff_eq!(coords[1], expected_y[i], epsilon = TOL_5K);
    }
}

// ---------------------- 7 control points (ellipse + horizontal) --------

/// Port of `test_mod_coord_pc_7_points` (cpp:206-227).
#[test]
fn pc_7_points() {
    let mut m = modifier_landscape();
    let x = [661.0_f32, 594.0, 461.0, 426.0, 530.0, 302.0, 815.0];
    let y = [501.0_f32, 440.0, 442.0, 534.0, 562.0, 491.0, 279.0];
    assert!(m.enable_perspective_correction(&x, &y, 0.0));

    let expected_x = [
        -138.18913_f32,
        3.8870707,
        144.48228,
        283.6199,
        421.32199,
        557.61121,
        692.50861,
        826.03589,
        958.21338,
        1089.0619,
    ];
    let expected_y = [
        522.41956_f32,
        532.48621,
        542.44806,
        552.30658,
        562.06348,
        571.72015,
        581.27826,
        590.73932,
        600.10474,
        609.37598,
    ];
    for i in 0..10 {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 * i as f32, 100.0 * i as f32, 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], expected_x[i], epsilon = TOL_5K);
        assert_abs_diff_eq!(coords[1], expected_y[i], epsilon = TOL_5K);
    }
}

// ---------------------- round-trip property ----------------------

/// Build forward + inverse perspective states from the same control points
/// (skipping the `Modifier` pixel→normalized conversion), then check that
/// running them in sequence returns the input within float tolerance.
#[test]
fn correction_then_distortion_is_near_identity() {
    // Hand-picked control points already in normalized-ish space, with a
    // small slant — enough for the SVD to be non-trivial.
    let xn = vec![-0.30_f64, 0.30, -0.28, 0.32];
    let yn = vec![-0.20_f64, -0.18, 0.20, 0.18];

    let fwd = build_perspective_state(&xn, &yn, 0.0, false).expect("forward");
    let rev = build_perspective_state(&xn, &yn, 0.0, true).expect("reverse");
    assert_eq!(fwd.direction, Direction::Correction);
    assert_eq!(rev.direction, Direction::Distortion);

    // A few sample normalized coords near the image center.
    let samples = [[-0.1_f32, 0.05], [0.0, 0.0], [0.2, -0.1], [-0.05, 0.15]];
    for s in &samples {
        let mut buf = [s[0], s[1]];
        apply_correction_kernel(&fwd, &mut buf);
        apply_distortion_kernel(&rev, &mut buf);
        assert_abs_diff_eq!(buf[0], s[0], epsilon = 1e-3);
        assert_abs_diff_eq!(buf[1], s[1], epsilon = 1e-3);
    }
}

// ---------------------- error type sanity ----------------------

#[test]
fn svd_no_convergence_displays() {
    let e = SvdNoConvergence;
    let s = format!("{e}");
    assert!(s.contains("SVD"));
}
