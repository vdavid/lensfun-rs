//! XML database loader + queries.
//!
//! Port of `libs/lensfun/database.cpp`.
//!
//! The upstream parser is a glib SAX parser; we use `roxmltree` (DOM) since the database
//! files are small (5 MB total uncompressed) and DOM is simpler to port.
//!
//! # What this module loads
//!
//! It walks every `*.xml` file in a directory, parses the `<lensdatabase version="N">`
//! envelope, and merges the `<mount>`, `<camera>`, and `<lens>` children into a single
//! [`Database`]. Localized child elements like `<name lang="de">` are preserved.
//!
//! Supported database envelope versions: 0..=2 (matching upstream
//! `LF_MIN_DATABASE_VERSION` and `LF_MAX_DATABASE_VERSION`).

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node, ParsingOptions};

use crate::auxfun::FuzzyStrCmp;
use crate::calib::{
    CalibDistortion, CalibTca, CalibVignetting, DistortionModel, TcaModel, VignettingModel,
};
use crate::camera::Camera;
use crate::error::{Error, Result};
use crate::lens::{Lens, LensType};
use crate::mount::Mount;

/// Lowest accepted value of the `<lensdatabase version="…">` attribute.
///
/// Mirrors upstream `LF_MIN_DATABASE_VERSION`.
pub const MIN_DATABASE_VERSION: u32 = 0;

/// Highest accepted value of the `<lensdatabase version="…">` attribute.
///
/// Mirrors upstream `LF_MAX_DATABASE_VERSION`.
pub const MAX_DATABASE_VERSION: u32 = 2;

/// In-memory snapshot of a parsed lensfun database directory.
#[derive(Debug, Default)]
pub struct Database {
    /// All loaded mounts.
    pub mounts: Vec<Mount>,
    /// All loaded cameras.
    pub cameras: Vec<Camera>,
    /// All loaded lenses.
    pub lenses: Vec<Lens>,
}

impl Database {
    /// Build an empty database. Equivalent to `Database::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load every `*.xml` file from the given directory.
    ///
    /// Mirrors upstream `lfDatabase::Load(path)` for a directory: each file is parsed,
    /// and all mounts / cameras / lenses are appended to a single in-memory database.
    /// The order of files is filesystem-defined (sorted here for determinism).
    ///
    /// Returns [`Error::Io`] if the directory can't be read. Per-file parse failures
    /// surface as [`Error::Xml`] or [`Error::InvalidEntry`] — upstream is more lenient
    /// (it logs a warning and continues), but for a Rust API we prefer to fail loudly.
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut db = Self::new();
        let entries = fs::read_dir(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;

        let mut xml_files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension() == Some(OsStr::new("xml")))
            .collect();
        xml_files.sort();

        for file in xml_files {
            db.load_file(&file)?;
        }

