//! Port of `tests/test_modifier_regression.cpp`.
//!
//! Pins specific output pixel values within `1e-3` tolerance, against the bundled
//! XML database. These ARE the spec for the high-level `Modifier` API.
//!
//! Float tolerance: upstream uses `1e-3` for absolute coordinate / pixel match.
//! We mirror that with `approx::assert_abs_diff_eq!`.

// Expected-value literals are copied verbatim from upstream
// `test_modifier_regression.cpp` (which prints `%.8f`). The trailing digits
// past `f32` precision are preserved so a diff against upstream is trivial.
#![allow(clippy::excessive_precision)]

use std::path::{Path, PathBuf};

use approx::assert_abs_diff_eq;
use lensfun::Database;
use lensfun::modifier::Modifier;

const IMG_WIDTH: u32 = 1500;
const IMG_HEIGHT: u32 = 1000;
const TOL: f32 = 1e-3;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db")
}

fn load_db() -> Database {
    Database::load_dir(data_dir()).expect("bundled database loads cleanly")
}

// ---------------------- distortion ----------------------

#[test]
fn verify_dist_poly3_pentax_50_200() {
    // Port of test_modifier_regression.cpp:36 (test_verify_dist_poly3).
    let db = load_db();
    let lenses = db.find_lenses(None, "pEntax 50-200 ED");
    assert!(!lenses.is_empty(), "expected to find Pentax 50-200 ED");
    let lens = lenses[0];
    assert_eq!(lens.model, "smc Pentax-DA 50-200mm f/4-5.6 DA ED");

    let mut m = Modifier::new(lens, 80.89, 1.534, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(
        m.enable_distortion_correction(lens),
        "distortion should enable"
    );

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let exp_x = [-14.016061_f32, 751.0, 810.27203, 1275.1655];
    let exp_y = [-9.3409109_f32, 497.0, 938.96729, 96.035286];

    for i in 0..xs.len() {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(xs[i], ys[i], 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], exp_x[i], epsilon = TOL);
        assert_abs_diff_eq!(coords[1], exp_y[i], epsilon = TOL);
    }
}

#[test]
fn verify_dist_poly5_canon_g12() {
    // Port of test_modifier_regression.cpp:89 (test_verify_dist_poly5).
    let db = load_db();
    let lenses = db.find_lenses(None, "Canon PowerShot G12");
    assert!(!lenses.is_empty(), "expected to find Canon PowerShot G12");
    let lens = lenses[0];
    assert_eq!(lens.model, "Canon PowerShot G12 & compatibles (Standard)");

    let mut m = Modifier::new(lens, 10.89, 4.6, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(m.enable_distortion_correction(lens));

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let exp_x = [28.805828_f32, 751.0, 809.50531, 1260.1396];
    let exp_y = [19.197506_f32, 497.0, 933.42279, 107.56808];

    for i in 0..xs.len() {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(xs[i], ys[i], 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], exp_x[i], epsilon = TOL);
        assert_abs_diff_eq!(coords[1], exp_y[i], epsilon = TOL);
    }
}

#[test]
fn verify_dist_ptlens_pentax_28_80() {
    // Port of test_modifier_regression.cpp:122 (test_verify_dist_ptlens).
    let db = load_db();
    let lenses = db.find_lenses(None, "PENTAX-F 28-80mm");
    assert!(!lenses.is_empty(), "expected to find Pentax-F 28-80mm");
    let lens = lenses[0];
    assert_eq!(lens.model, "Pentax-F 28-80mm f/3.5-4.5");

    let mut m = Modifier::new(lens, 30.89, 1.534, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(m.enable_distortion_correction(lens));

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let exp_x = [29.019449_f32, 750.99969, 808.74231, 1255.1388];
    let exp_y = [19.339846_f32, 497.00046, 927.90521, 111.40639];

    for i in 0..xs.len() {
        let mut coords = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(xs[i], ys[i], 1, 1, &mut coords));
        assert_abs_diff_eq!(coords[0], exp_x[i], epsilon = TOL);
        assert_abs_diff_eq!(coords[1], exp_y[i], epsilon = TOL);
    }
}

// ---------------------- vignetting ----------------------

