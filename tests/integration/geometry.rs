//! Tests for geometry projection conversions in `mod_coord::geometry`.
//!
//! Upstream `tests/test_modifier_coord_geometry.cpp` and `test_modifier_coord_scale.cpp`
//! only assert that `lfModifier::ApplyGeometryDistortion` returns `true`. They are smoke
//! tests for the buffer pipeline, not for the math itself. Since the `Modifier` wiring
//! lives behind a separate effort, the upstream-style "loop over a 300×300 buffer" tests
//! are marked `#[ignore]` below with pointers to the upstream lines.
//!
//! The tests here exercise the per-pixel kernels directly:
//!
//! - **Optical-axis singularity:** `(0, 0) → (0, 0)` (or the documented sentinel).
//! - **Closed-form pivot points:** points where the upstream formula collapses to a
//!   recognizable identity (e.g. `tan(0) = 0`, `atan(1) = π/4`).
//! - **Round trips:** forward then reverse should be near-identity within a tolerance
//!   that matches `f32 → f64 → f32` precision loss.
//!
//! When upstream's high-level `ApplyGeometryDistortion` lands in this crate, port the
//! buffer-driving tests in full. See `tests/test_modifier_coord_geometry.cpp:74-101`.

use approx::assert_abs_diff_eq;
use std::f32::consts::PI;

use lensfun::mod_coord::geometry::{
    equisolid_erect, erect_equisolid, erect_fisheye, erect_orthographic, erect_panoramic,
    erect_rect, erect_stereographic, erect_thoby, fisheye_erect, fisheye_panoramic, fisheye_rect,
    orthographic_erect, panoramic_erect, panoramic_fisheye, panoramic_rect, rect_erect,
    rect_fisheye, rect_panoramic, stereographic_erect, thoby_erect,
};

const EPS: f32 = 1.0e-5;
const ROUND_TRIP_EPS: f32 = 1.0e-4;

// ----------------------------- optical-axis tests -----------------------------

#[test]
fn fisheye_rect_at_origin() {
    // r == 0 → rho == 1 → (0, 0). Mirrors mod-coord.cpp:795-796.
    let (x, y) = fisheye_rect(0.0, 0.0);
    assert_eq!((x, y), (0.0, 0.0));
}

#[test]
fn rect_fisheye_at_origin() {
    let (x, y) = rect_fisheye(0.0, 0.0);
    assert_eq!((x, y), (0.0, 0.0));
}