        Ok(db)
    }

    /// Load a single XML file and merge its entries into `self`.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        self.load_str_with_context(&contents, path)
    }

    /// Parse an XML string and merge its entries into `self`.
    ///
    /// Useful for tests and for callers that have already loaded the bytes themselves.
    pub fn load_str(&mut self, xml: &str) -> Result<()> {
        self.load_str_with_context(xml, Path::new("<memory>"))
    }

    fn load_str_with_context(&mut self, xml: &str, context: &Path) -> Result<()> {
        // Bundled XML files start with `<!DOCTYPE lensdatabase SYSTEM "lensfun-database.dtd">`,
        // which roxmltree refuses unless `allow_dtd` is set. We don't validate against the DTD
        // (no Rust validator dep), but we do tolerate its presence.
        let opts = ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        };
        let doc = Document::parse_with_options(xml, opts).map_err(|err| Error::Xml {
            path: context.to_path_buf(),
            message: err.to_string(),
        })?;

        let root = doc.root_element();
        if root.tag_name().name() != "lensdatabase" {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!(
                    "expected root element <lensdatabase>, got <{}>",
                    root.tag_name().name()
                ),
            });
        }

        let version: u32 = match root.attribute("version") {
            Some(v) => v.trim().parse().map_err(|_| Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("invalid lensdatabase version attribute: {v:?}"),
            })?,
            None => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: "missing version attribute on <lensdatabase>".into(),
                });
            }
        };
        // Mirror upstream's two-sided check. Today MIN_DATABASE_VERSION is 0, which
        // makes the lower bound vacuous for `u32`, but we keep the comparison so the
        // structure stays in sync if upstream ever raises the floor.
        #[allow(clippy::absurd_extreme_comparisons)]
        if version < MIN_DATABASE_VERSION {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!(
                    "database version {version} is older than the oldest supported version \
                     ({MIN_DATABASE_VERSION})"
                ),
            });
        }
        if version > MAX_DATABASE_VERSION {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!(
                    "database version {version} is newer than the newest supported version \
                     ({MAX_DATABASE_VERSION})"
                ),
            });
        }

        for child in root.children().filter(Node::is_element) {
            match child.tag_name().name() {
                "mount" => {
                    let mount = parse_mount(&child, context)?;
                    self.mounts.push(mount);
                }
                "camera" => {
                    let camera = parse_camera(&child, context)?;
                    self.cameras.push(camera);
                }
                "lens" => {
                    let lens = parse_lens(&child, context)?;
                    self.lenses.push(lens);
                }
                other => {
                    return Err(Error::InvalidEntry {
                        path: context.to_path_buf(),
                        message: format!("unexpected child of <lensdatabase>: <{other}>"),
                    });
                }
            }
        }

        Ok(())
    }

    /// Find cameras matching `maker` and `model` using fuzzy string scoring.
    ///
    /// Sorted by score descending. Empty maker / model treated as wildcard.
    // Port of lfDatabase::FindCamerasExt at database.cpp:1194.
    pub fn find_cameras(&self, maker: Option<&str>, model: &str) -> Vec<&Camera> {
        let maker = maker.filter(|s| !s.is_empty());
        let model = if model.is_empty() { None } else { Some(model) };

        let fc_maker = maker.map(|m| FuzzyStrCmp::new(m, true));
        let fc_model = model.map(|m| FuzzyStrCmp::new(m, true));

        let mut scored: Vec<(i32, &Camera)> = Vec::new();
        for cam in &self.cameras {
            let mut score1 = 0;
            let mut score2 = 0;
            if let Some(fc) = &fc_maker {
                score1 = fc.compare(&cam.maker);
                if score1 == 0 {
                    continue;
                }
            }
            if let Some(fc) = &fc_model {
                score2 = fc.compare(&cam.model);
                if score2 == 0 {
                    continue;
                }
            }
            scored.push((score1 + score2, cam));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, c)| c).collect()
    }

    /// Find lenses matching `model` (and an optional `camera` for mount + crop bucketing).
    ///
    /// Sorted by score descending. Mirrors upstream `lfDatabase::FindLenses` with
    /// `sflags = 0` (strict matcher, no sort-and-uniquify pass).
    // Port of lfDatabase::FindLenses at database.cpp:1414.
    pub fn find_lenses(&self, camera: Option<&Camera>, model: &str) -> Vec<&Lens> {
        let model_opt = if model.is_empty() { None } else { Some(model) };

        // Build a synthetic pattern lens, then run GuessParameters to extract focal /
        // aperture ranges from the model name (matches upstream's prep at database.cpp:1422).
        let mut pattern = Lens::default();
        if let Some(m) = model_opt {
            pattern.model = m.to_string();
        }
        pattern.guess_parameters();

        let fc = FuzzyStrCmp::new(pattern.model.as_str(), true);

        // Resolve compatible mounts via the camera's mount.
        let compat_mounts: Vec<&str> = match camera {
            Some(cam) => self
                .mounts
                .iter()
                .find(|m| m.name == cam.mount)
                .map(|m| m.compat.iter().map(String::as_str).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        };

        let mut scored: Vec<(i32, &Lens)> = Vec::new();
        for lens in &self.lenses {
            let s = match_score(&pattern, lens, camera, &fc, &compat_mounts);
            if s > 0 {
                scored.push((s, lens));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, l)| l).collect()
    }
}

// -----------------------------// MatchScore //-----------------------------//

// Port of `_lf_compare_num` at database.cpp:1241.
fn compare_num(a: f32, b: f32) -> i32 {
    if a == 0.0 || b == 0.0 {
        return 0; // neutral
    }
    let r = a / b;
    if r <= 0.99 || r >= 1.01 {
        return -1; // strong no
    }
    1 // strong yes
}

