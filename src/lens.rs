//! Lens type and 4D calibration interpolation.
//!
//! Port of `libs/lensfun/lens.cpp` and the `lfLens` type in
//! `include/lensfun/lensfun.h.in`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::auxfun::{NO_NEIGHBOR, catmull_rom_interpolate};
use crate::calib::{
    CalibDistortion, CalibTca, CalibVignetting, DistortionModel, TcaModel, VignettingModel,
};

/// Coarse lens family.
///
/// Mirrors upstream `lfLensType` — see `include/lensfun/lensfun.h.in`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LensType {
    /// Standard rectilinear lens.
    #[default]
    Rectilinear,
    /// Equidistant fisheye — `r = f · θ`.
    FisheyeEquidistant,
    /// Orthographic fisheye — `r = f · sin(θ)`.
    FisheyeOrthographic,
    /// Equisolid fisheye — `r = 2 · f · sin(θ / 2)`.
    FisheyeEquisolid,
    /// Stereographic fisheye — `r = 2 · f · tan(θ / 2)`.
    FisheyeStereographic,
    /// Thoby fisheye — empirical model used by some Nikon lenses.
    FisheyeThoby,
    /// Equirectangular projection.
    Equirectangular,
    /// Panoramic (cylindrical) projection.
    Panoramic,
    /// Unknown / not yet classified.
    Unknown,
}

/// A camera lens entry.
///
/// Calibrations are sparse — a database entry has, for example, three distortion
/// measurements at 24mm/35mm/70mm. The interpolation in `interpolate_distortion`
/// (port of `lfLens::Interpolate*` in `lens.cpp:910-1292`) finds the right
/// profile for an arbitrary focal length / aperture / distance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lens {
    /// Lens manufacturer (e.g. `"Canon"`).
    pub maker: String,
    /// Localized maker names keyed by `lang`.
    pub maker_localized: BTreeMap<String, String>,
    /// Lens model (e.g. `"EF 24-70mm f/2.8L II USM"`).
    pub model: String,
    /// Localized model names keyed by `lang`.
    pub model_localized: BTreeMap<String, String>,
    /// Coarse projection type.
    pub lens_type: LensType,
    /// Mounts this lens fits.
    pub mounts: Vec<String>,
    /// Minimum focal length in mm.
    pub focal_min: f32,
    /// Maximum focal length in mm.
    pub focal_max: f32,
    /// Minimum aperture (f-number).
    pub aperture_min: f32,
    /// Maximum aperture (f-number).
    pub aperture_max: f32,
    /// Crop factor of the camera body the lens was calibrated against.
    pub crop_factor: f32,
    /// Aspect ratio of the calibration images. Defaults to 1.5 (3:2). Mirrors the
    /// `lfLens::AspectRatio` legacy field upstream.
    pub aspect_ratio: f32,
    /// Horizontal shift of the lens distortion center, relative to the image center,
    /// expressed as a fraction of the longer image dimension (range -0.5 .. +0.5).
    /// Mirrors `lfLens::CenterX`.
    pub center_x: f32,
    /// Vertical shift of the lens distortion center. Mirrors `lfLens::CenterY`.
    pub center_y: f32,
    /// Distortion calibration samples.
    pub calib_distortion: Vec<CalibDistortion>,
    /// TCA calibration samples.
    pub calib_tca: Vec<CalibTca>,
    /// Vignetting calibration samples.
    pub calib_vignetting: Vec<CalibVignetting>,
    /// Match score for query results.
    pub score: i32,
}

// -----------------------------// 4D calibration interpolation //-----------------------------//
//
// Port of `lfLens::Interpolate*` (lens.cpp:910-1207). The Rust port doesn't
// model `lfLensCalibrationSet` — calibrations are stored flat on `Lens` — so
// the "find calibration set with closest crop factor" preamble is dropped, and
// the per-set crop/aspect sync isn't needed either. Everything else is a
// faithful port.
//
// Float discipline: f32 end-to-end, mirroring upstream.

/// Cap of `Terms` per upstream `lfLensCalibDistortion`: `Terms[5]`.
const DISTORTION_TERM_COUNT: usize = 5;
/// Cap of `Terms` per upstream `lfLensCalibTCA`: `Terms[12]` — but the Linear
/// and Poly3 models we support use 2 and 6 entries respectively.
const TCA_TERM_COUNT: usize = 6;
/// Cap of `Terms` per upstream `lfLensCalibVignetting`: `Terms[3]` for `Pa`.
const VIGNETTING_TERM_COUNT: usize = 3;

