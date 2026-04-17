//! Tests for the 4D calibration interpolation in `Lens`.
//!
//! These exercise `interpolate_distortion`, `interpolate_tca`, and
//! `interpolate_vignetting` — the Rust port of `lfLens::Interpolate*`
//! (`lens.cpp:910-1207`). Hand-built lenses with synthetic calibration grids
//! drive the cases; we don't depend on the bundled XML for these checks
//! because matching upstream exactly there would require a running upstream
//! to compare against.

use approx::assert_relative_eq;

use lensfun::Lens;
use lensfun::calib::{
    CalibDistortion, CalibTca, CalibVignetting, DistortionModel, TcaModel, VignettingModel,
};

// -----------------------------// helpers //-----------------------------//

fn distortion(focal: f32, k1: f32) -> CalibDistortion {
    CalibDistortion {
        focal,
        model: DistortionModel::Poly3 { k1 },
        real_focal: None,
    }
}

fn ptlens(focal: f32, a: f32, b: f32, c: f32) -> CalibDistortion {
    CalibDistortion {
        focal,
        model: DistortionModel::Ptlens { a, b, c },
        real_focal: None,
    }
}

fn tca_linear(focal: f32, kr: f32, kb: f32) -> CalibTca {
    CalibTca {
        focal,
        model: TcaModel::Linear { kr, kb },
    }
}

fn vignetting(focal: f32, aperture: f32, distance: f32, k1: f32) -> CalibVignetting {
    CalibVignetting {
        focal,
        aperture,
        distance,
        model: VignettingModel::Pa {
            k1,
            k2: 0.0,
            k3: 0.0,
        },
    }
}

fn lens_with_distortion(samples: Vec<CalibDistortion>) -> Lens {
    Lens {
        focal_min: 10.0,
        focal_max: 200.0,
        crop_factor: 1.0,
        aspect_ratio: 1.5,
        calib_distortion: samples,
        ..Lens::default()
    }
}

fn lens_with_tca(samples: Vec<CalibTca>) -> Lens {
    Lens {
        focal_min: 10.0,
        focal_max: 200.0,
        crop_factor: 1.0,
        aspect_ratio: 1.5,
        calib_tca: samples,
        ..Lens::default()
    }
}

fn lens_with_vignetting(focal_min: f32, focal_max: f32, samples: Vec<CalibVignetting>) -> Lens {
    Lens {
        focal_min,
        focal_max,
        crop_factor: 1.0,
        aspect_ratio: 1.5,
        calib_vignetting: samples,
        ..Lens::default()
    }
}

// -----------------------------// distortion //-----------------------------//

#[test]
fn distortion_no_calibration_returns_none() {
    let lens = lens_with_distortion(vec![]);
    assert!(lens.interpolate_distortion(50.0).is_none());
}

#[test]
fn distortion_only_none_entries_returns_none() {
    let lens = lens_with_distortion(vec![CalibDistortion {
        focal: 35.0,
        model: DistortionModel::None,
        real_focal: None,
    }]);
    assert!(lens.interpolate_distortion(35.0).is_none());
}

#[test]
fn distortion_exact_sample_returns_that_sample() {
    let lens = lens_with_distortion(vec![distortion(24.0, 0.05), distortion(35.0, 0.03)]);
    let res = lens.interpolate_distortion(35.0).expect("hit");
    // The exact-match branch returns the original sample verbatim.
    assert_eq!(res.focal, 35.0);
    match res.model {
        DistortionModel::Poly3 { k1 } => assert_relative_eq!(k1, 0.03),
        other => panic!("unexpected model {other:?}"),
    }
}

#[test]
fn distortion_single_sample_lens_returns_that_sample() {
    let lens = lens_with_distortion(vec![distortion(50.0, 0.04)]);
    // Query above and below the only sample: nearest-neighbor in both directions.
    for q in [10.0, 35.0, 50.0, 80.0, 200.0] {
        let res = lens.interpolate_distortion(q).expect("got value");
        let k = match res.model {
            DistortionModel::Poly3 { k1 } => k1,
            _ => panic!(),
        };
        if q == 50.0 {
            // Exact match path.
            assert_eq!(res.focal, 50.0);
        }
        assert_relative_eq!(k, 0.04);
    }
}

#[test]
fn distortion_nearest_neighbor_below_range() {
    // Two samples on the upper side; query is below both → nearest-neighbor to the closer (24mm).
    let lens = lens_with_distortion(vec![distortion(24.0, 0.05), distortion(35.0, 0.03)]);
    let res = lens.interpolate_distortion(15.0).expect("got value");
    // Both samples are to the right of the query, so nearest = 24mm.
    assert_eq!(res.focal, 24.0);
}