/// Score how well `match_lens` satisfies `pattern` (and `camera`).
///
/// Returns 0 if the entry is incompatible (mount mismatch, wrong crop bucket,
/// numeric range out of tolerance). Otherwise, a positive integer; higher is better.
// Port of lfDatabase::MatchScore at database.cpp:1252.
fn match_score(
    pattern: &Lens,
    match_lens: &Lens,
    camera: Option<&Camera>,
    fuzzycmp: &FuzzyStrCmp,
    compat_mounts: &[&str],
) -> i32 {
    let mut score: i32 = 0;

    // Crop-factor bucketing. Upstream iterates `Calibrations`; we have a single
    // `crop_factor` per lens, so the loop degenerates to one iteration.
    if let Some(cam) = camera {
        if match_lens.crop_factor > 0.0 {
            let mc = match_lens.crop_factor;
            let mut crop_score = 0;
            // Skip if camera crop is significantly *smaller* than the lens calibration crop.
            if !(cam.crop_factor > 0.01 && cam.crop_factor < mc * 0.96) {
                if cam.crop_factor >= mc * 1.41 {
                    crop_score = crop_score.max(2);
                } else if cam.crop_factor >= mc * 1.31 {
                    crop_score = crop_score.max(4);
                } else if cam.crop_factor >= mc * 1.21 {
                    crop_score = crop_score.max(6);
                } else if cam.crop_factor >= mc * 1.11 {
                    crop_score = crop_score.max(8);
                } else if cam.crop_factor >= mc * 1.01 {
                    crop_score = crop_score.max(10);
                } else if cam.crop_factor >= mc {
                    crop_score = crop_score.max(5);
                } else if cam.crop_factor >= mc * 0.96 {
                    crop_score = crop_score.max(3);
                }
            }
            if crop_score == 0 {
                return 0;
            }
            score += crop_score;
        }
    }

    match compare_num(pattern.focal_min, match_lens.focal_min) {
        -1 => return 0,
        1 => score += 10,
        _ => {}
    }
    match compare_num(pattern.focal_max, match_lens.focal_max) {
        -1 => return 0,
        1 => score += 10,
        _ => {}
    }
    match compare_num(pattern.aperture_min, match_lens.aperture_min) {
        -1 => return 0,
        1 => score += 10,
        _ => {}
    }
    match compare_num(pattern.aperture_max, match_lens.aperture_max) {
        -1 => return 0,
        1 => score += 10,
        _ => {}
    }

    // Mount compatibility.
    if !match_lens.mounts.is_empty() && (camera.is_some() || !compat_mounts.is_empty()) {
        let mut matching_mount_found = false;

        if let Some(cam) = camera {
            if !cam.mount.is_empty() {
                for m in &match_lens.mounts {
                    if m.eq_ignore_ascii_case(&cam.mount) {
                        matching_mount_found = true;
                        score += 10;
                        break;
                    }
                }
            }
        }

        if !matching_mount_found && !compat_mounts.is_empty() {
            'compat: for cm in compat_mounts {
                for m in &match_lens.mounts {
                    if m.eq_ignore_ascii_case(cm) {
                        matching_mount_found = true;
                        score += 9;
                        break 'compat;
                    }
                }
            }
        }

        if !matching_mount_found {
            return 0;
        }
    }

    // Maker comparison (case-insensitive).
    if !pattern.maker.is_empty() && !match_lens.maker.is_empty() {
        if !pattern.maker.eq_ignore_ascii_case(&match_lens.maker) {
            return 0;
        }
        score += 10;
    }

    // Fuzzy model comparison — the most complex part.
    if !pattern.model.is_empty() && !match_lens.model.is_empty() {
        let mut fz = fuzzycmp.compare(&match_lens.model);
        if fz == 0 {
            return 0;
        }
        fz = (fz * 4) / 10;
        if fz == 0 {
            fz = 1;
        }
        score += fz;
    }

    score
}

// -----------------------------// element parsers //-----------------------------//

