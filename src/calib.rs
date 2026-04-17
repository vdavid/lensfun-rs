//! Calibration entries: distortion, transverse chromatic aberration, vignetting.
//!
//! These mirror the `lfLensCalibDistortion`, `lfLensCalibTCA`, and `lfLensCalibVignetting`
//! types in upstream `include/lensfun/lensfun.h.in`. They are plain data; the math that
//! consumes them lives in `mod_coord`, `mod_subpix`, and `mod_color`.

/// Distortion model used by a calibration entry.
///
/// Three models are supported by upstream:
///
/// - `Ptlens` — 4th-order radial polynomial (closed-form inverse).
/// - `Poly3` — `Rd = (1 - k1) · Ru + k1 · Ru³` (Newton-iteration inverse).
/// - `Poly5` — 5th-order radial polynomial (Newton-iteration inverse).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistortionModel {
    /// No distortion correction.
    None,
    /// `Rd = (1 - k1) · Ru + k1 · Ru³`. One coefficient.
    Poly3 {
        /// `k1` coefficient.
        k1: f32,
    },
    /// 5th-order radial polynomial. Two coefficients.
    Poly5 {
        /// `k1` coefficient.
        k1: f32,
        /// `k2` coefficient.
        k2: f32,
    },
    /// Pierre Toscani's 4th-order ("ptlens") model. Three coefficients.
    Ptlens {
        /// `a` coefficient.
        a: f32,
        /// `b` coefficient.
        b: f32,
        /// `c` coefficient.
        c: f32,
    },
}

/// Distortion calibration entry — one per `(focal length)` sample in the database.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibDistortion {
    /// Focal length in millimeters at which this calibration was measured.
    pub focal: f32,
    /// Which model and its coefficients.
    pub model: DistortionModel,
    /// Real focal length of the lens at the measurement focal, used to compensate
    /// the marketed focal-length value when known. Optional.
    pub real_focal: Option<f32>,
}

/// Transverse chromatic aberration model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TcaModel {
    /// No TCA correction.
    None,
    /// Pure radial scale per channel: red and blue scaled relative to green.
    Linear {
        /// Red channel scale.
        kr: f32,
        /// Blue channel scale.
        kb: f32,
    },
    /// Cubic radial polynomial per channel: `r' = r · (a + b · r + c · r²)`.
    Poly3 {
        /// Red channel coefficients `(a, b, c)`.
        red: [f32; 3],
        /// Blue channel coefficients `(a, b, c)`.
        blue: [f32; 3],
    },
}

/// TCA calibration entry — one per focal-length sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibTca {
    /// Focal length in millimeters at which this calibration was measured.
    pub focal: f32,
    /// Which model and its coefficients.
    pub model: TcaModel,
}

/// Vignetting model. Upstream supports only the `pa` model in v1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VignettingModel {
    /// No vignetting correction.
    None,
    /// `gain = 1 + k1·r² + k2·r⁴ + k3·r⁶`.
    Pa {
        /// `k1` coefficient.
        k1: f32,
        /// `k2` coefficient.
        k2: f32,
        /// `k3` coefficient.
        k3: f32,
    },
}

/// Vignetting calibration entry — one per `(focal, aperture, distance)` triple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibVignetting {
    /// Focal length in millimeters.
    pub focal: f32,
    /// Aperture (f-number).
    pub aperture: f32,
    /// Subject distance in meters.
    pub distance: f32,
    /// Which model and its coefficients.
    pub model: VignettingModel,
}
