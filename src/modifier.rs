//! Modifier — composition of correction passes.
//!
//! Port of `libs/lensfun/modifier.cpp` (constructor + `Apply*` methods),
//! `mod-coord.cpp` (distortion enable / `ApplyGeometryDistortion`),
//! `mod-subpix.cpp` (TCA enable / `ApplySubpixelDistortion`),
//! and `mod-color.cpp` (vignetting enable / `ApplyColorModification`).
//!
//! The `Modifier` owns image dimensions + per-shot focal/aperture/distance, runs
//! `Lens::interpolate_*` to get coefficients, sets up the pixel ↔ normalized-coord
//! mapping, and exposes `apply_*` methods that walk image buffers and call the
//! pure kernels in [`crate::mod_coord`], [`crate::mod_subpix`], [`crate::mod_color`].

use crate::calib::{
    CalibDistortion, CalibTca, CalibVignetting, DistortionModel, TcaModel, VignettingModel,
};
use crate::lens::Lens;
use crate::mod_coord::{
    dist_poly3, dist_poly5, dist_ptlens, undist_poly3, undist_poly5, undist_ptlens,
};
use crate::mod_subpix::{tca_linear, tca_poly3_forward, tca_poly3_reverse};

/// Distortion pass state — interpolated coefficients post-rescaling, plus
/// forward/reverse direction.
#[derive(Debug, Clone, Copy)]
struct DistortionPass {
    model: DistortionModel,
    reverse: bool,
}

/// TCA pass state — interpolated coefficients post-rescaling, plus
/// forward/reverse direction.
#[derive(Debug, Clone, Copy)]
struct TcaPass {
    model: TcaModel,
    reverse: bool,
}

/// Vignetting pass state — interpolated coefficients post-rescaling, plus
/// forward/reverse direction.
#[derive(Debug, Clone, Copy)]
struct VignettingPass {
    model: VignettingModel,
    reverse: bool,
}

/// A correction modifier configured for a specific shot.
///
/// Built from a [`Lens`] plus shooting parameters. Holds the interpolated
/// coefficients for distortion, TCA, and vignetting, ready to apply to pixel
/// buffers via the `apply_*` methods.
///
/// Mirrors upstream `lfModifier` — see `libs/lensfun/modifier.cpp`. The
/// `reverse` flag follows upstream: `false` simulates the lens distortion
/// (forward), `true` corrects an already-distorted image.
#[derive(Debug, Clone)]
pub struct Modifier {
    // Stored per-shot focal — used by `enable_*` to drive `Lens::interpolate_*`.
    focal: f32,
    reverse: bool,
    // Real focal length (defaults to nominal `focal` if no calibration says otherwise).
    real_focal: f64,
    // Pixel ↔ normalized-coordinate mapping.
    norm_scale: f64,
    norm_unscale: f64,
    center_x: f64,
    center_y: f64,
    // Per-pass coefficients, populated by `enable_*`.
    distortion: Option<DistortionPass>,
    tca: Option<TcaPass>,
    vignetting: Option<VignettingPass>,
}