fn parse_mount(node: &Node<'_, '_>, context: &Path) -> Result<Mount> {
    let mut mount = Mount::default();

    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "name" => {
                let text = trimmed_text(&child).unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                match child.attribute("lang") {
                    Some(lang) => {
                        mount
                            .names_localized
                            .insert(lang.to_string(), text.to_string());
                    }
                    None => mount.name = text.to_string(),
                }
            }
            "compat" => {
                if let Some(text) = trimmed_text(&child) {
                    mount.compat.push(text.to_string());
                }
            }
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unexpected child of <mount>: <{other}>"),
                });
            }
        }
    }

    if mount.name.is_empty() {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: "<mount> is missing a default <name>".into(),
        });
    }

    Ok(mount)
}

fn parse_camera(node: &Node<'_, '_>, context: &Path) -> Result<Camera> {
    let mut camera = Camera::default();
    let mut crop_factor: Option<f32> = None;

    for child in node.children().filter(Node::is_element) {
        let name = child.tag_name().name();
        match name {
            "maker" => set_localized(&child, &mut camera.maker, &mut camera.maker_localized),
            "model" => set_localized(&child, &mut camera.model, &mut camera.model_localized),
            "variant" => {
                if let Some(text) = trimmed_text(&child) {
                    // Variant is a multi-language string upstream, but the public API on
                    // lfCamera here only stores the default. Localized variants are dropped
                    // until we have a use case for them.
                    if child.attribute("lang").is_none() {
                        camera.variant = Some(text.to_string());
                    }
                }
            }
            "mount" => {
                if let Some(text) = trimmed_text(&child) {
                    camera.mount = text.to_string();
                }
            }
            "cropfactor" => {
                let text = trimmed_text(&child).unwrap_or("");
                crop_factor = Some(parse_float(text, "<cropfactor>", context)?);
            }
            "aspect-ratio" => {
                let text = trimmed_text(&child).unwrap_or("");
                camera.aspect_ratio = Some(parse_aspect_ratio(text, context)?);
            }
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unexpected child of <camera>: <{other}>"),
                });
            }
        }
    }

    camera.crop_factor = crop_factor.unwrap_or(0.0);

    // Mirror lfCamera::Check.
    if camera.maker.is_empty()
        || camera.model.is_empty()
        || camera.mount.is_empty()
        || camera.crop_factor <= 0.0
    {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: format!(
                "invalid camera definition ({}/{})",
                if camera.maker.is_empty() {
                    "???"
                } else {
                    &camera.maker
                },
                if camera.model.is_empty() {
                    "???"
                } else {
                    &camera.model
                },
            ),
        });
    }

    Ok(camera)
}

fn parse_lens(node: &Node<'_, '_>, context: &Path) -> Result<Lens> {
    let mut lens = Lens {
        lens_type: LensType::Rectilinear,
        crop_factor: 1.0,
        aspect_ratio: 1.5,
        ..Lens::default()
    };
    // Per-calibration defaults; reset by `<calibration cropfactor=…>` on the block.
    let mut calib_crop_factor = lens.crop_factor;
    let mut calib_aspect_ratio = lens.aspect_ratio;

    for child in node.children().filter(Node::is_element) {
        let name = child.tag_name().name();
        match name {
            "maker" => set_localized(&child, &mut lens.maker, &mut lens.maker_localized),
            "model" => set_localized(&child, &mut lens.model, &mut lens.model_localized),
            "mount" => {
                if let Some(text) = trimmed_text(&child) {
                    lens.mounts.push(text.to_string());
                }
            }
            "focal" => parse_focal(&child, &mut lens, context)?,
            "aperture" => parse_aperture(&child, &mut lens, context)?,
            "center" => parse_center(&child, &mut lens, context)?,
            "type" => {
                if let Some(text) = trimmed_text(&child) {
                    lens.lens_type = parse_lens_type(text, context)?;
                }
            }
            "cropfactor" => {
                let text = trimmed_text(&child).unwrap_or("");
                let v = parse_float(text, "<cropfactor>", context)?;
                lens.crop_factor = v;
                calib_crop_factor = v;
            }
            "aspect-ratio" => {
                let text = trimmed_text(&child).unwrap_or("");
                let v = parse_aspect_ratio(text, context)?;
                lens.aspect_ratio = v;
                calib_aspect_ratio = v;
            }
            "calibration" => {
                parse_calibration(
                    &child,
                    &mut lens,
                    calib_crop_factor,
                    calib_aspect_ratio,
                    context,
                )?;
            }
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unexpected child of <lens>: <{other}>"),
                });
            }
        }
    }

    // Mirror lfLens::Check (without GuessParameters, which is a v0.4 task).
    if lens.model.is_empty() || lens.mounts.is_empty() {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: format!(
                "invalid lens definition ({}/{})",
                if lens.maker.is_empty() {
                    "???"
                } else {
                    &lens.maker
                },
                if lens.model.is_empty() {
                    "???"
                } else {
                    &lens.model
                },
            ),
        });
    }
    if lens.focal_max != 0.0 && lens.focal_min > lens.focal_max {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: format!(
                "invalid lens definition (focal min {} > max {})",
                lens.focal_min, lens.focal_max
            ),
        });
    }
    if lens.aperture_max != 0.0 && lens.aperture_min > lens.aperture_max {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: format!(
                "invalid lens definition (aperture min {} > max {})",
                lens.aperture_min, lens.aperture_max
            ),
        });
    }
    if lens.crop_factor <= 0.0 || lens.aspect_ratio < 1.0 {
        return Err(Error::InvalidEntry {
            path: context.to_path_buf(),
            message: format!(
                "invalid lens definition (crop {} aspect {})",
                lens.crop_factor, lens.aspect_ratio
            ),
        });
    }

    Ok(lens)
}

