//! Camera body.
//!
//! Port of `libs/lensfun/camera.cpp` and the `lfCamera` type in
//! `include/lensfun/lensfun.h.in`.

use std::collections::BTreeMap;

/// A camera body, identified by maker + model + variant + mount + crop factor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Camera {
    /// Camera manufacturer (e.g. `"Canon"`).
    pub maker: String,
    /// Localized maker names keyed by `lang`.
    pub maker_localized: BTreeMap<String, String>,
    /// Camera model (e.g. `"EOS R5"`).
    pub model: String,
    /// Localized model names keyed by `lang`.
    pub model_localized: BTreeMap<String, String>,
    /// Optional variant string (e.g. `"firmware A"`).
    pub variant: Option<String>,
    /// Mount this body uses.
    pub mount: String,
    /// Crop factor relative to a 35 mm full-frame sensor.
    pub crop_factor: f32,
    /// Score returned by `Database::find_camera` matching; populated by query results.
    pub score: i32,
}