#[test]
fn distortion_nearest_neighbor_above_range() {
    let lens = lens_with_distortion(vec![distortion(24.0, 0.05), distortion(35.0, 0.03)]);
    let res = lens.interpolate_distortion(100.0).expect("got value");
    // Both samples are to the left of the query, so nearest = 35mm.
    assert_eq!(res.focal, 35.0);
}

#[test]
fn distortion_three_sample_interpolation_at_30mm() {
    // Samples at 24/35/70mm; query at 30mm.
    // Spline window: spline[0] = none (no left neighbor < 24mm),
    //                spline[1] = 24 (closest left),
    //                spline[2] = 35 (closest right),
    //                spline[3] = 70 (next right).
    // t = (30 - 24) / (35 - 24) = 6/11.
    // Poly3 k1 → upstream Terms[0] only, no parameter scaling for Poly3
    // distortion, so y_i = k1_i * focal_i, and the result is divided by 30.
    let lens = lens_with_distortion(vec![
        distortion(24.0, 0.05),
        distortion(35.0, 0.03),
        distortion(70.0, 0.01),
    ]);
    let res = lens.interpolate_distortion(30.0).expect("got value");
    assert_eq!(res.focal, 30.0);
    let k = match res.model {
        DistortionModel::Poly3 { k1 } => k1,
        _ => panic!(),
    };
    // Hand-computed against the Catmull-Rom formula in auxfun.rs:
    //   y1 = 0.05 * 24 = 1.20
    //   y2 = 0.03 * 35 = 1.05
    //   y3 = 0.01 * 70 = 0.70
    //   tg2 (no left) = y3 - y2 = -0.15
    //   tg3 = (y4 - y2)/2 = (0.70 - 1.20)/2 = -0.25
    //   t = 6/11 ≈ 0.5454545
    //   value ≈ 1.131706, divided by 30 ≈ 0.0377235.
    assert_relative_eq!(k, 0.037_723_5, epsilon = 1e-5);
}

#[test]
fn distortion_real_focal_propagates_when_present() {
    // Both flanking samples have real_focal → result has interpolated real_focal.
    let lens = lens_with_distortion(vec![
        CalibDistortion {
            focal: 24.0,
            model: DistortionModel::Poly3 { k1: 0.05 },
            real_focal: Some(23.5),
        },
        CalibDistortion {
            focal: 35.0,
            model: DistortionModel::Poly3 { k1: 0.03 },
            real_focal: Some(34.6),
        },
    ]);
    let res = lens.interpolate_distortion(30.0).expect("got value");
    let rf = res.real_focal.expect("real_focal interpolated");
    // With only two samples, both end-tangents collapse to (y3 - y2) = (34.6 - 23.5) = 11.1.
    // The Hermite at t = (30-24)/(35-24) = 6/11 reduces to a linear blend in this case:
    //   value = 23.5 + (34.6 - 23.5) * 6/11 = 23.5 + 6.054545... = 29.554545.
    assert_relative_eq!(rf, 29.554_546, epsilon = 1e-4);
}

#[test]
fn distortion_ptlens_model_round_trips_terms() {
    // Verify the multi-term packing/unpacking for Ptlens (a, b, c).
    let lens = lens_with_distortion(vec![
        ptlens(24.0, 0.01, 0.02, 0.03),
        ptlens(70.0, 0.005, 0.01, 0.015),
    ]);
    let res = lens.interpolate_distortion(24.0).expect("exact match");
    match res.model {
        DistortionModel::Ptlens { a, b, c } => {
            assert_relative_eq!(a, 0.01);
            assert_relative_eq!(b, 0.02);
            assert_relative_eq!(c, 0.03);
        }
        other => panic!("unexpected model {other:?}"),
    }
}

#[test]
fn distortion_mismatched_model_skipped() {
    // First-encountered model (Poly3) wins; the Ptlens entry is silently skipped.
    let lens = lens_with_distortion(vec![distortion(24.0, 0.05), ptlens(35.0, 0.01, 0.02, 0.03)]);
    // With only one usable sample, single-sample fallback returns that sample.
    let res = lens.interpolate_distortion(50.0).expect("got value");
    match res.model {
        DistortionModel::Poly3 { k1 } => assert_relative_eq!(k1, 0.05),
        other => panic!("expected Poly3, got {other:?}"),
    }
}

// -----------------------------// TCA //-----------------------------//

#[test]
fn tca_no_calibration_returns_none() {
    let lens = lens_with_tca(vec![]);
    assert!(lens.interpolate_tca(50.0).is_none());
}

