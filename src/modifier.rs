//! Modifier — composition of correction passes.
//!
//! Port of `libs/lensfun/modifier.cpp` and the `lfModifier` type in
//! `include/lensfun/lensfun.h.in`.

use crate::lens::Lens;

/// A correction modifier configured for a specific shot (focal, aperture, distance).
///
/// Built from a `Lens` plus shooting parameters. Holds the interpolated coefficients
/// for distortion, TCA, and vignetting, ready to apply to pixel buffers.
///
/// **Not yet implemented.**
#[derive(Debug, Clone, Default)]
pub struct Modifier {
    // Interpolated coefficients land here in v0.2+.
}

impl Modifier {
    /// Build a modifier for a lens + shot.
    ///
    /// **Not yet implemented.**
    pub fn for_lens(_lens: &Lens, _focal: f32, _aperture: f32, _distance: f32) -> Self {
        Self::default()
    }
}