impl Modifier {
    /// Build a modifier for a `lens` + shot.
    ///
    /// `crop` is the *image's* crop factor (i.e. the camera body the photo was
    /// taken with — may differ from the lens's calibration crop).
    /// `image_width` / `image_height` are the actual pixel dimensions. `reverse`
    /// matches upstream: `false` simulates the distortion (forward), `true`
    /// corrects it.
    // Port of modifier.cpp:61-88.
    pub fn new(
        lens: &Lens,
        focal: f32,
        crop: f32,
        image_width: u32,
        image_height: u32,
        reverse: bool,
    ) -> Self {
        // Upstream: `Width = imgwidth >= 2 ? imgwidth - 1 : 1` (avoid /0 on singular).
        let width = if image_width >= 2 {
            (image_width - 1) as f64
        } else {
            1.0
        };
        let height = if image_height >= 2 {
            (image_height - 1) as f64
        } else {
            1.0
        };

        // Try to extract RealFocal from the distortion calibration; fall back to nominal.
        let real_focal = match lens.interpolate_distortion(focal) {
            Some(lcd) => lcd.real_focal.map(|f| f as f64).unwrap_or(focal as f64),
            None => focal as f64,
        };

        // NormScale = hypot(36, 24) / Crop / hypot(Width + 1, Height + 1) / RealFocal.
        let norm_scale =
            (36.0_f64.hypot(24.0)) / crop as f64 / ((width + 1.0).hypot(height + 1.0)) / real_focal;
        let norm_unscale = 1.0 / norm_scale;

        // CenterX = (Width / 2 + size / 2 * Lens.CenterX) * NormScale.
        let size = width.min(height);
        let center_x = (width / 2.0 + size / 2.0 * lens.center_x as f64) * norm_scale;
        let center_y = (height / 2.0 + size / 2.0 * lens.center_y as f64) * norm_scale;

        // `width` / `height` (the f64 versions) and `crop` are used only to
        // derive the normalization above — drop them rather than carry dead state.
        let _ = (width, height, crop);

        Self {
            focal,
            reverse,
            real_focal,
            norm_scale,
            norm_unscale,
            center_x,
            center_y,
            distortion: None,
            tca: None,
            vignetting: None,
        }
    }

    /// Enable distortion correction by interpolating `lens` calibration at
    /// the constructor's focal length. Returns `true` if a usable calibration
    /// was found.
    // Port of mod-coord.cpp:73-158.
    pub fn enable_distortion_correction(&mut self, lens: &Lens) -> bool {
        let Some(lcd) = lens.interpolate_distortion(self.focal) else {
            return false;
        };
        // Use the lens calibration's crop/aspect (mirrors upstream's
        // `lcd.CalibAttr.{CropFactor,AspectRatio}`), NOT the image's crop.
        let model = rescale_distortion(&lcd, lens.aspect_ratio, lens.crop_factor, self.real_focal);
        if matches!(model, DistortionModel::None) {
            return false;
        }
        // Upstream skips poly3 forward+reverse if `k1 == 0`. For other models,
        // it always proceeds.
        if let DistortionModel::Poly3 { k1 } = model {
            if k1 == 0.0 {
                return false;
            }
        }
        self.distortion = Some(DistortionPass {
            model,
            reverse: self.reverse,
        });
        true
    }

    /// Enable TCA correction by interpolating `lens` calibration. Returns
    /// `true` if a usable calibration was found.
    // Port of mod-subpix.cpp:42-108.
    pub fn enable_tca_correction(&mut self, lens: &Lens) -> bool {
        let Some(lctca) = lens.interpolate_tca(self.focal) else {
            return false;
        };
        let model = rescale_tca(
            &lctca,
            lens.aspect_ratio,
            lens.crop_factor,
            self.real_focal,
            self.reverse,
        );
        if matches!(model, TcaModel::None) {
            return false;
        }
        self.tca = Some(TcaPass {
            model,
            reverse: self.reverse,
        });
        true
    }

    /// Enable vignetting correction by interpolating `lens` calibration at
    /// `aperture` / `distance`. Returns `true` if a usable calibration was
    /// found.
    // Port of mod-color.cpp:36-152.
    pub fn enable_vignetting_correction(
        &mut self,
        lens: &Lens,
        aperture: f32,
        distance: f32,
    ) -> bool {
        let Some(lcv) = lens.interpolate_vignetting(self.focal, aperture, distance) else {
            return false;
        };
        let model = rescale_vignetting(&lcv, lens.crop_factor, self.real_focal);
        if matches!(model, VignettingModel::None) {
            return false;
        }
        self.vignetting = Some(VignettingPass {
            model,
            reverse: self.reverse,
        });
        true
    }