/// Discriminant of the distortion model variant — used to enforce upstream's
/// "first encountered model wins" rule when scanning sparse calibrations.
fn distortion_kind(m: &DistortionModel) -> u8 {
    match m {
        DistortionModel::None => 0,
        DistortionModel::Poly3 { .. } => 1,
        DistortionModel::Poly5 { .. } => 2,
        DistortionModel::Ptlens { .. } => 3,
    }
}

fn tca_kind(m: &TcaModel) -> u8 {
    match m {
        TcaModel::None => 0,
        TcaModel::Linear { .. } => 1,
        TcaModel::Poly3 { .. } => 2,
    }
}

fn vignetting_kind(m: &VignettingModel) -> u8 {
    match m {
        VignettingModel::None => 0,
        VignettingModel::Pa { .. } => 1,
    }
}

/// Pack a distortion model into the upstream `Terms[5]` layout.
// Mirrors the XML attribute order in db.rs:590-594 (a/k1, b/k2, c/k3, k4, k5).
fn distortion_terms(m: &DistortionModel) -> [f32; DISTORTION_TERM_COUNT] {
    match *m {
        DistortionModel::None => [0.0; DISTORTION_TERM_COUNT],
        DistortionModel::Poly3 { k1 } => [k1, 0.0, 0.0, 0.0, 0.0],
        DistortionModel::Poly5 { k1, k2 } => [k1, k2, 0.0, 0.0, 0.0],
        DistortionModel::Ptlens { a, b, c } => [a, b, c, 0.0, 0.0],
    }
}

/// Rebuild a distortion model variant from the upstream `Terms[5]` layout.
fn distortion_from_terms(kind: u8, t: [f32; DISTORTION_TERM_COUNT]) -> DistortionModel {
    match kind {
        1 => DistortionModel::Poly3 { k1: t[0] },
        2 => DistortionModel::Poly5 { k1: t[0], k2: t[1] },
        3 => DistortionModel::Ptlens {
            a: t[0],
            b: t[1],
            c: t[2],
        },
        _ => DistortionModel::None,
    }
}

/// Pack a TCA model into upstream `Terms[6]` order: `(vr, vb, cr, cb, br, bb)`.
// See db.rs:692-698 for the inverse mapping.
fn tca_terms(m: &TcaModel) -> [f32; TCA_TERM_COUNT] {
    match *m {
        TcaModel::None => [0.0; TCA_TERM_COUNT],
        TcaModel::Linear { kr, kb } => [kr, kb, 0.0, 0.0, 0.0, 0.0],
        TcaModel::Poly3 { red, blue } => [red[0], blue[0], red[1], blue[1], red[2], blue[2]],
    }
}

fn tca_from_terms(kind: u8, t: [f32; TCA_TERM_COUNT]) -> TcaModel {
    match kind {
        1 => TcaModel::Linear { kr: t[0], kb: t[1] },
        2 => TcaModel::Poly3 {
            red: [t[0], t[2], t[4]],
            blue: [t[1], t[3], t[5]],
        },
        _ => TcaModel::None,
    }
}

fn vignetting_terms(m: &VignettingModel) -> [f32; VIGNETTING_TERM_COUNT] {
    match *m {
        VignettingModel::None => [0.0; VIGNETTING_TERM_COUNT],
        VignettingModel::Pa { k1, k2, k3 } => [k1, k2, k3],
    }
}

fn vignetting_from_terms(kind: u8, t: [f32; VIGNETTING_TERM_COUNT]) -> VignettingModel {
    match kind {
        1 => VignettingModel::Pa {
            k1: t[0],
            k2: t[1],
            k3: t[2],
        },
        _ => VignettingModel::None,
    }
}

/// In-place parameter axis scaling for distortion. Mirrors `__parameter_scales`
/// (lens.cpp:854-907) for `LF_MODIFY_DISTORTION` over the Poly3/Poly5/Ptlens
/// models we support — all of which are no-ops, so `values` is left as-is. ACM
/// (the only branch that mutates) isn't ported.
fn distortion_param_scales(_values: &mut [f32], _model_kind: u8, _index: usize) {
    // Intentional no-op for Poly3/Poly5/Ptlens.
}