fn parse_focal(node: &Node<'_, '_>, lens: &mut Lens, context: &Path) -> Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "min" => lens.focal_min = parse_float(value, "focal/min", context)?,
            "max" => lens.focal_max = parse_float(value, "focal/max", context)?,
            "value" => {
                let v = parse_float(value, "focal/value", context)?;
                lens.focal_min = v;
                lens.focal_max = v;
            }
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <focal>: {other}"),
                });
            }
        }
    }
    Ok(())
}

fn parse_aperture(node: &Node<'_, '_>, lens: &mut Lens, context: &Path) -> Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "min" => lens.aperture_min = parse_float(value, "aperture/min", context)?,
            "max" => lens.aperture_max = parse_float(value, "aperture/max", context)?,
            "value" => {
                let v = parse_float(value, "aperture/value", context)?;
                lens.aperture_min = v;
                lens.aperture_max = v;
            }
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <aperture>: {other}"),
                });
            }
        }
    }
    Ok(())
}

fn parse_center(node: &Node<'_, '_>, lens: &mut Lens, context: &Path) -> Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "x" => lens.center_x = parse_float(value, "center/x", context)?,
            "y" => lens.center_y = parse_float(value, "center/y", context)?,
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <center>: {other}"),
                });
            }
        }
    }
    Ok(())
}

fn parse_lens_type(text: &str, context: &Path) -> Result<LensType> {
    // Upstream uses _lf_strcmp, which is case-insensitive.
    let lower = text.to_ascii_lowercase();
    Ok(match lower.as_str() {
        "rectilinear" => LensType::Rectilinear,
        "fisheye" => LensType::FisheyeEquidistant,
        "panoramic" => LensType::Panoramic,
        "equirectangular" => LensType::Equirectangular,
        "orthographic" => LensType::FisheyeOrthographic,
        "stereographic" => LensType::FisheyeStereographic,
        "equisolid" => LensType::FisheyeEquisolid,
        "fisheye_thoby" => LensType::FisheyeThoby,
        other => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("invalid lens type `{other}`"),
            });
        }
    })
}

fn parse_calibration(
    node: &Node<'_, '_>,
    lens: &mut Lens,
    parent_crop: f32,
    parent_aspect: f32,
    context: &Path,
) -> Result<()> {
    let mut crop_factor = parent_crop;
    let mut aspect_ratio = parent_aspect;

    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "cropfactor" => crop_factor = parse_float(value, "calibration/cropfactor", context)?,
            "aspect-ratio" => aspect_ratio = parse_aspect_ratio(value, context)?,
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <calibration>: {other}"),
                });
            }
        }
    }

    let _ = (crop_factor, aspect_ratio); // currently stashed only on the lens, not per-entry

    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "distortion" => lens
                .calib_distortion
                .push(parse_distortion(&child, context)?),
            "tca" => lens.calib_tca.push(parse_tca(&child, context)?),
            "vignetting" => lens
                .calib_vignetting
                .push(parse_vignetting(&child, context)?),
            // Upstream also supports <crop> and <field_of_view>; those are out of
            // scope for v0.1 since the type surface doesn't carry them yet.
            "crop" | "field_of_view" => {}
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unexpected child of <calibration>: <{other}>"),
                });
            }
        }
    }

    Ok(())
}

