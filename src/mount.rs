//! Lens mount.
//!
//! Port of `libs/lensfun/mount.cpp` and the `lfMount` type in
//! `include/lensfun/lensfun.h.in`.

use std::collections::BTreeMap;

/// A lens mount: the physical interface between a lens and a camera body.
///
/// Mounts can be compatible with one another (for example, Canon EF lenses
/// mount on Canon EF-S bodies via no adapter, and on Canon RF bodies via an
/// adapter). The compatibility list captures these relationships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mount {
    /// Mount name (e.g. `"Canon EF"`).
    pub name: String,
    /// Localized mount names keyed by `lang` attribute (e.g. `"en"`, `"de"`).
    pub names_localized: BTreeMap<String, String>,
    /// Names of compatible mounts.
    pub compat: Vec<String>,
}

impl Mount {
    /// Build a mount with just a name; convenience for tests.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}