/// In-place parameter axis scaling for TCA. Mirrors `__parameter_scales`
/// (lens.cpp:875-891) for `LF_MODIFY_TCA`. Linear and Poly3 share the rule:
/// indices < 2 (the "v" terms close to 1) collapse to scale 1.0; the rest keep
/// the focal scaling.
fn tca_param_scales(values: &mut [f32], _model_kind: u8, index: usize) {
    if index < 2 {
        for v in values.iter_mut() {
            *v = 1.0;
        }
    }
}

/// In-place parameter axis scaling for vignetting. Mirrors
/// `__parameter_scales` (lens.cpp:893-906) for `LF_MODIFY_VIGNETTING`. The
/// `Pa` model collapses every value to 1.0, so the focal scaling drops out
/// entirely on both ends of the IDW interpolation.
fn vignetting_param_scales(values: &mut [f32], _model_kind: u8, _index: usize) {
    for v in values.iter_mut() {
        *v = 1.0;
    }
}

/// Insert a sample into the four-slot spline window keyed by signed focal-axis
/// distance `dist = focal - sample.focal`.
///
/// `slots[0]` and `slots[1]` are the two closest left neighbors (`dist < 0`,
/// most negative first); `slots[2]` and `slots[3]` are the two closest right
/// neighbors. `dists[0..4]` are seeded as `[-INF, -INF, +INF, +INF]` so the
/// first samples on each side win automatically.
// Port of `__insert_spline` in lens.cpp:751-788.
fn insert_spline<T: Copy>(slots: &mut [Option<T>; 4], dists: &mut [f32; 4], dist: f32, val: T) {
    if dist < 0.0 {
        if dist > dists[1] {
            dists[0] = dists[1];
            dists[1] = dist;
            slots[0] = slots[1];
            slots[1] = Some(val);
        } else if dist > dists[0] {
            dists[0] = dist;
            slots[0] = Some(val);
        }
    } else if dist < dists[2] {
        dists[3] = dists[2];
        dists[2] = dist;
        slots[3] = slots[2];
        slots[2] = Some(val);
    } else if dist < dists[3] {
        dists[3] = dist;
        slots[3] = Some(val);
    }
}

/// Vignetting "interpolation distance" — the IDW kernel distance.
///
/// Port of `__vignetting_dist` (lens.cpp:1101-1120). Translates focal /
/// aperture / distance to a normalized linear-ish space, then takes the L2
/// distance.
fn vignetting_dist(
    lens_min_focal: f32,
    lens_max_focal: f32,
    sample: &CalibVignetting,
    focal: f32,
    aperture: f32,
    distance: f32,
) -> f32 {
    let mut f1 = focal - lens_min_focal;
    let mut f2 = sample.focal - lens_min_focal;
    let df = lens_max_focal - lens_min_focal;
    if df != 0.0 {
        f1 /= df;
        f2 /= df;
    }
    let a1 = 4.0 / aperture;
    let a2 = 4.0 / sample.aperture;
    let d1 = 0.1 / distance;
    let d2 = 0.1 / sample.distance;

    ((f2 - f1).powi(2) + (a2 - a1).powi(2) + (d2 - d1).powi(2)).sqrt()
}

impl Lens {
    /// Interpolate a distortion calibration at the requested `focal` length.
    ///
    /// Returns `None` if the lens has no usable distortion calibrations (every
    /// entry is `DistortionModel::None`). Falls back to the nearest sample if
    /// only one side of `focal` has data.
    // Port of `lfLens::InterpolateDistortion` in lens.cpp:910-1005.
    pub fn interpolate_distortion(&self, focal: f32) -> Option<CalibDistortion> {
        let mut slots: [Option<&CalibDistortion>; 4] = [None; 4];
        let mut dists: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];
        let mut model_kind: u8 = 0;

        for c in &self.calib_distortion {
            let kind = distortion_kind(&c.model);
            if kind == 0 {
                continue;
            }
            if model_kind == 0 {
                model_kind = kind;
            } else if model_kind != kind {
                // Upstream warns about multiple models per lens and skips. We
                // do the same silently — there's no logger plumbed yet.
                continue;
            }

            let df = focal - c.focal;
            if df == 0.0 {
                // Exact match: no interpolation needed.
                return Some(*c);
            }
            insert_spline(&mut slots, &mut dists, df, c);
        }

