//! Tests for the lens / camera fuzzy-match scorer and `Lens::guess_parameters`.
//!
//! Mirrors the scenarios upstream exercises in `tests/test_database.cpp`,
//! plus targeted unit checks for the magic-number tiers in `MatchScore`
//! (port of `lfDatabase::MatchScore`, database.cpp:1252).

use lensfun::Database;
use lensfun::camera::Camera;
use lensfun::lens::Lens;

// -----------------------------// helpers //-----------------------------//

fn make_lens(maker: &str, model: &str, mounts: &[&str], crop: f32, focal: (f32, f32)) -> Lens {
    Lens {
        maker: maker.to_string(),
        model: model.to_string(),
        mounts: mounts.iter().map(|s| s.to_string()).collect(),
        crop_factor: crop,
        aspect_ratio: 1.5,
        focal_min: focal.0,
        focal_max: focal.1,
        ..Lens::default()
    }
}

fn make_camera(maker: &str, model: &str, mount: &str, crop: f32) -> Camera {
    Camera {
        maker: maker.to_string(),
        model: model.to_string(),
        mount: mount.to_string(),
        crop_factor: crop,
        ..Camera::default()
    }
}

fn db_with_lenses(lenses: Vec<Lens>) -> Database {
    let mut db = Database::new();
    db.lenses.extend(lenses);
    db
}

// -----------------------------// guess_parameters //-----------------------------//

#[test]
fn guess_parameters_zoom_with_aperture() {
    let mut lens = Lens {
        model: "24-70mm f/2.8".to_string(),
        ..Lens::default()
    };
    lens.guess_parameters();
    assert_eq!(lens.focal_min, 24.0);
    assert_eq!(lens.focal_max, 70.0);
    assert_eq!(lens.aperture_min, 2.8);
}

#[test]
fn guess_parameters_prime_with_prefix() {
    let mut lens = Lens {
        model: "EF 50mm f/1.4 USM".to_string(),
        ..Lens::default()
    };
    lens.guess_parameters();
    assert_eq!(lens.focal_min, 50.0);
    // Upstream's prime fallback: max := min when only one focal is captured.
    assert_eq!(lens.focal_max, 50.0);
    assert_eq!(lens.aperture_min, 1.4);
}

#[test]
fn guess_parameters_variable_aperture_zoom() {
    let mut lens = Lens {
        model: "10-20mm f/4.5-5.6".to_string(),
        ..Lens::default()
    };
    lens.guess_parameters();
    assert_eq!(lens.focal_min, 10.0);
    assert_eq!(lens.focal_max, 20.0);
    assert_eq!(lens.aperture_min, 4.5);
}

#[test]
fn guess_parameters_skips_extender() {
    let mut lens = Lens {
        model: "Canon EF 1.4x Extender III".to_string(),
        ..Lens::default()
    };
    lens.guess_parameters();
    // Extender keyword + magnification regex both reject; nothing extracted.
    assert_eq!(lens.focal_min, 0.0);
}

#[test]
fn guess_parameters_idempotent() {
    let mut lens = Lens {
        model: "24-70mm f/2.8".to_string(),
        focal_min: 99.0,
        focal_max: 199.0,
        aperture_min: 9.9,
        ..Lens::default()
    };
    lens.guess_parameters();
    // Already-set fields stay put.
    assert_eq!(lens.focal_min, 99.0);
    assert_eq!(lens.focal_max, 199.0);
    assert_eq!(lens.aperture_min, 9.9);
}

// -----------------------------// fuzzy lens search //-----------------------------//

#[test]
fn finds_lens_by_close_model_name() {
    let db = db_with_lenses(vec![
        make_lens(
            "Canon",
            "EF 24-70mm f/2.8L II USM",
            &["Canon EF"],
            1.0,
            (24.0, 70.0),
        ),
        make_lens(
            "Canon",
            "EF 50mm f/1.4 USM",
            &["Canon EF"],
            1.0,
            (50.0, 50.0),
        ),
        make_lens(
            "Canon",
            "EF 70-200mm f/2.8L IS USM",
            &["Canon EF"],
            1.0,
            (70.0, 200.0),
        ),
    ]);

    let hits = db.find_lenses(None, "EF 24-70mm f/2.8L II USM");
    assert!(!hits.is_empty(), "expected at least one match");
    assert_eq!(hits[0].model, "EF 24-70mm f/2.8L II USM");
}

#[test]
fn fuzzy_search_picks_best_among_similar() {
    let db = db_with_lenses(vec![
        make_lens(
            "Canon",
            "EF 24-70mm f/2.8L II USM",
            &["Canon EF"],
            1.0,
            (24.0, 70.0),
        ),
        make_lens(
            "Canon",
            "EF 24-105mm f/4L IS USM",
            &["Canon EF"],
            1.0,
            (24.0, 105.0),
        ),
    ]);
    let hits = db.find_lenses(None, "EF 24-70 2.8");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].model, "EF 24-70mm f/2.8L II USM");
}

