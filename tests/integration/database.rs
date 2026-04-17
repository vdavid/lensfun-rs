//! Port of upstream `tests/test_database.cpp`.
//!
//! Upstream's test loads the bundled XML database and exercises:
//!   - lens search by name (fuzzy)
//!   - camera search by maker + model (fuzzy)
//!   - serializing back out to a new file (Save)
//!
//! Fuzzy search and Save are scoped to later milestones (v0.4 fuzzy matcher,
//! and we explicitly do not port Save — the crate is read-only). Those tests are
//! marked `#[ignore]` with a reason. The loader-level tests below exercise
//! real-world XML files from `data/db/` to keep the parser honest.
//!
//! Mirror of `related-repos/lensfun/tests/test_database.cpp`.

use std::path::{Path, PathBuf};

use lensfun::Database;
use lensfun::calib::{DistortionModel, TcaModel, VignettingModel};
use lensfun::db::{MAX_DATABASE_VERSION, MIN_DATABASE_VERSION};
use lensfun::lens::LensType;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db")
}

fn load_bundled_db() -> Database {
    Database::load_dir(data_dir()).expect("bundled database loads cleanly")
}

// -----------------------------// loader sanity //-----------------------------//

#[test]
fn version_constants_match_upstream() {
    // Mirror of `LF_MIN_DATABASE_VERSION` / `LF_MAX_DATABASE_VERSION` in lensfun.h.in.
    assert_eq!(MIN_DATABASE_VERSION, 0);
    assert_eq!(MAX_DATABASE_VERSION, 2);
}

#[test]
fn load_bundled_database_parses() {
    let db = load_bundled_db();
    assert!(!db.mounts.is_empty(), "expected to load some mounts");
    assert!(!db.cameras.is_empty(), "expected to load some cameras");
    assert!(!db.lenses.is_empty(), "expected to load some lenses");
}

#[test]
fn load_str_handles_minimal_envelope() {
    let mut db = Database::new();
    db.load_str(
        r#"<?xml version="1.0"?>
        <lensdatabase version="2">
            <mount><name>Test Mount</name></mount>
        </lensdatabase>"#,
    )
    .unwrap();
    assert_eq!(db.mounts.len(), 1);
    assert_eq!(db.mounts[0].name, "Test Mount");
}

#[test]
fn rejects_unknown_root_element() {
    let mut db = Database::new();
    let err = db
        .load_str(r#"<?xml version="1.0"?><root version="2"></root>"#)
        .unwrap_err();
    assert!(format!("{err}").contains("expected root element"));
}

#[test]
fn rejects_too_new_version() {
    let mut db = Database::new();
    let err = db
        .load_str(r#"<?xml version="1.0"?><lensdatabase version="999"></lensdatabase>"#)
        .unwrap_err();
    assert!(format!("{err}").contains("999"));
}

// -----------------------------// element parsers //-----------------------------//

#[test]
fn parses_localized_mount_names() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <mount>
                <name>Canon EF</name>
                <name lang="de">Canon EF</name>
                <compat>Canon EF-S</compat>
                <compat>Canon RF</compat>
            </mount>
        </lensdatabase>"#,
    )
    .unwrap();
    let m = &db.mounts[0];
    assert_eq!(m.name, "Canon EF");
    assert_eq!(
        m.names_localized.get("de").map(String::as_str),
        Some("Canon EF")
    );
    assert_eq!(m.compat, vec!["Canon EF-S", "Canon RF"]);
}

#[test]
fn parses_camera_with_localized_strings() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <camera>
                <maker>Pentax Corporation</maker>
                <maker lang="en">Pentax</maker>
                <model>Pentax K100D</model>
                <model lang="en">K100D</model>
                <mount>Pentax KAF</mount>
                <cropfactor>1.531</cropfactor>
            </camera>
        </lensdatabase>"#,
    )
    .unwrap();
    let cam = &db.cameras[0];
    assert_eq!(cam.maker, "Pentax Corporation");
    assert_eq!(
        cam.maker_localized.get("en").map(String::as_str),
        Some("Pentax")
    );
    assert_eq!(cam.model, "Pentax K100D");
    assert_eq!(
        cam.model_localized.get("en").map(String::as_str),
        Some("K100D")
    );
    assert_eq!(cam.mount, "Pentax KAF");
    assert!((cam.crop_factor - 1.531).abs() < 1e-6);
}

