//! XML database loader + queries.
//!
//! Port of `libs/lensfun/database.cpp`.
//!
//! The upstream parser is a glib SAX parser; we use `roxmltree` (DOM) since the database
//! files are small (5 MB total uncompressed) and DOM is simpler to port.

use std::path::{Path, PathBuf};

use crate::camera::Camera;
use crate::error::{Error, Result};
use crate::lens::Lens;
use crate::mount::Mount;

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
    /// Load every `*.xml` file from the given directory.
    ///
    /// Mirrors upstream `lfDatabase::Load` behavior: each file's contents are merged
    /// into a single in-memory database. Conflicting entries are resolved by
    /// last-loaded-wins, matching upstream.
    ///
    /// **Not yet implemented.** Tracked by task `v0.1 — XML database loader`.
    pub fn load_dir(_path: impl AsRef<Path>) -> Result<Self> {
        Err(Error::NoMatch(
            "Database::load_dir not implemented yet".to_string(),
        ))
    }

    /// Load a single XML file and merge its entries.
    ///
    /// **Not yet implemented.**
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let _path: PathBuf = path.as_ref().to_path_buf();
        Err(Error::NoMatch(
            "Database::load_file not implemented yet".to_string(),
        ))
    }

    /// Find the cameras that best match the given maker + model strings.
    ///
    /// **Not yet implemented.** Will port `lfDatabase::FindCameras`.
    pub fn find_cameras(&self, _maker: Option<&str>, _model: &str) -> Vec<&Camera> {
        Vec::new()
    }

    /// Find the lenses that best match the given camera + model string.
    ///
    /// **Not yet implemented.** Will port `lfDatabase::FindLenses`.
    pub fn find_lenses(&self, _camera: Option<&Camera>, _model: &str) -> Vec<&Lens> {
        Vec::new()
    }
}