fn parse_distortion(node: &Node<'_, '_>, context: &Path) -> Result<CalibDistortion> {
    let mut focal: f32 = 0.0;
    let mut real_focal: Option<f32> = None;
    let mut model_kind: Option<&str> = None;
    let mut terms: [f32; 5] = [0.0; 5];

    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "model" => model_kind = Some(value),
            "focal" => focal = parse_float(value, "distortion/focal", context)?,
            "real-focal" => {
                real_focal = Some(parse_float(value, "distortion/real-focal", context)?)
            }
            "a" | "k1" => terms[0] = parse_float(value, "distortion coefficient", context)?,
            "b" | "k2" => terms[1] = parse_float(value, "distortion coefficient", context)?,
            "c" | "k3" => terms[2] = parse_float(value, "distortion coefficient", context)?,
            "k4" => terms[3] = parse_float(value, "distortion coefficient", context)?,
            "k5" => terms[4] = parse_float(value, "distortion coefficient", context)?,
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <distortion>: {other}"),
                });
            }
        }
    }

    let model = match model_kind {
        Some("none") => DistortionModel::None,
        Some("poly3") => DistortionModel::Poly3 { k1: terms[0] },
        Some("poly5") => DistortionModel::Poly5 {
            k1: terms[0],
            k2: terms[1],
        },
        Some("ptlens") => DistortionModel::Ptlens {
            a: terms[0],
            b: terms[1],
            c: terms[2],
        },
        Some("acm") => {
            // ACM is part of upstream; the type surface here doesn't model it yet, so
            // store as None (with the focal preserved) to avoid losing the entry entirely.
            // TODO(v0.2): expand DistortionModel to cover ACM.
            DistortionModel::None
        }
        Some(other) => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("unknown distortion model `{other}`"),
            });
        }
        None => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: "<distortion> is missing the model attribute".into(),
            });
        }
    };

    // Upstream computes RealFocal from the model when not given explicitly.
    let real_focal = match real_focal {
        Some(rf) if rf > 0.0 => Some(rf),
        _ => match model {
            DistortionModel::Ptlens { a, b, c } => Some(focal * (1.0 - a - b - c)),
            DistortionModel::Poly3 { k1 } => Some(focal * (1.0 - k1)),
            DistortionModel::Poly5 { .. } | DistortionModel::None => Some(focal),
        },
    };

    Ok(CalibDistortion {
        focal,
        model,
        real_focal,
    })
}

fn parse_tca(node: &Node<'_, '_>, context: &Path) -> Result<CalibTca> {
    let mut focal: f32 = 0.0;
    let mut model_kind: Option<&str> = None;
    // Defaults match upstream: Terms[0]=Terms[1]=1.0, rest zero.
    let mut terms: [f32; 12] = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "model" => model_kind = Some(value),
            "focal" => focal = parse_float(value, "tca/focal", context)?,
            "kr" | "vr" | "alpha0" => terms[0] = parse_float(value, "tca term", context)?,
            "kb" | "vb" | "beta0" => terms[1] = parse_float(value, "tca term", context)?,
            "cr" | "alpha1" => terms[2] = parse_float(value, "tca term", context)?,
            "cb" | "beta1" => terms[3] = parse_float(value, "tca term", context)?,
            "br" | "alpha2" => terms[4] = parse_float(value, "tca term", context)?,
            "bb" | "beta2" => terms[5] = parse_float(value, "tca term", context)?,
            "alpha3" => terms[6] = parse_float(value, "tca term", context)?,
            "beta3" => terms[7] = parse_float(value, "tca term", context)?,
            "alpha4" => terms[8] = parse_float(value, "tca term", context)?,
            "beta4" => terms[9] = parse_float(value, "tca term", context)?,
            "alpha5" => terms[10] = parse_float(value, "tca term", context)?,
            "beta5" => terms[11] = parse_float(value, "tca term", context)?,
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <tca>: {other}"),
                });
            }
        }
    }

    let model = match model_kind {
        Some("none") => TcaModel::None,
        Some("linear") => TcaModel::Linear {
            kr: terms[0],
            kb: terms[1],
        },
        Some("poly3") => TcaModel::Poly3 {
            // Upstream stores poly3 TCA as Terms[0..6] = (vr, vb, cr, cb, br, bb).
            // Our type expresses each channel as (a, b, c) for `r' = r·(a + b·r + c·r²)`,
            // which maps to red = (vr, cr, br) and blue = (vb, cb, bb).
            red: [terms[0], terms[2], terms[4]],
            blue: [terms[1], terms[3], terms[5]],
        },
        Some("acm") => {
            // ACM not modelled yet; preserve the entry shape as None so we don't drop the focal.
            TcaModel::None
        }
        Some(other) => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("unknown tca model `{other}`"),
            });
        }
        None => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: "<tca> is missing the model attribute".into(),
            });
        }
    };

    Ok(CalibTca { focal, model })
}