#[test]
fn rejects_camera_missing_required_field() {
    let mut db = Database::new();
    let err = db
        .load_str(
            r#"<lensdatabase version="2">
                <camera>
                    <maker>Acme</maker>
                    <mount>Acme M</mount>
                    <cropfactor>1.5</cropfactor>
                </camera>
            </lensdatabase>"#,
        )
        .unwrap_err();
    assert!(format!("{err}").contains("invalid camera"));
}

#[test]
fn parses_camera_aspect_ratio_fraction() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <camera>
                <maker>Canon</maker>
                <model>PowerShot</model>
                <mount>Compact</mount>
                <cropfactor>5.6</cropfactor>
                <aspect-ratio>4:3</aspect-ratio>
            </camera>
        </lensdatabase>"#,
    )
    .unwrap();
    let ar = db.cameras[0].aspect_ratio.unwrap();
    assert!((ar - (4.0 / 3.0)).abs() < 1e-6);
}

#[test]
fn parses_lens_focal_aperture_value_form() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>Canon</maker>
                <model>EF 50mm f/1.8</model>
                <mount>Canon EF</mount>
                <focal value="50"/>
                <aperture min="1.8" max="22"/>
                <cropfactor>1</cropfactor>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    let lens = &db.lenses[0];
    assert_eq!(lens.focal_min, 50.0);
    assert_eq!(lens.focal_max, 50.0);
    assert_eq!(lens.aperture_min, 1.8);
    assert_eq!(lens.aperture_max, 22.0);
    assert_eq!(lens.lens_type, LensType::Rectilinear);
}

#[test]
fn parses_lens_type_case_insensitive() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>Sigma</maker>
                <model>8mm Fisheye</model>
                <mount>Canon EF</mount>
                <focal value="8"/>
                <type>Fisheye</type>
                <cropfactor>1</cropfactor>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    assert_eq!(db.lenses[0].lens_type, LensType::FisheyeEquidistant);
}

#[test]
fn parses_distortion_models() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>X</maker>
                <model>Test</model>
                <mount>Test</mount>
                <focal min="10" max="20"/>
                <cropfactor>1.5</cropfactor>
                <calibration>
                    <distortion model="poly3" focal="10" k1="-0.02"/>
                    <distortion model="poly5" focal="15" k1="0.01" k2="0.005"/>
                    <distortion model="ptlens" focal="20" a="0.001" b="-0.002" c="0.003"/>
                </calibration>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    let lens = &db.lenses[0];
    assert_eq!(lens.calib_distortion.len(), 3);
    assert!(matches!(
        lens.calib_distortion[0].model,
        DistortionModel::Poly3 { k1 } if (k1 + 0.02).abs() < 1e-6
    ));
    assert!(matches!(
        lens.calib_distortion[1].model,
        DistortionModel::Poly5 { k1, k2 }
            if (k1 - 0.01).abs() < 1e-6 && (k2 - 0.005).abs() < 1e-6
    ));
    assert!(matches!(
        lens.calib_distortion[2].model,
        DistortionModel::Ptlens { a, b, c }
            if (a - 0.001).abs() < 1e-6 && (b + 0.002).abs() < 1e-6 && (c - 0.003).abs() < 1e-6
    ));
    // RealFocal computed from terms when not given explicitly.
    let rf0 = lens.calib_distortion[0].real_focal.unwrap();
    assert!((rf0 - 10.0 * (1.0 - (-0.02))).abs() < 1e-5);
}

#[test]
fn parses_tca_linear_and_poly3() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>X</maker>
                <model>Test</model>
                <mount>Test</mount>
                <focal min="35" max="35"/>
                <cropfactor>1.5</cropfactor>
                <calibration>
                    <tca model="linear" focal="35" kr="1.0003" kb="1.0007"/>
                    <tca model="poly3" focal="35" vr="1.0001" vb="1.0002"
                         cr="0.0003" cb="0.0004" br="-0.0001" bb="-0.0002"/>
                </calibration>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    let lens = &db.lenses[0];
    assert_eq!(lens.calib_tca.len(), 2);
    match lens.calib_tca[0].model {
        TcaModel::Linear { kr, kb } => {
            assert!((kr - 1.0003).abs() < 1e-6);
            assert!((kb - 1.0007).abs() < 1e-6);
        }
        _ => panic!("expected Linear"),
    }
    match lens.calib_tca[1].model {
        TcaModel::Poly3 { red, blue } => {
            assert_eq!(red, [1.0001, 0.0003, -0.0001]);
            assert_eq!(blue, [1.0002, 0.0004, -0.0002]);
        }
        _ => panic!("expected Poly3"),
    }
}