        // Only one side has samples → nearest neighbor.
        if slots[1].is_none() || slots[2].is_none() {
            return slots[1].or(slots[2]).copied();
        }
        let s0 = slots[0];
        let s1 = slots[1].expect("checked above");
        let s2 = slots[2].expect("checked above");
        let s3 = slots[3];

        let t = (focal - s1.focal) / (s2.focal - s1.focal);

        // RealFocal is interpolated as a plain scalar, no parameter scaling.
        let real_focal = match (s1.real_focal, s2.real_focal) {
            (Some(rf1), Some(rf2)) => {
                let rf0 = s0.and_then(|c| c.real_focal).unwrap_or(NO_NEIGHBOR);
                let rf3 = s3.and_then(|c| c.real_focal).unwrap_or(NO_NEIGHBOR);
                Some(catmull_rom_interpolate(rf0, rf1, rf2, rf3, t))
            }
            _ => None,
        };

        let t0 = distortion_terms(&s1.model);
        let t1 = distortion_terms(&s2.model);
        let t_left = s0.map(|c| distortion_terms(&c.model));
        let t_right = s3.map(|c| distortion_terms(&c.model));

        let mut out_terms = [0.0_f32; DISTORTION_TERM_COUNT];
        for i in 0..DISTORTION_TERM_COUNT {
            let f0 = s0.map(|c| c.focal).unwrap_or(f32::NAN);
            let f3 = s3.map(|c| c.focal).unwrap_or(f32::NAN);
            let mut values = [f0, s1.focal, s2.focal, f3, focal];
            distortion_param_scales(&mut values, model_kind, i);

            let y0 = t_left.map(|t| t[i] * values[0]).unwrap_or(NO_NEIGHBOR);
            let y1 = t0[i] * values[1];
            let y2 = t1[i] * values[2];
            let y3 = t_right.map(|t| t[i] * values[3]).unwrap_or(NO_NEIGHBOR);

            out_terms[i] = catmull_rom_interpolate(y0, y1, y2, y3, t) / values[4];
        }