#[test]
fn tca_exact_sample_returns_that_sample() {
    let lens = lens_with_tca(vec![tca_linear(35.0, 1.0001, 0.9998)]);
    let res = lens.interpolate_tca(35.0).expect("got value");
    match res.model {
        TcaModel::Linear { kr, kb } => {
            assert_relative_eq!(kr, 1.0001);
            assert_relative_eq!(kb, 0.9998);
        }
        other => panic!("unexpected model {other:?}"),
    }
}

#[test]
fn tca_linear_two_sample_linear_blend() {
    // Linear TCA, indices 0 and 1, both use scale=1.0 (the "v" terms), so the
    // interpolation collapses to a plain Catmull-Rom on the kr/kb values.
    let lens = lens_with_tca(vec![
        tca_linear(24.0, 1.001, 0.999),
        tca_linear(70.0, 1.000, 1.000),
    ]);
    let res = lens.interpolate_tca(47.0).expect("got value");
    let (kr, kb) = match res.model {
        TcaModel::Linear { kr, kb } => (kr, kb),
        _ => panic!(),
    };
    // Two-sample case → both end-tangents = (y3 - y2). For a linear blend
    // between two points, that reduces to a straight line.
    // t = (47 - 24) / (70 - 24) = 23/46 = 0.5.
    // kr = 1.001 + (1.000 - 1.001) * 0.5 = 1.0005.
    // kb = 0.999 + (1.000 - 0.999) * 0.5 = 0.9995.
    assert_relative_eq!(kr, 1.0005, epsilon = 1e-5);
    assert_relative_eq!(kb, 0.9995, epsilon = 1e-5);
}

// -----------------------------// vignetting //-----------------------------//

#[test]
fn vignetting_no_calibration_returns_none() {
    let lens = lens_with_vignetting(20.0, 50.0, vec![]);
    assert!(lens.interpolate_vignetting(35.0, 4.0, 5.0).is_none());
}

#[test]
fn vignetting_exact_sample_returns_that_sample() {
    let lens = lens_with_vignetting(20.0, 50.0, vec![vignetting(35.0, 4.0, 5.0, -0.3)]);
    let res = lens
        .interpolate_vignetting(35.0, 4.0, 5.0)
        .expect("got value");
    assert_eq!(res.focal, 35.0);
    assert_eq!(res.aperture, 4.0);
    assert_eq!(res.distance, 5.0);
    match res.model {
        VignettingModel::Pa { k1, k2, k3 } => {
            assert_relative_eq!(k1, -0.3);
            assert_relative_eq!(k2, 0.0);
            assert_relative_eq!(k3, 0.0);
        }
        other => panic!("unexpected model {other:?}"),
    }
}

#[test]
fn vignetting_far_query_returns_none() {
    // Sample at f=20mm, a=2.8, d=1m — query way out (focal=200, aperture=22, distance=100)
    // pushes the IDW distance above 1, which upstream treats as "no data".
    let lens = lens_with_vignetting(20.0, 50.0, vec![vignetting(20.0, 2.8, 1.0, -0.5)]);
    let res = lens.interpolate_vignetting(50.0, 22.0, 100.0);
    assert!(res.is_none());
}

#[test]
fn vignetting_2x2x2_grid_returns_blended_value() {
    // 2x2x2 grid in (focal, aperture, distance). Query in the middle.
    // The IDW kernel is symmetric in this setup, so all eight corners get
    // equal weight, and the result averages all eight k1s.
    let k1s = [-0.2, -0.3, -0.4, -0.5, -0.25, -0.35, -0.45, -0.55];
    let mut samples = Vec::new();
    let mut idx = 0;
    for &f in &[20.0_f32, 50.0] {
        for &a in &[2.8_f32, 8.0] {
            for &d in &[1.0_f32, 10.0] {
                samples.push(vignetting(f, a, d, k1s[idx]));
                idx += 1;
            }
        }
    }
    let lens = lens_with_vignetting(20.0, 50.0, samples);

    // Query at the geometric center on the rescaled axes:
    //   focal: midpoint = 35 ((20+50)/2)
    //   aperture: 4/aperture midpoint between 4/2.8 and 4/8 → 4/a = (1.4286+0.5)/2 = 0.9643 → a ≈ 4.148
    //   distance: 0.1/d midpoint between 0.1 and 0.01 → 0.055 → d ≈ 1.818
    let res = lens
        .interpolate_vignetting(35.0, 4.148, 1.818)
        .expect("got value");
    let k1 = match res.model {
        VignettingModel::Pa { k1, .. } => k1,
        _ => panic!(),
    };

    let mean: f32 = k1s.iter().sum::<f32>() / k1s.len() as f32;
    // IDW with equal distances → simple mean. Tolerance loose because the
    // 4.148/1.818 rounding above isn't exact-center.
    assert_relative_eq!(k1, mean, epsilon = 5e-3);
}