    /// Apply geometry + distortion to coordinates. Mirrors upstream
    /// `lfModifier::ApplyGeometryDistortion`.
    ///
    /// `coords` is a `[x0, y0, x1, y1, ...]` buffer of length `2 * width * rows`,
    /// covering a `width × rows` rectangle whose top-left pixel sits at
    /// `(x_start, y_start)`. Returns `true` if any callback ran.
    // Port of mod-coord.cpp:514-547.
    pub fn apply_geometry_distortion(
        &self,
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        coords: &mut [f32],
    ) -> bool {
        let Some(pass) = self.distortion else {
            return false;
        };
        if rows == 0 || width == 0 {
            return false;
        }
        debug_assert_eq!(coords.len(), 2 * width * rows);

        // Convert pixel start to normalized.
        let xu = x_start as f64 * self.norm_scale - self.center_x;
        let yu = y_start as f64 * self.norm_scale - self.center_y;
        let ns = self.norm_scale;

        for row in 0..rows {
            let y = (yu + ns * row as f64) as f32;
            let row_off = row * width * 2;

            // Fill row with normalized coords.
            for i in 0..width {
                let x = (xu + ns * i as f64) as f32;
                coords[row_off + 2 * i] = x;
                coords[row_off + 2 * i + 1] = y;
            }

            // Apply distortion in place.
            apply_distortion_kernel(&pass, &mut coords[row_off..row_off + 2 * width]);

            // Convert back to pixel coords.
            for i in 0..width {
                let cx = coords[row_off + 2 * i] as f64;
                let cy = coords[row_off + 2 * i + 1] as f64;
                coords[row_off + 2 * i] = ((cx + self.center_x) * self.norm_unscale) as f32;
                coords[row_off + 2 * i + 1] = ((cy + self.center_y) * self.norm_unscale) as f32;
            }
        }

        true
    }

    /// Apply per-channel TCA shifts. Mirrors `lfModifier::ApplySubpixelDistortion`.
    ///
    /// `coords` is laid out as `[xR, yR, xG, yG, xB, yB, ...]` per pixel
    /// (length `6 * width * rows`).
    // Port of mod-subpix.cpp:122-157.
    pub fn apply_subpixel_distortion(
        &self,
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        coords: &mut [f32],
    ) -> bool {
        let Some(pass) = self.tca else {
            return false;
        };
        if rows == 0 || width == 0 {
            return false;
        }
        debug_assert_eq!(coords.len(), 6 * width * rows);

        let xu = x_start as f64 * self.norm_scale - self.center_x;
        let yu = y_start as f64 * self.norm_scale - self.center_y;
        let ns = self.norm_scale;

        for row in 0..rows {
            let y = (yu + ns * row as f64) as f32;
            let row_off = row * width * 6;

            for i in 0..width {
                let x = (xu + ns * i as f64) as f32;
                let off = row_off + 6 * i;
                coords[off] = x;
                coords[off + 1] = y;
                coords[off + 2] = x;
                coords[off + 3] = y;
                coords[off + 4] = x;
                coords[off + 5] = y;
            }

            apply_tca_kernel(&pass, &mut coords[row_off..row_off + 6 * width]);

            // Convert all 3 channels back to pixel coords.
            for i in 0..3 * width {
                let off = row_off + 2 * i;
                let cx = coords[off] as f64;
                let cy = coords[off + 1] as f64;
                coords[off] = ((cx + self.center_x) * self.norm_unscale) as f32;
                coords[off + 1] = ((cy + self.center_y) * self.norm_unscale) as f32;
            }
        }

        true
    }

    /// Apply vignetting (color modification) on `f32` pixels.
    ///
    /// `pixels` is interleaved row-major (`width * rows * channels` elements),
    /// covering a `width × rows` rectangle whose top-left pixel sits at
    /// `(x_start, y_start)`.
    // Port of mod-color.cpp:167-184 (templated on T = lf_f32).
    pub fn apply_color_modification_f32(
        &self,
        pixels: &mut [f32],
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        channels: usize,
    ) -> bool {
        self.apply_color_modification(
            pixels,
            x_start,
            y_start,
            width,
            rows,
            channels,
            |chunk, c| {
                for p in chunk {
                    *p *= c;
                }
            },
        )
    }