#[test]
fn parses_vignetting_pa() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>X</maker>
                <model>Test</model>
                <mount>Test</mount>
                <focal value="50"/>
                <cropfactor>1.5</cropfactor>
                <calibration>
                    <vignetting model="pa" focal="50" aperture="2.8" distance="10"
                                k1="-0.5" k2="0.3" k3="-0.1"/>
                </calibration>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    let v = &db.lenses[0].calib_vignetting[0];
    assert_eq!(v.focal, 50.0);
    assert_eq!(v.aperture, 2.8);
    assert_eq!(v.distance, 10.0);
    assert!(matches!(
        v.model,
        VignettingModel::Pa { k1, k2, k3 }
            if (k1 + 0.5).abs() < 1e-6 && (k2 - 0.3).abs() < 1e-6 && (k3 + 0.1).abs() < 1e-6
    ));
}

#[test]
fn parses_lens_center_and_aspect_ratio() {
    let mut db = Database::new();
    db.load_str(
        r#"<lensdatabase version="2">
            <lens>
                <maker>X</maker>
                <model>Test</model>
                <mount>Test</mount>
                <focal value="50"/>
                <center x="0.01" y="-0.02"/>
                <cropfactor>2</cropfactor>
                <aspect-ratio>16:9</aspect-ratio>
            </lens>
        </lensdatabase>"#,
    )
    .unwrap();
    let lens = &db.lenses[0];
    assert_eq!(lens.center_x, 0.01);
    assert_eq!(lens.center_y, -0.02);
    assert!((lens.aspect_ratio - (16.0 / 9.0)).abs() < 1e-6);
    assert_eq!(lens.crop_factor, 2.0);
}

// -----------------------------// upstream-spec ports //-----------------------------//

// These mirror upstream `test_DB_lens_search` and `test_DB_cam_search`. They use the
// fuzzy matcher (`lfFuzzyStrCmp`), which is a separate work item. Once `find_cameras`
// and `find_lenses` get a faithful port of `lfDatabase::FindCamerasExt` /
// `FindLenses`, drop the `#[ignore]`.

#[test]
#[ignore = "blocked on v0.4 fuzzy matcher (lfFuzzyStrCmp + MatchScore)"]
fn db_cam_search() {
    let db = load_bundled_db();
    let cameras = db.find_cameras(Some("pentax"), "K100D");
    assert!(!cameras.is_empty());
    assert_eq!(cameras[0].model, "Pentax K100D");

    let cameras = db.find_cameras(None, "K 100 D");
    assert!(!cameras.is_empty());
    assert_eq!(cameras[0].model, "Pentax K100D");

    let cameras = db.find_cameras(None, "PentAX K100 D");
    assert!(!cameras.is_empty());
    assert_eq!(cameras[0].model, "Pentax K100D");
}

#[test]
#[ignore = "blocked on v0.4 fuzzy matcher (lfFuzzyStrCmp + MatchScore)"]
fn db_lens_search() {
    let db = load_bundled_db();

    let lenses = db.find_lenses(None, "pEntax 50-200 ED");
    assert!(!lenses.is_empty());
    assert_eq!(lenses[0].model, "smc Pentax-DA 50-200mm f/4-5.6 DA ED");

    let lenses = db.find_lenses(None, "smc Pentax-DA 50-200mm f/4-5.6 DA ED");
    assert!(!lenses.is_empty());
    assert_eq!(lenses[0].model, "smc Pentax-DA 50-200mm f/4-5.6 DA ED");

    let lenses = db.find_lenses(None, "PENTAX fa 28mm 2.8");
    assert!(!lenses.is_empty());
    assert_eq!(lenses[0].model, "smc Pentax-FA 28mm f/2.8 AL");

    let cameras = db.find_cameras(Some("Ricoh"), "k-70");
    assert!(!cameras.is_empty());

    let lenses = db.find_lenses(Some(cameras[0]), "Fotasy M3517 35mm");
    assert!(!lenses.is_empty());
    assert_eq!(lenses[0].model, "Fotasy M3517 35mm f/1.7");
}

// Upstream `test_DB_save` is intentionally not ported: the crate is read-only and
// the upstream `Save` path is explicitly out of scope (see AGENTS.md: "No SaveXML /
// database authoring. Read-only.").