// -----------------------------// mount compatibility //-----------------------------//

#[test]
fn mount_mismatch_excludes_lens() {
    // Lens A only fits Canon EF; querying with a Nikon F body must return no Canon EF hits.
    let db = db_with_lenses(vec![
        make_lens("Canon", "Test Lens", &["Canon EF"], 1.0, (50.0, 50.0)),
        make_lens("Nikon", "Test Lens", &["Nikon F"], 1.0, (50.0, 50.0)),
    ]);
    let cam = make_camera("Nikon", "Test Body", "Nikon F", 1.0);
    let hits = db.find_lenses(Some(&cam), "Test Lens");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].maker, "Nikon");
}

#[test]
fn mount_compat_chain_admits_lens() {
    // Camera on Canon EF-S — a Canon EF lens reaches it via the compat list (+9 vs +10
    // for native), so it should still appear in results.
    let mut db = Database::new();
    db.mounts.push(lensfun::Mount {
        name: "Canon EF-S".to_string(),
        compat: vec!["Canon EF".to_string()],
        ..lensfun::Mount::default()
    });
    db.lenses.push(make_lens(
        "Canon",
        "Test 50mm",
        &["Canon EF"],
        1.6,
        (50.0, 50.0),
    ));

    let cam = make_camera("Canon", "EOS Rebel", "Canon EF-S", 1.6);
    let hits = db.find_lenses(Some(&cam), "Test 50mm");
    assert_eq!(hits.len(), 1, "compat-mounted lens should be reachable");
}

#[test]
fn no_compat_chain_excludes_lens() {
    // Same lens, but the camera's mount has no compat entry — strict mismatch.
    let mut db = Database::new();
    db.mounts.push(lensfun::Mount {
        name: "Sony E".to_string(),
        compat: vec![], // no adapters
        ..lensfun::Mount::default()
    });
    db.lenses.push(make_lens(
        "Canon",
        "Test 50mm",
        &["Canon EF"],
        1.0,
        (50.0, 50.0),
    ));

    let cam = make_camera("Sony", "A7", "Sony E", 1.0);
    let hits = db.find_lenses(Some(&cam), "Test 50mm");
    assert!(hits.is_empty(), "no compat path → no match");
}

// -----------------------------// crop bucketing //-----------------------------//

#[test]
fn crop_factor_buckets_excludes_too_small_camera() {
    // Lens calibrated at crop 1.5; full-frame camera (crop 1.0) is below the 0.96 floor.
    let db = db_with_lenses(vec![make_lens(
        "X",
        "Test 50mm",
        &["Mount X"],
        1.5,
        (50.0, 50.0),
    )]);
    let cam = make_camera("X", "FF Body", "Mount X", 1.0);
    let hits = db.find_lenses(Some(&cam), "Test 50mm");
    assert!(hits.is_empty(), "FF camera should not match crop-1.5 lens");
}

#[test]
fn crop_factor_buckets_exact_match_picks_top_tier() {
    // Lens at crop 1.5, camera at crop 1.5 → bucket "10" (cam >= mc * 1.01 path is just barely
    // not hit; we hit the >= mc tier giving 5).
    let db = db_with_lenses(vec![make_lens(
        "X",
        "Test 50mm",
        &["Mount X"],
        1.5,
        (50.0, 50.0),
    )]);
    let cam = make_camera("X", "Crop Body", "Mount X", 1.5);
    let hits = db.find_lenses(Some(&cam), "Test 50mm");
    assert!(!hits.is_empty(), "exact crop match should score > 0");
}

#[test]
fn crop_factor_buckets_camera_crop_far_above_lens_still_matches() {
    // Lens calibrated at 1.0 (FF), camera at 2.0 → cam >= mc * 1.41 → low bucket but non-zero.
    let db = db_with_lenses(vec![make_lens(
        "X",
        "Test 50mm",
        &["Mount X"],
        1.0,
        (50.0, 50.0),
    )]);
    let cam = make_camera("X", "M43 Body", "Mount X", 2.0);
    let hits = db.find_lenses(Some(&cam), "Test 50mm");
    assert!(!hits.is_empty());
}

// -----------------------------// camera fuzzy search //-----------------------------//

#[test]
fn finds_camera_by_fuzzy_model() {
    let mut db = Database::new();
    db.cameras
        .push(make_camera("Pentax", "Pentax K100D", "Pentax KAF", 1.5));
    db.cameras
        .push(make_camera("Canon", "EOS R5", "Canon RF", 1.0));

    let hits = db.find_cameras(Some("pentax"), "K100D");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].model, "Pentax K100D");
}