    /// Apply vignetting (color modification) on `u16` pixels.
    ///
    /// Mirrors upstream's 22.10 fixed-point per-pixel multiply
    /// (mod-color.cpp:263-296).
    pub fn apply_color_modification_u16(
        &self,
        pixels: &mut [u16],
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        channels: usize,
    ) -> bool {
        self.apply_color_modification(
            pixels,
            x_start,
            y_start,
            width,
            rows,
            channels,
            |chunk, c| {
                // 22.10 fixed-point with saturation at (31 << 10).
                let mut c10 = (c as f64 * 1024.0) as i32;
                if c10 > (31 << 10) {
                    c10 = 31 << 10;
                }
                for p in chunk {
                    let r = (i32::from(*p) * c10 + 512) >> 10;
                    *p = clamp_bits_16(r);
                }
            },
        )
    }

    /// Apply vignetting (color modification) on `u8` pixels.
    ///
    /// Mirrors upstream's 20.12 fixed-point per-pixel multiply
    /// (mod-color.cpp:227-260).
    pub fn apply_color_modification_u8(
        &self,
        pixels: &mut [u8],
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        channels: usize,
    ) -> bool {
        self.apply_color_modification(
            pixels,
            x_start,
            y_start,
            width,
            rows,
            channels,
            |chunk, c| {
                // 20.12 fixed-point with saturation at (2047 << 12).
                let mut c12 = (c as f64 * 4096.0) as i32;
                if c12 > (2047 << 12) {
                    c12 = 2047 << 12;
                }
                for p in chunk {
                    let r = (i32::from(*p) * c12 + 2048) >> 12;
                    *p = clamp_bits_8(r);
                }
            },
        )
    }

    /// Internal walker — mirrors `ApplyColorModification` plus the per-pixel
    /// `ModifyColor_Vignetting_PA` / `ModifyColor_DeVignetting_PA` body
    /// (mod-color.cpp:298-358). The `op` closure receives the per-pixel
    /// channel slice plus the gain multiplier (`f32`).
    fn apply_color_modification<T>(
        &self,
        pixels: &mut [T],
        x_start: f32,
        y_start: f32,
        width: usize,
        rows: usize,
        channels: usize,
        mut op: impl FnMut(&mut [T], f32),
    ) -> bool {
        let Some(pass) = self.vignetting else {
            return false;
        };
        if rows == 0 || width == 0 || channels == 0 {
            return false;
        }
        debug_assert_eq!(pixels.len(), width * rows * channels);

        let VignettingModel::Pa { k1, k2, k3 } = pass.model else {
            return false;
        };

        let ns = self.norm_scale as f32;
        let d1 = 2.0_f32 * ns;
        let d2 = ns * ns;

        // Same x/y starting math as the geom/subpix passes.
        let xu = (x_start as f64 * self.norm_scale - self.center_x) as f32;
        let yu = (y_start as f64 * self.norm_scale - self.center_y) as f32;

        for row in 0..rows {
            let y = yu + ns * row as f32;
            let mut x = xu;
            let mut r2 = x * x + y * y;
            let row_off = row * width * channels;

            for col in 0..width {
                let r4 = r2 * r2;
                let r6 = r4 * r2;
                // Don't refactor into Horner form — upstream keeps the explicit
                // `1 + k1·r² + k2·r⁴ + k3·r⁶` order; bit-exact floats matter.
                let gain = 1.0 + k1 * r2 + k2 * r4 + k3 * r6;
                // Upstream maps reverse=false → DeVignetting (apply 1/gain to
                // correct existing darkening), reverse=true → Vignetting (apply
                // gain to simulate). Opposite of the distortion-pass convention.
                // See mod-color.cpp:36-152.
                let mult = if pass.reverse { gain } else { 1.0 / gain };

                let pix_off = row_off + col * channels;
                op(&mut pixels[pix_off..pix_off + channels], mult);

                // Incremental r² update (mod-color.cpp:324).
                r2 += d1 * x + d2;
                x += ns;
            }
        }

        true
    }
}

// -------------- coefficient rescaling (rescale_polynomial_coefficients) --------------