        Some(CalibDistortion {
            focal,
            model: distortion_from_terms(model_kind, out_terms),
            real_focal,
        })
    }

    /// Interpolate a TCA calibration at the requested `focal` length.
    ///
    /// Returns `None` if the lens has no usable TCA calibrations.
    // Port of `lfLens::InterpolateTCA` in lens.cpp:1007-1099.
    pub fn interpolate_tca(&self, focal: f32) -> Option<CalibTca> {
        let mut slots: [Option<&CalibTca>; 4] = [None; 4];
        let mut dists: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];
        let mut model_kind: u8 = 0;

        for c in &self.calib_tca {
            let kind = tca_kind(&c.model);
            if kind == 0 {
                continue;
            }
            if model_kind == 0 {
                model_kind = kind;
            } else if model_kind != kind {
                continue;
            }

            let df = focal - c.focal;
            if df == 0.0 {
                return Some(*c);
            }
            insert_spline(&mut slots, &mut dists, df, c);
        }

        if slots[1].is_none() || slots[2].is_none() {
            return slots[1].or(slots[2]).copied();
        }
        let s0 = slots[0];
        let s1 = slots[1].expect("checked above");
        let s2 = slots[2].expect("checked above");
        let s3 = slots[3];

        let t = (focal - s1.focal) / (s2.focal - s1.focal);

        let t1_arr = tca_terms(&s1.model);
        let t2_arr = tca_terms(&s2.model);
        let t_left = s0.map(|c| tca_terms(&c.model));
        let t_right = s3.map(|c| tca_terms(&c.model));

        let mut out_terms = [0.0_f32; TCA_TERM_COUNT];
        for i in 0..TCA_TERM_COUNT {
            let f0 = s0.map(|c| c.focal).unwrap_or(f32::NAN);
            let f3 = s3.map(|c| c.focal).unwrap_or(f32::NAN);
            let mut values = [f0, s1.focal, s2.focal, f3, focal];
            tca_param_scales(&mut values, model_kind, i);

            let y0 = t_left.map(|t| t[i] * values[0]).unwrap_or(NO_NEIGHBOR);
            let y1 = t1_arr[i] * values[1];
            let y2 = t2_arr[i] * values[2];
            let y3 = t_right.map(|t| t[i] * values[3]).unwrap_or(NO_NEIGHBOR);

            out_terms[i] = catmull_rom_interpolate(y0, y1, y2, y3, t) / values[4];
        }

        Some(CalibTca {
            focal,
            model: tca_from_terms(model_kind, out_terms),
        })
    }

    /// Interpolate a vignetting calibration at the requested
    /// `(focal, aperture, distance)`.
    ///
    /// Uses inverse-distance-weighting (`p = 3.5`) over the calibration grid,
    /// after rescaling the axes (focal: linear-normalized to lens range;
    /// aperture/distance: reciprocal). Returns `None` if no calibration entry
    /// is closer than 1 unit in the rescaled space.
    // Port of `lfLens::InterpolateVignetting` in lens.cpp:1122-1207.
    pub fn interpolate_vignetting(
        &self,
        focal: f32,
        aperture: f32,
        distance: f32,
    ) -> Option<CalibVignetting> {
        let mut model_kind: u8 = 0;
        let mut accum = [0.0_f32; VIGNETTING_TERM_COUNT];
        let mut total_weight = 0.0_f32;
        let mut smallest = f32::MAX;
        let power = 3.5_f32;

        for c in &self.calib_vignetting {
            let kind = vignetting_kind(&c.model);
            if kind == 0 {
                // Upstream sets res.Model from the first encountered entry
                // unconditionally; for `None` the entry contributes nothing,
                // so skipping is harmless.
                continue;
            }
            if model_kind == 0 {
                model_kind = kind;
            } else if model_kind != kind {
                continue;
            }

            let id = vignetting_dist(self.focal_min, self.focal_max, c, focal, aperture, distance);
            if id < 0.0001 {
                // Exact-enough match: return this sample with the query keys.
                return Some(CalibVignetting {
                    focal,
                    aperture,
                    distance,
                    model: c.model,
                });
            }

            if id < smallest {
                smallest = id;
            }
            let weight = (1.0 / id.powf(power)).abs();
            let terms = vignetting_terms(&c.model);
            for i in 0..VIGNETTING_TERM_COUNT {
                let mut values = [c.focal];
                vignetting_param_scales(&mut values, model_kind, i);
                accum[i] += weight * terms[i] * values[0];
            }
            total_weight += weight;
        }

        if smallest > 1.0 {
            return None;
        }
        if total_weight <= 0.0 || smallest >= f32::MAX {
            return None;
        }

        let mut out = [0.0_f32; VIGNETTING_TERM_COUNT];
        for i in 0..VIGNETTING_TERM_COUNT {
            let mut values = [focal];
            vignetting_param_scales(&mut values, model_kind, i);
            out[i] = accum[i] / (total_weight * values[0]);
        }

        Some(CalibVignetting {
            focal,
            aperture,
            distance,
            model: vignetting_from_terms(model_kind, out),
        })
    }
}

// -----------------------------// GuessParameters //-----------------------------//
//
// Port of `lfLens::GuessParameters` (lens.cpp:171). Three regexes against the
// model-name string, plus a fallback that scans calibration entries for focal /
// aperture range. The Rust version handles only the regex pass; the calibration
// fallback works against the flat `calib_*` Vecs.
//
// Upstream uses POSIX-style `regex_match` which requires the whole string to
// match. We anchor with `^…$` to get the same semantics.

// Regex 0: `<minf>-<maxf>mm <ap-prefix>?<mina>(-<maxa>)?`
//   group 1 = minf, group 2 = maxf, group 5 = mina.
// Port of lens.cpp:154.
static LENS_NAME_RE_0: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^:]*?([0-9]+[0-9.]*)[-]?([0-9]+[0-9.]*)?(mm)[[:space:]]+(f/|f|1/|1:)?([0-9.]+)(-[0-9.]+)?.*$")
        .expect("regex 0 compiles")
});

// Regex 1: `1:<mina>-<maxa> <minf>-<maxf>mm?`
//   group 3 = minf, group 4 = maxf, group 1 = mina.
// Port of lens.cpp:157.
static LENS_NAME_RE_1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^.*?1:([0-9.]+)[-]?([0-9.]+)?[[:space:]]+([0-9.]+)[-]?([0-9.]+)?(mm)?.*$")
        .expect("regex 1 compiles")
});

