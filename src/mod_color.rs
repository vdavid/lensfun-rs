//! Color pass: vignetting correction.
//!
//! Port of `libs/lensfun/mod-color.cpp`. The math is the polynomial-in-`r²`
//! gain
//!
//! ```text
//!     gain = 1 + k1·r² + k2·r⁴ + k3·r⁶
//! ```
//!
//! applied per pixel. Two perspectives:
//!
//! * `vignetting_pa_apply_*` — forward (`Modify`): multiplies a clean image by `gain` to
//!   simulate vignetting. Mirrors `lfModifier::ModifyColor_Vignetting_PA`.
//! * `vignetting_pa_correct_*` — reverse (`UnModify`): multiplies an image by `1/gain` to
//!   correct existing vignetting. Mirrors `lfModifier::ModifyColor_DeVignetting_PA`.
//!
//! Three element types are exposed (`u8`, `u16`, `f32`) to match upstream's templated
//! `lf_u8`/`lf_u16`/`lf_f32` instantiations. The gain math always runs in `f32`, matching
//! upstream's `float c = 1.0 + ...`. Integer outputs are clamped and rounded the same way
//! upstream's `apply_multiplier` does (round-half-up via `+ 0.5` then truncate; saturating
//! at the type max).
//!
//! # Coordinate system
//!
//! `r` is the radial coordinate normalized so that `r = 1` is the unit circle inscribed in
//! the longer side of the image — i.e., distance from the image center to the midpoint of
//! the longer edge. The center pixel has `r = 0`. This is the same convention used by the
//! Hugin polynomial coefficients these kernels consume.
//!
//! Note: upstream's per-row callback receives normalized `(x, y)` from
//! [`lfModifier::ApplyColorModification`] and steps along `x` using `NormScale`. Here we
//! roll the whole-image walk into one function. The arithmetic per pixel is unchanged.
//!
//! # Pixel layout
//!
//! `pixels` is interleaved row-major: `width * height * channels` elements with channels
//! tightly packed (`RGBRGBRGB…` for `channels = 3`, `RGBARGBA…` for `channels = 4`). All
//! channels of a pixel get the same gain, matching upstream when every component role is
//! a real channel (`LF_CR_RED`, etc.). We don't model `LF_CR_UNKNOWN` skip-this-channel
//! semantics — the caller is expected to pass real image data.
//!
//! SSE variants (`mod-color-sse.cpp`, `mod-color-sse2.cpp`) are deferred to a post-v1
//! milestone — see `AGENTS.md`.

/// Forward vignetting on `f32` pixels: multiply by `gain` to darken corners.
// Port of mod-color.cpp:298 (ModifyColor_Vignetting_PA<lf_f32>).
pub fn vignetting_pa_apply_f32(
    pixels: &mut [f32],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| *p *= c);
}

/// Reverse vignetting on `f32` pixels: multiply by `1/gain` to correct darkened corners.
// Port of mod-color.cpp:329 (ModifyColor_DeVignetting_PA<lf_f32>).
pub fn vignetting_pa_correct_f32(
    pixels: &mut [f32],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| {
        *p *= 1.0 / c
    });
}

/// Forward vignetting on `u16` pixels.
// Port of mod-color.cpp:298 (ModifyColor_Vignetting_PA<lf_u16>).
pub fn vignetting_pa_apply_u16(
    pixels: &mut [u16],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| {
        *p = clamp_u16(f32::from(*p) * c);
    });
}

/// Reverse vignetting on `u16` pixels.
// Port of mod-color.cpp:329 (ModifyColor_DeVignetting_PA<lf_u16>).
pub fn vignetting_pa_correct_u16(
    pixels: &mut [u16],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| {
        *p = clamp_u16(f32::from(*p) * (1.0 / c));
    });
}

/// Forward vignetting on `u8` pixels.
// Port of mod-color.cpp:298 (ModifyColor_Vignetting_PA<lf_u8>).
pub fn vignetting_pa_apply_u8(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| {
        *p = clamp_u8(f32::from(*p) * c);
    });
}