#[test]
fn verify_vignetting_pa_olympus_zuiko() {
    // Port of test_modifier_regression.cpp:153 (test_verify_vignetting_pa).
    let db = load_db();
    let lenses = db.find_lenses(None, "Olympus ED 14-42mm");
    assert!(!lenses.is_empty(), "expected to find Olympus 14-42mm");
    let lens = lenses[0];
    assert_eq!(lens.model, "Olympus Zuiko Digital ED 14-42mm f/3.5-5.6");

    let mut m = Modifier::new(lens, 17.89, 2.0, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(m.enable_vignetting_correction(lens, 5.0, 1000.0));

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let expected: [u16; 4] = [22406, 22406, 24156, 28803];

    // NB: upstream re-uses the same buffer across samples without resetting it
    // (see test_modifier_regression.cpp:169). Sample 1 sits at image center where
    // gain ≈ 1, so it keeps sample 0's output. Mirror that here.
    let mut buf = [16000_u16, 16000, 16000];
    for i in 0..xs.len() {
        assert!(m.apply_color_modification_u16(&mut buf, xs[i], ys[i], 1, 1, 3));
        for (ch, &v) in buf.iter().enumerate() {
            assert_eq!(
                v, expected[i],
                "vignetting ch{ch} at sample {i}: got {} expected {}",
                v, expected[i]
            );
        }
    }
}

// ---------------------- subpixel TCA ----------------------

#[test]
fn verify_subpix_linear_olympus_zuiko() {
    // Port of test_modifier_regression.cpp:208 (test_verify_subpix_linear).
    let db = load_db();
    let lenses = db.find_lenses(None, "Olympus ED 14-42mm");
    assert!(!lenses.is_empty());
    let lens = lenses[0];
    assert_eq!(lens.model, "Olympus Zuiko Digital ED 14-42mm f/3.5-5.6");

    let mut m = Modifier::new(lens, 17.89, 2.0, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(m.enable_tca_correction(lens), "TCA should enable");

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let expected: [[f32; 6]; 4] = [
        [
            -0.08681729,
            -0.05789410,
            0.00002450,
            -0.00001032,
            -0.02400517,
            -0.01601936,
        ],
        [
            751.00061035,
            496.99899292,
            751.00000000,
            497.00000000,
            751.00000000,
            497.00000000,
        ],
        [
            810.01995850,
            937.14440918,
            810.00000000,
            937.00000000,
            810.00042725,
            937.00305176,
        ],
        [
            1270.12915039,
            99.90086365,
            1270.00000000,
            100.00000763,
            1270.00854492,
            99.99343872,
        ],
    ];

    for i in 0..xs.len() {
        let mut coords = [0.0_f32; 6];
        assert!(m.apply_subpixel_distortion(xs[i], ys[i], 1, 1, &mut coords));
        for j in 0..6 {
            assert_abs_diff_eq!(coords[j], expected[i][j], epsilon = TOL);
        }
    }
}

#[test]
fn verify_subpix_poly3_olympus_zuiko() {
    // Port of test_modifier_regression.cpp:243 (test_verify_subpix_poly3).
    let db = load_db();
    let lenses = db.find_lenses(None, "Olympus ED 14-42mm");
    assert!(!lenses.is_empty());
    let lens = lenses[0];
    assert_eq!(lens.model, "Olympus Zuiko Digital ED 14-42mm f/3.5-5.6");

    let mut m = Modifier::new(lens, 26.89, 2.0, IMG_WIDTH, IMG_HEIGHT, false);
    assert!(m.enable_tca_correction(lens));

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let expected: [[f32; 6]; 4] = [
        [
            -0.05537901,
            -0.03692452,
            0.00002450,
            -0.00001032,
            0.01445518,
            0.00962087,
        ],
        [
            751.00061035,
            496.99902344,
            751.00000000,
            497.00000000,
            750.99981689,
            497.00030518,
        ],
        [
            810.01898193,
            937.13732910,
            810.00000000,
            937.00000000,
            809.99389648,
            936.95599365,
        ],
        [
            1270.11572266,
            99.91123199,
            1270.00000000,
            100.00000763,
            1269.96374512,
            100.02780914,
        ],
    ];

    for i in 0..xs.len() {
        let mut coords = [0.0_f32; 6];
        assert!(m.apply_subpixel_distortion(xs[i], ys[i], 1, 1, &mut coords));
        for j in 0..6 {
            assert_abs_diff_eq!(coords[j], expected[i][j], epsilon = TOL);
        }
    }
}

// ---------------------- multi-pixel sanity ----------------------

#[test]
fn apply_geometry_distortion_walks_a_row() {
    // Single-row, multi-column buffer call. Compares to single-pixel calls so any
    // row-stepping bug shows up.
    let db = load_db();
    let lenses = db.find_lenses(None, "pEntax 50-200 ED");
    let lens = lenses[0];
    let mut m = Modifier::new(lens, 80.89, 1.534, IMG_WIDTH, IMG_HEIGHT, false);
    m.enable_distortion_correction(lens);

    let mut row = vec![0.0_f32; 4 * 2];
    assert!(m.apply_geometry_distortion(100.0, 200.0, 4, 1, &mut row));

    for i in 0..4 {
        let mut single = [0.0_f32; 2];
        assert!(m.apply_geometry_distortion(100.0 + i as f32, 200.0, 1, 1, &mut single));
        assert_abs_diff_eq!(row[2 * i], single[0], epsilon = TOL);
        assert_abs_diff_eq!(row[2 * i + 1], single[1], epsilon = TOL);
    }
}
