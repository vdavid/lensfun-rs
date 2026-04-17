//! Lens type and 4D calibration interpolation.
//!
//! Port of `libs/lensfun/lens.cpp` and the `lfLens` type in
//! `include/lensfun/lensfun.h.in`.

use std::collections::BTreeMap;

use crate::calib::{CalibDistortion, CalibTca, CalibVignetting};

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
    /// Distortion calibration samples.
    pub calib_distortion: Vec<CalibDistortion>,
    /// TCA calibration samples.
    pub calib_tca: Vec<CalibTca>,
    /// Vignetting calibration samples.
    pub calib_vignetting: Vec<CalibVignetting>,
    /// Match score for query results.
    pub score: i32,
}
