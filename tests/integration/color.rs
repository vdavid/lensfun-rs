//! Port of `tests/test_modifier_color.cpp` and the vignetting bits of
//! `tests/test_modifier_regression.cpp`.
//!
//! Upstream's `test_modifier_color.cpp` builds an `lfModifier` and exercises
//! `ApplyColorModification` over a uniform-gray image for every combination of pixel
//! format (`u8`, `u16`, `u32`, `f32`, `f64`), pixel description (`RGB`, `RGBA`, `ARGB`),
//! and reverse/forward. The assertion is "doesn't crash" — there are no expected pixel
//! values. The first vignetting tests with concrete expected outputs live in
//! `test_modifier_regression.cpp::test_verify_vignetting_pa`, but those need the full
//! [`Modifier`] wired up (which we don't have yet).
//!
//! For v0.3 we cover the stand-alone kernel surface: identity, center-pixel,
//! roundtrip, monotonic darkening for typical Hugin coefficients, and a u8/u16
//! clamp/round sanity check. Modifier-driven verification is `#[ignore]`'d below.

use lensfun::mod_color::{
    vignetting_pa_apply_f32, vignetting_pa_apply_u8, vignetting_pa_apply_u16,
    vignetting_pa_correct_f32, vignetting_pa_correct_u16,
};

// Coefficients taken from `test_modifier_color.cpp:97` — the Canon EF 24-70mm f/2.8L II
// USM at f/2.8, 24mm, ∞. Negative k1 → corner darkening, the typical PA shape.
const K1: f32 = -0.5334;
const K2: f32 = -0.7926;
const K3: f32 = 0.5243;

// -------------------------- identity / center --------------------------//

#[test]
fn center_pixel_gain_one_apply() {
    // Odd dimensions place the exact image center on a pixel; gain there is `1 + 0 = 1`.
    let (w, h) = (101, 101);
    let mut buf = vec![0.5_f32; w * h];
    vignetting_pa_apply_f32(&mut buf, w, h, 1, K1, K2, K3);
    let center = (h / 2) * w + (w / 2);
    assert!(
        (buf[center] - 0.5).abs() < 1e-6,
        "center pixel should be unchanged, got {}",
        buf[center]
    );
}

#[test]
fn center_pixel_gain_one_correct() {
    let (w, h) = (101, 101);
    let mut buf = vec![0.5_f32; w * h];
    vignetting_pa_correct_f32(&mut buf, w, h, 1, K1, K2, K3);
    let center = (h / 2) * w + (w / 2);
    assert!((buf[center] - 0.5).abs() < 1e-6);
}

#[test]
fn zero_coefficients_is_identity() {
    let (w, h, c) = (32, 24, 3);
    let mut buf: Vec<f32> = (0..w * h * c).map(|i| (i as f32) * 1e-3).collect();
    let original = buf.clone();
    vignetting_pa_apply_f32(&mut buf, w, h, c, 0.0, 0.0, 0.0);
    for (a, b) in buf.iter().zip(original.iter()) {
        assert_eq!(a, b);
    }
}

// -------------------------- roundtrip ----------------------------------//

#[test]
fn apply_then_correct_recovers_input_f32() {
    let (w, h, c) = (97, 73, 3);
    let mut buf = vec![0.5_f32; w * h * c];
    let original = buf.clone();

    vignetting_pa_apply_f32(&mut buf, w, h, c, K1, K2, K3);
    vignetting_pa_correct_f32(&mut buf, w, h, c, K1, K2, K3);

    for (i, (a, b)) in buf.iter().zip(original.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-5,
            "pixel {i}: got {a}, expected {b} (delta {})",
            (a - b).abs()
        );
    }
}

#[test]
fn correct_then_apply_recovers_input_f32() {
    let (w, h, c) = (50, 50, 1);
    let mut buf = vec![0.4_f32; w * h * c];
    let original = buf.clone();

    vignetting_pa_correct_f32(&mut buf, w, h, c, K1, K2, K3);
    vignetting_pa_apply_f32(&mut buf, w, h, c, K1, K2, K3);

    for (a, b) in buf.iter().zip(original.iter()) {
        assert!((a - b).abs() < 1e-5);
    }
}

// -------------------------- shape sanity -------------------------------//

#[test]
fn forward_darkens_corners_for_negative_k1() {
    // With negative k1 the gain at the corner < 1 < gain at the center.
    let (w, h) = (199, 149);
    let mut buf = vec![1.0_f32; w * h];
    vignetting_pa_apply_f32(&mut buf, w, h, 1, K1, K2, K3);

    let center = (h / 2) * w + (w / 2);
    let corner = 0; // top-left
    assert!(
        buf[corner] < buf[center],
        "corner ({}) should be darker than center ({})",
        buf[corner],
        buf[center]
    );
    assert!((buf[center] - 1.0).abs() < 1e-6);
}

#[test]
fn correct_brightens_corners_for_negative_k1() {
    // Inverse: correcting an under-corrected image should *brighten* the corners.
    let (w, h) = (199, 149);
    let mut buf = vec![0.5_f32; w * h];
    vignetting_pa_correct_f32(&mut buf, w, h, 1, K1, K2, K3);

    let center = (h / 2) * w + (w / 2);
    let corner = 0;
    assert!(
        buf[corner] > buf[center],
        "corner ({}) should be brighter than center ({})",
        buf[corner],
        buf[center]
    );
}

// -------------------------- multi-channel ------------------------------//