/// Port of `rescale_polynomial_coefficients` for distortion (mod-coord.cpp:37-71).
fn rescale_distortion(
    lcd: &CalibDistortion,
    aspect_ratio: f32,
    crop: f32,
    real_focal: f64,
) -> DistortionModel {
    let hugin_scale_in_mm =
        (36.0_f64).hypot(24.0) / crop as f64 / (aspect_ratio as f64).hypot(1.0) / 2.0;
    let hugin_scaling = (real_focal / hugin_scale_in_mm) as f32;
    // Upstream evaluates `*= pow(hugin_scaling, N) / pow(d, M)` in double then
    // stores back to float — mirror that via `f64`.
    let hs = hugin_scaling as f64;
    match lcd.model {
        DistortionModel::None => DistortionModel::None,
        DistortionModel::Poly3 { k1 } => {
            let d = 1.0_f64 - k1 as f64;
            DistortionModel::Poly3 {
                k1: (k1 as f64 * hs.powi(2) / d.powi(3)) as f32,
            }
        }
        DistortionModel::Poly5 { k1, k2 } => DistortionModel::Poly5 {
            k1: (k1 as f64 * hs.powi(2)) as f32,
            k2: (k2 as f64 * hs.powi(4)) as f32,
        },
        DistortionModel::Ptlens { a, b, c } => {
            let d = 1.0_f64 - a as f64 - b as f64 - c as f64;
            DistortionModel::Ptlens {
                a: (a as f64 * hs.powi(3) / d.powi(4)) as f32,
                b: (b as f64 * hs.powi(2) / d.powi(3)) as f32,
                c: (c as f64 * hs / d.powi(2)) as f32,
            }
        }
    }
}

/// Port of `rescale_polynomial_coefficients` for TCA (mod-subpix.cpp:13-40).
fn rescale_tca(
    lctca: &CalibTca,
    aspect_ratio: f32,
    crop: f32,
    real_focal: f64,
    reverse: bool,
) -> TcaModel {
    let hugin_scale_in_mm =
        (36.0_f64).hypot(24.0) / crop as f64 / (aspect_ratio as f64).hypot(1.0) / 2.0;
    let hugin_scaling = (real_focal / hugin_scale_in_mm) as f32;
    match lctca.model {
        TcaModel::None => TcaModel::None,
        TcaModel::Linear { kr, kb } => {
            if reverse {
                TcaModel::Linear {
                    kr: 1.0 / kr,
                    kb: 1.0 / kb,
                }
            } else {
                TcaModel::Linear { kr, kb }
            }
        }
        TcaModel::Poly3 { red, blue } => {
            // Terms layout in upstream: [vr, vb, cr, cb, br, bb]. Index 0/1 (the
            // "v" near-1 terms) stay; index 2/3 multiply by hugin_scaling; index
            // 4/5 by hugin_scaling². Map back to per-channel [v, c, b].
            // Upstream `*= pow(hugin_scaling, N)` evaluates in double, so we do too.
            let hs = hugin_scaling as f64;
            TcaModel::Poly3 {
                red: [
                    red[0],
                    (red[1] as f64 * hs) as f32,
                    (red[2] as f64 * hs.powi(2)) as f32,
                ],
                blue: [
                    blue[0],
                    (blue[1] as f64 * hs) as f32,
                    (blue[2] as f64 * hs.powi(2)) as f32,
                ],
            }
        }
    }
}

/// Port of `rescale_polynomial_coefficients` for vignetting (mod-color.cpp:13-34).
///
/// Upstream does `*= pow(hugin_scaling, N)` which evaluates in `double` and
/// stores back to `float`. We mirror that by promoting via `f64` for the
/// multiply step.
fn rescale_vignetting(lcv: &CalibVignetting, crop: f32, real_focal: f64) -> VignettingModel {
    let hugin_scale_in_mm = (36.0_f64).hypot(24.0) / crop as f64 / 2.0;
    let hugin_scaling = (real_focal / hugin_scale_in_mm) as f32;
    let hs = hugin_scaling as f64;
    match lcv.model {
        VignettingModel::None => VignettingModel::None,
        VignettingModel::Pa { k1, k2, k3 } => VignettingModel::Pa {
            k1: (k1 as f64 * hs.powi(2)) as f32,
            k2: (k2 as f64 * hs.powi(4)) as f32,
            k3: (k3 as f64 * hs.powi(6)) as f32,
        },
    }
}