// Regex 2: `<mina>-<maxa>/<minf>-<maxf>`
//   group 3 = minf, group 4 = maxf, group 1 = mina.
// Port of lens.cpp:160.
static LENS_NAME_RE_2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^.*?([0-9.]+)[-]?([0-9.]+)?\s*/\s*([0-9.]+)[-]?([0-9.]+)?.*$")
        .expect("regex 2 compiles")
});

// `<digit>(.<digit+>)?x` — any model that looks like a teleconverter is skipped.
// Port of lens.cpp:169 (verbatim, including the upstream `[0.9]` typo — port,
// don't fix).
static EXTENDER_MAGNIFICATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?[0-9](\.[0.9]+)?x.*$").expect("extender regex compiles"));

/// Per-regex `(minf, maxf, mina)` capture-group indices.
// Port of lens.cpp:163's `lens_name_matches` table.
const LENS_NAME_MATCHES: [[usize; 3]; 3] = [[1, 2, 5], [3, 4, 1], [3, 4, 1]];

impl Lens {
    /// Fill in `focal_min`, `focal_max`, `aperture_min`, `aperture_max` from the
    /// model name (and, as a fallback, from the calibration sample focal / aperture
    /// values).
    ///
    /// Idempotent for already-set fields: only zero entries are overwritten.
    // Port of lfLens::GuessParameters at lens.cpp:171.
    pub fn guess_parameters(&mut self) {
        let mut minf = f32::MAX;
        let mut maxf = f32::MIN;
        let mut mina = f32::MAX;
        let maxa = f32::MIN; // upstream tracks but never assigns from regex

        let model = &self.model;
        let model_lower = model.to_ascii_lowercase();

        let skip_extender = model_lower.contains("adapter")
            || model_lower.contains("reducer")
            || model_lower.contains("booster")
            || model_lower.contains("extender")
            || model_lower.contains("converter")
            || model_lower.contains("magnifier")
            || EXTENDER_MAGNIFICATION_RE.is_match(model);

        if !model.is_empty()
            && (self.aperture_min == 0.0 || self.focal_min == 0.0)
            && !skip_extender
        {
            let regexes: [&Regex; 3] = [&LENS_NAME_RE_0, &LENS_NAME_RE_1, &LENS_NAME_RE_2];
            for (i, re) in regexes.iter().enumerate() {
                if let Some(caps) = re.captures(model) {
                    let idx = LENS_NAME_MATCHES[i];

                    if let Some(m) = caps.get(idx[0]) {
                        if let Ok(v) = m.as_str().parse::<f32>() {
                            minf = v;
                        }
                    }
                    if let Some(m) = caps.get(idx[1]) {
                        if let Ok(v) = m.as_str().parse::<f32>() {
                            maxf = v;
                        }
                    }
                    if let Some(m) = caps.get(idx[2]) {
                        if let Ok(v) = m.as_str().parse::<f32>() {
                            mina = v;
                        }
                    }
                    break;
                }
            }
        }

        // Fallback: scan calibration samples for focal / aperture range.
        if self.aperture_min == 0.0 || self.focal_min == 0.0 {
            for c in &self.calib_distortion {
                if c.focal < minf {
                    minf = c.focal;
                }
                if c.focal > maxf {
                    maxf = c.focal;
                }
            }
            for c in &self.calib_tca {
                if c.focal < minf {
                    minf = c.focal;
                }
                if c.focal > maxf {
                    maxf = c.focal;
                }
            }
            for c in &self.calib_vignetting {
                if c.focal < minf {
                    minf = c.focal;
                }
                if c.focal > maxf {
                    maxf = c.focal;
                }
                if c.aperture < mina {
                    mina = c.aperture;
                }
                // Upstream tracks maxa here too, but never writes it.
            }
        }

        if minf != f32::MAX && self.focal_min == 0.0 {
            self.focal_min = minf;
        }
        if maxf != f32::MIN && self.focal_max == 0.0 {
            self.focal_max = maxf;
        }
        if mina != f32::MAX && self.aperture_min == 0.0 {
            self.aperture_min = mina;
        }
        if maxa != f32::MIN && self.aperture_max == 0.0 {
            self.aperture_max = maxa;
        }

        if self.focal_max == 0.0 {
            self.focal_max = self.focal_min;
        }
    }
}