fn parse_vignetting(node: &Node<'_, '_>, context: &Path) -> Result<CalibVignetting> {
    let mut focal: f32 = 0.0;
    let mut aperture: f32 = 0.0;
    let mut distance: f32 = 0.0;
    let mut model_kind: Option<&str> = None;
    let mut terms: [f32; 3] = [0.0; 3];

    for attr in node.attributes() {
        let name = attr.name();
        let value = attr.value();
        match name {
            "model" => model_kind = Some(value),
            "focal" => focal = parse_float(value, "vignetting/focal", context)?,
            "aperture" => aperture = parse_float(value, "vignetting/aperture", context)?,
            "distance" => distance = parse_float(value, "vignetting/distance", context)?,
            "k1" | "alpha1" => terms[0] = parse_float(value, "vignetting term", context)?,
            "k2" | "alpha2" => terms[1] = parse_float(value, "vignetting term", context)?,
            "k3" | "alpha3" => terms[2] = parse_float(value, "vignetting term", context)?,
            other => {
                return Err(Error::InvalidEntry {
                    path: context.to_path_buf(),
                    message: format!("unknown attribute on <vignetting>: {other}"),
                });
            }
        }
    }

    let model = match model_kind {
        Some("none") => VignettingModel::None,
        Some("pa") => VignettingModel::Pa {
            k1: terms[0],
            k2: terms[1],
            k3: terms[2],
        },
        Some("acm") => VignettingModel::None, // not modelled yet
        Some(other) => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("unknown vignetting model `{other}`"),
            });
        }
        None => {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: "<vignetting> is missing the model attribute".into(),
            });
        }
    };

    Ok(CalibVignetting {
        focal,
        aperture,
        distance,
        model,
    })
}

// -----------------------------// helpers //-----------------------------//

fn set_localized(
    node: &Node<'_, '_>,
    default: &mut String,
    localized: &mut std::collections::BTreeMap<String, String>,
) {
    let Some(text) = trimmed_text(node) else {
        return;
    };
    match node.attribute("lang") {
        Some(lang) => {
            localized.insert(lang.to_string(), text.to_string());
        }
        None => *default = text.to_string(),
    }
}

fn trimmed_text<'a>(node: &Node<'a, '_>) -> Option<&'a str> {
    node.text().map(str::trim).filter(|s| !s.is_empty())
}

fn parse_float(text: &str, what: &str, context: &Path) -> Result<f32> {
    text.trim().parse::<f32>().map_err(|_| Error::InvalidEntry {
        path: context.to_path_buf(),
        message: format!("invalid float value `{text}` for {what}"),
    })
}

fn parse_aspect_ratio(text: &str, context: &Path) -> Result<f32> {
    let text = text.trim();
    if let Some((num, den)) = text.split_once(':') {
        let n = parse_float(num, "<aspect-ratio> numerator", context)?;
        let d = parse_float(den, "<aspect-ratio> denominator", context)?;
        if d == 0.0 {
            return Err(Error::InvalidEntry {
                path: context.to_path_buf(),
                message: format!("zero denominator in <aspect-ratio> `{text}`"),
            });
        }
        Ok(n / d)
    } else {
        parse_float(text, "<aspect-ratio>", context)
    }
}