// -------------- per-row kernel dispatchers --------------

/// Dispatch a row of `[x0, y0, x1, y1, ...]` through the active distortion kernel.
fn apply_distortion_kernel(pass: &DistortionPass, row: &mut [f32]) {
    let n = row.len() / 2;
    match (pass.model, pass.reverse) {
        (DistortionModel::Poly3 { k1 }, false) => {
            for i in 0..n {
                let (xo, yo) = dist_poly3(row[2 * i], row[2 * i + 1], k1);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::Poly3 { k1 }, true) => {
            for i in 0..n {
                let (xo, yo) = undist_poly3(row[2 * i], row[2 * i + 1], k1);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::Poly5 { k1, k2 }, false) => {
            for i in 0..n {
                let (xo, yo) = dist_poly5(row[2 * i], row[2 * i + 1], k1, k2);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::Poly5 { k1, k2 }, true) => {
            for i in 0..n {
                let (xo, yo) = undist_poly5(row[2 * i], row[2 * i + 1], k1, k2);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::Ptlens { a, b, c }, false) => {
            for i in 0..n {
                let (xo, yo) = dist_ptlens(row[2 * i], row[2 * i + 1], a, b, c);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::Ptlens { a, b, c }, true) => {
            for i in 0..n {
                let (xo, yo) = undist_ptlens(row[2 * i], row[2 * i + 1], a, b, c);
                row[2 * i] = xo;
                row[2 * i + 1] = yo;
            }
        }
        (DistortionModel::None, _) => {}
    }
}

/// Dispatch a row of `[xR, yR, xG, yG, xB, yB, ...]` through the active TCA kernel.
fn apply_tca_kernel(pass: &TcaPass, row: &mut [f32]) {
    let n = row.len() / 6;
    match pass.model {
        TcaModel::None => {}
        TcaModel::Linear { kr, kb } => {
            // tca_linear handles both forward and reverse — the reverse case
            // already inverted kr/kb in `rescale_tca`.
            for i in 0..n {
                let off = 6 * i;
                let x = row[off];
                let y = row[off + 1];
                let (xr, yr, xb, yb) = tca_linear(x, y, kr, kb);
                row[off] = xr;
                row[off + 1] = yr;
                // Green stays at (x, y) — already filled by the caller.
                row[off + 4] = xb;
                row[off + 5] = yb;
            }
        }
        TcaModel::Poly3 { red, blue } => {
            for i in 0..n {
                let off = 6 * i;
                let x = row[off];
                let y = row[off + 1];
                let (xr, yr, xb, yb) = if pass.reverse {
                    tca_poly3_reverse(x, y, red, blue)
                } else {
                    tca_poly3_forward(x, y, red, blue)
                };
                row[off] = xr;
                row[off + 1] = yr;
                // Green stays.
                row[off + 4] = xb;
                row[off + 5] = yb;
            }
        }
    }
}

// -------------- u8 / u16 clamp helpers (match mod_color::{clamp_u8, clamp_u16}) --------------

/// Port of upstream `clampbits` (mod-color.cpp:223). Saturates `x` to `n` bits.
///
/// `clampbits(x, n) = if (x >> n) != 0 { ~(x >> n) >> (32 - n) } else { x }`
/// — both wraps negative to 0 and clamps overflow to the max of `n` bits.
fn clamp_bits_8(x: i32) -> u8 {
    let shifted = (x as u32) >> 8;
    if shifted != 0 {
        ((!shifted) >> 24) as u8
    } else {
        x as u8
    }
}

fn clamp_bits_16(x: i32) -> u16 {
    let shifted = (x as u32) >> 16;
    if shifted != 0 {
        ((!shifted) >> 16) as u16
    } else {
        x as u16
    }
}