#[test]
fn all_channels_get_same_gain() {
    let (w, h, c) = (33, 17, 4);
    let mut buf = vec![0.5_f32; w * h * c];
    vignetting_pa_apply_f32(&mut buf, w, h, c, K1, K2, K3);

    // Within any one pixel, all `c` channels share the same gain → same output.
    for px in 0..(w * h) {
        let base = px * c;
        for ch in 1..c {
            assert_eq!(
                buf[base],
                buf[base + ch],
                "pixel {px}: channel 0 ({}) != channel {ch} ({})",
                buf[base],
                buf[base + ch]
            );
        }
    }
}

// -------------------------- integer kernels ----------------------------//

#[test]
fn u16_apply_then_correct_recovers_input() {
    let (w, h) = (97, 73);
    let mut buf = vec![16000_u16; w * h];
    let original = buf.clone();

    vignetting_pa_apply_u16(&mut buf, w, h, 1, K1, K2, K3);
    vignetting_pa_correct_u16(&mut buf, w, h, 1, K1, K2, K3);

    // Integer roundtrip: allow a few LSB. Where the forward gain is small (corners),
    // the reverse pass divides by it and amplifies rounding noise — empirically up to
    // ~5 LSB on a u16 mid-gray with the Canon EF coefficients.
    for (i, (a, b)) in buf.iter().zip(original.iter()).enumerate() {
        let d = (i32::from(*a) - i32::from(*b)).abs();
        assert!(d <= 8, "pixel {i}: got {a}, expected ~{b} (delta {d})");
    }
}

#[test]
fn u16_zero_input_stays_zero() {
    let (w, h) = (32, 32);
    let mut buf = vec![0_u16; w * h];
    vignetting_pa_apply_u16(&mut buf, w, h, 1, K1, K2, K3);
    assert!(buf.iter().all(|&p| p == 0));
}

#[test]
fn u8_saturates_at_top() {
    // Forward at center has gain = 1 → pixel unchanged. Make sure clamping doesn't
    // do anything weird at the saturation edge.
    let (w, h) = (5, 5);
    let mut buf = vec![255_u8; w * h];
    vignetting_pa_apply_u8(&mut buf, w, h, 1, 0.0, 0.0, 0.0);
    assert!(buf.iter().all(|&p| p == 255));
}

// -------------------------- modifier-blocked ---------------------------//

#[test]
fn modifier_apply_color_modification_does_not_crash_for_every_format() {
    // Port of `test_modifier_color.cpp::test_mod_color<T>`. The upstream sweep
    // covers every (format, pixel desc, alignment) tuple; we cover the three
    // pixel formats we expose (`u8`, `u16`, `f32`) and the typical channel
    // counts. Just an "apply doesn't crash and returns true" smoke.
    use std::path::Path;

    use lensfun::Database;
    use lensfun::modifier::Modifier;

    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db");
    let db = Database::load_dir(&data_dir).expect("bundled DB loads");
    let lenses = db.find_lenses(None, "Olympus ED 14-42mm");
    let lens = lenses[0];

    let (w, h) = (32_usize, 16_usize);
    for &reverse in &[false, true] {
        let mut m = Modifier::new(lens, 17.89, 2.0, w as u32, h as u32, reverse);
        assert!(m.enable_vignetting_correction(lens, 5.0, 1000.0));

        for &channels in &[1_usize, 3, 4] {
            let mut buf_u8 = vec![128_u8; w * h * channels];
            assert!(m.apply_color_modification_u8(&mut buf_u8, 0.0, 0.0, w, h, channels));

            let mut buf_u16 = vec![32000_u16; w * h * channels];
            assert!(m.apply_color_modification_u16(&mut buf_u16, 0.0, 0.0, w, h, channels));

            let mut buf_f32 = vec![0.5_f32; w * h * channels];
            assert!(m.apply_color_modification_f32(&mut buf_f32, 0.0, 0.0, w, h, channels));
        }
    }
}

#[test]
fn verify_vignetting_pa_olympus_zuiko() {
    // Port of `test_verify_vignetting_pa` — uses bundled-DB lookup
    // (`Olympus Zuiko Digital ED 14-42mm f/3.5-5.6`), `RealFocal`-aware coefficient
    // rescaling, and `Modifier::apply_color_modification_u16`. Reproduces upstream's exact
    // u16 expected values (22406, 22406, 24156, 28803). NB: upstream re-uses the same
    // 3-pixel buffer across all four samples without reset (test_modifier_regression.cpp:169).
    use std::path::Path;

    use lensfun::Database;
    use lensfun::modifier::Modifier;

    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db");
    let db = Database::load_dir(&data_dir).expect("bundled DB loads");
    let lenses = db.find_lenses(None, "Olympus ED 14-42mm");
    let lens = lenses[0];

    let mut m = Modifier::new(lens, 17.89, 2.0, 1500, 1000, false);
    assert!(m.enable_vignetting_correction(lens, 5.0, 1000.0));

    let xs = [0.0_f32, 751.0, 810.0, 1270.0];
    let ys = [0.0_f32, 497.0, 937.0, 100.0];
    let expected: [u16; 4] = [22406, 22406, 24156, 28803];

    let mut buf = [16000_u16, 16000, 16000];
    for i in 0..xs.len() {
        assert!(m.apply_color_modification_u16(&mut buf, xs[i], ys[i], 1, 1, 3));
        for (ch, &v) in buf.iter().enumerate() {
            assert_eq!(v, expected[i], "ch{ch} sample {i}");
        }
    }
}