/// Reverse vignetting on `u8` pixels.
// Port of mod-color.cpp:329 (ModifyColor_DeVignetting_PA<lf_u8>).
pub fn vignetting_pa_correct_u8(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
) {
    walk(pixels, width, height, channels, k1, k2, k3, |p, c| {
        *p = clamp_u8(f32::from(*p) * (1.0 / c));
    });
}

/// Iterate every pixel, compute gain `c = 1 + k1·r² + k2·r⁴ + k3·r⁶`, and apply `op`
/// to each channel of that pixel.
///
/// We keep upstream's incremental r² update for bit-exact float behavior (mod-color.cpp:309):
/// `r2 += d1·x + d2; x += norm_scale` where `d1 = 2·norm_scale` and `d2 = norm_scale²`.
/// `norm_scale` here is the per-pixel step in normalized coordinates — for our convention
/// (r = 1 at the inscribed unit circle of the longer side) that is `2 / (longer_side - 1)`.
fn walk<T>(
    pixels: &mut [T],
    width: usize,
    height: usize,
    channels: usize,
    k1: f32,
    k2: f32,
    k3: f32,
    mut op: impl FnMut(&mut T, f32),
) {
    if width == 0 || height == 0 || channels == 0 {
        return;
    }
    debug_assert_eq!(
        pixels.len(),
        width * height * channels,
        "buffer length must equal width * height * channels"
    );

    // Match upstream's `Width = imgwidth - 1` convention so the corner pixel sits exactly on
    // the normalization edge. Guard for 1-pixel sides (avoid divide-by-zero).
    let denom = (width.max(height).saturating_sub(1)).max(1) as f32;
    let norm_scale = 2.0_f32 / denom;
    let d1 = 2.0_f32 * norm_scale;
    let d2 = norm_scale * norm_scale;

    let cx = (width.saturating_sub(1)) as f32 * 0.5 * norm_scale;
    let cy = (height.saturating_sub(1)) as f32 * 0.5 * norm_scale;

    let mut y = -cy;
    for row in 0..height {
        let x_start = -cx;
        let mut r2 = x_start * x_start + y * y;
        let mut x = x_start;

        let row_off = row * width * channels;
        for col in 0..width {
            let r4 = r2 * r2;
            let r6 = r4 * r2;
            // Don't refactor into Horner form — upstream keeps the explicit
            // `1 + k1·r² + k2·r⁴ + k3·r⁶` order; bit-exact floats matter.
            let c = 1.0 + k1 * r2 + k2 * r4 + k3 * r6;

            let pix_off = row_off + col * channels;
            for ch in 0..channels {
                op(&mut pixels[pix_off + ch], c);
            }

            // Incremental r² update (mod-color.cpp:324).
            r2 += d1 * x + d2;
            x += norm_scale;
        }

        y += norm_scale;
    }
}

/// Saturating round-half-up to `u8`, matching upstream's `clampbits`/fixed-point round.
fn clamp_u8(v: f32) -> u8 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    let r = (v + 0.5) as i32;
    r.clamp(0, 255) as u8
}

/// Saturating round-half-up to `u16`.
fn clamp_u16(v: f32) -> u16 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    let r = (v + 0.5) as i32;
    r.clamp(0, 65535) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_pixel_gain_is_one_f32() {
        // Odd-sized image: the exact center pixel sits at r = 0 → gain = 1 → unchanged.
        let mut buf = vec![0.5_f32; 5 * 5];
        vignetting_pa_apply_f32(&mut buf, 5, 5, 1, -0.5, 0.2, 0.1);
        assert!((buf[2 * 5 + 2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn zero_coefficients_is_identity_f32() {
        let mut buf = vec![0.5_f32; 4 * 3 * 3];
        let original = buf.clone();
        vignetting_pa_apply_f32(&mut buf, 4, 3, 3, 0.0, 0.0, 0.0);
        for (a, b) in buf.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn empty_buffers_are_noops() {
        let mut empty: Vec<f32> = Vec::new();
        vignetting_pa_apply_f32(&mut empty, 0, 0, 3, 0.1, 0.0, 0.0);
        vignetting_pa_correct_f32(&mut empty, 0, 0, 3, 0.1, 0.0, 0.0);
        assert!(empty.is_empty());
    }
}