#[test]
fn panoramic_rect_at_origin() {
    let (x, y) = panoramic_rect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn rect_panoramic_at_origin() {
    let (x, y) = rect_panoramic(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn fisheye_panoramic_at_origin() {
    let (x, y) = fisheye_panoramic(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn panoramic_fisheye_at_origin() {
    let (x, y) = panoramic_fisheye(0.0, 0.0);
    // sin(0) == 0, atan2(0, cos(0)) == 0 → theta == 0 (special case).
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn rect_erect_at_origin() {
    let (x, y) = rect_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn fisheye_erect_at_origin() {
    let (x, y) = fisheye_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn erect_panoramic_at_origin() {
    let (x, y) = erect_panoramic(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn panoramic_erect_at_origin() {
    let (x, y) = panoramic_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn orthographic_erect_at_origin() {
    let (x, y) = orthographic_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn equisolid_erect_at_origin() {
    let (x, y) = equisolid_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn thoby_erect_at_origin() {
    let (x, y) = thoby_erect(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn erect_stereographic_at_origin() {
    // cosphi = 1, ksp = 2/(1+1) = 1, so result is (0, 0).
    let (x, y) = erect_stereographic(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn stereographic_erect_at_origin_returns_sentinel() {
    // mod-coord.cpp:1078-1083: rh < EPSLN → out_y = 1.6e16, out_x = 0.
    let (x, y) = stereographic_erect(0.0, 0.0);
    assert_eq!(x, 0.0);
    assert_eq!(y, 1.6e16_f32);
}

#[test]
fn erect_equisolid_at_origin() {
    // cos(0)*cos(0)+1 = 2 → not in singular branch. k1 = 1, result (0, 0).
    let (x, y) = erect_equisolid(0.0, 0.0);
    assert_abs_diff_eq!(x, 0.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

// ----------------------------- pivot-point checks -----------------------------

#[test]
fn fisheye_rect_returns_sentinel_outside_pi_half() {
    // r >= PI/2 → rho == 1.6e16. mod-coord.cpp:793-794.
    let (x, _) = fisheye_rect(PI / 2.0 + 0.1, 0.0);
    // x_out = rho * x_in = 1.6e16 * (PI/2 + 0.1).
    assert!(x > 1.0e16);
}

#[test]
fn rect_erect_known_point() {
    // rect_erect(1, 0) = (atan2(1, 1), atan2(0, sqrt(2))) = (PI/4, 0).
    let (x, y) = rect_erect(1.0, 0.0);
    assert_abs_diff_eq!(x, PI / 4.0, epsilon = EPS);
    assert_abs_diff_eq!(y, 0.0, epsilon = EPS);
}

#[test]
fn erect_panoramic_known_point() {
    // y' = tan(y), x' = x.
    let (x, y) = erect_panoramic(0.5, PI / 4.0);
    assert_abs_diff_eq!(x, 0.5, epsilon = EPS);
    assert_abs_diff_eq!(y, 1.0, epsilon = EPS);
}

#[test]
fn panoramic_erect_known_point() {
    // y' = atan(y), x' = x. Inverse of above.
    let (x, y) = panoramic_erect(0.5, 1.0);
    assert_abs_diff_eq!(x, 0.5, epsilon = EPS);
    assert_abs_diff_eq!(y, PI / 4.0, epsilon = EPS);
}

// ----------------------------- round-trip tests -----------------------------

fn assert_round_trip((rx, ry): (f32, f32), (ex, ey): (f32, f32), tag: &str) {
    assert_abs_diff_eq!(rx, ex, epsilon = ROUND_TRIP_EPS);
    assert_abs_diff_eq!(ry, ey, epsilon = ROUND_TRIP_EPS);
    let _ = tag;
}

#[test]
fn round_trip_rect_fisheye() {
    // fisheye_rect maps rect → fisheye, rect_fisheye maps fisheye → rect.
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5), (0.6, 0.0)] {
        let fwd = fisheye_rect(x, y);
        let back = rect_fisheye(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "rect↔fisheye");
    }
}

#[test]
fn round_trip_panoramic_rect() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = panoramic_rect(x, y);
        let back = rect_panoramic(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "panoramic↔rect");
    }
}

#[test]
fn round_trip_erect_panoramic() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = erect_panoramic(x, y);
        let back = panoramic_erect(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "erect↔panoramic");
    }
}

#[test]
fn round_trip_rect_erect() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5), (0.6, 0.7)] {
        let fwd = rect_erect(x, y);
        let back = erect_rect(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "rect↔erect");
    }
}

#[test]
fn round_trip_fisheye_erect() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = fisheye_erect(x, y);
        let back = erect_fisheye(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "fisheye↔erect");
    }
}

#[test]
fn round_trip_stereographic_erect() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = stereographic_erect(x, y);
        let back = erect_stereographic(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "stereographic↔erect");
    }
}

#[test]
fn round_trip_equisolid_erect() {
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = equisolid_erect(x, y);
        let back = erect_equisolid(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "equisolid↔erect");
    }
}

#[test]
fn round_trip_thoby_erect() {
    // Thoby kernel is parameterised, stay well inside |rho| < THOBY_K1_PARM (1.47).
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = thoby_erect(x, y);
        let back = erect_thoby(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "thoby↔erect");
    }
}

#[test]
fn round_trip_orthographic_erect() {
    // r < 1 keeps us out of the clamped branch.
    for &(x, y) in &[(0.1_f32, 0.1_f32), (0.3, -0.2), (-0.4, 0.5)] {
        let fwd = orthographic_erect(x, y);
        let back = erect_orthographic(fwd.0, fwd.1);
        assert_round_trip(back, (x, y), "orthographic↔erect");
    }
}

// ----------------------- ignored upstream high-level smoke -----------------------

#[test]
fn upstream_apply_geometry_distortion_smoke() {
    // Upstream sweeps every (source, target) lens-type pair on a 300×300 buffer
    // and only checks `ApplyGeometryDistortion` returns true. We exercise the
    // distortion path here using a real bundled lens (Pentax 50-200) — the
    // geometry-projection variants are not yet wired through `Modifier`.
    use std::path::Path;

    use lensfun::Database;
    use lensfun::modifier::Modifier;

    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db");
    let db = Database::load_dir(&data_dir).expect("bundled DB loads");
    let lenses = db.find_lenses(None, "pEntax 50-200 ED");
    let lens = lenses[0];

    let (img_w, img_h) = (300_u32, 300_u32);
    let mut modifier = Modifier::new(lens, 80.89, 1.534, img_w, img_h, false);
    assert!(
        modifier.enable_distortion_correction(lens),
        "distortion should enable"
    );

    let mut buf = vec![0.0_f32; (img_w as usize) * 2];
    for y in 0..img_h {
        assert!(modifier.apply_geometry_distortion(0.0, y as f32, img_w as usize, 1, &mut buf));
    }
}
