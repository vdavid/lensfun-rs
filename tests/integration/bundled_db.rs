//! Verify `Database::load_bundled()` matches `Database::load_dir("data/db")`.

use std::path::{Path, PathBuf};

use lensfun::Database;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/db")
}

#[test]
fn bundled_matches_dir_counts() {
    let from_dir = Database::load_dir(data_dir()).expect("load_dir");
    let bundled = Database::load_bundled().expect("load_bundled");

    assert_eq!(bundled.mounts.len(), from_dir.mounts.len());
    assert_eq!(bundled.cameras.len(), from_dir.cameras.len());
    assert_eq!(bundled.lenses.len(), from_dir.lenses.len());
}

#[test]
fn bundled_finds_known_camera() {
    let bundled = Database::load_bundled().expect("load_bundled");
    let cameras = bundled.find_cameras(Some("pentax"), "K100D");
    assert!(!cameras.is_empty());
    assert_eq!(cameras[0].model, "Pentax K100D");
}

#[test]
fn bundled_finds_known_lens_with_same_data_as_dir() {
    let from_dir = Database::load_dir(data_dir()).expect("load_dir");
    let bundled = Database::load_bundled().expect("load_bundled");

    let dir_hit = from_dir
        .find_lenses(None, "smc Pentax-DA 50-200mm f/4-5.6 DA ED")
        .first()
        .copied()
        .cloned()
        .expect("dir-loaded db has the lens");
    let bundled_hit = bundled
        .find_lenses(None, "smc Pentax-DA 50-200mm f/4-5.6 DA ED")
        .first()
        .copied()
        .cloned()
        .expect("bundled db has the lens");

    // Compare every field via Debug — both loaders go through the same parser,
    // so every field (incl. the parsed calibration coefficients) must match.
    assert_eq!(format!("{dir_hit:?}"), format!("{bundled_hit:?}"));
}

#[test]
fn bundled_full_lens_list_matches_dir() {
    let from_dir = Database::load_dir(data_dir()).expect("load_dir");
    let bundled = Database::load_bundled().expect("load_bundled");

    assert_eq!(bundled.lenses.len(), from_dir.lenses.len());
    for (a, b) in bundled.lenses.iter().zip(from_dir.lenses.iter()) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
