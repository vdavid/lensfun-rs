//! Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction
//! (distortion, transverse chromatic aberration, vignetting) without C dependencies.
//!
//! # Status
//!
//! Pre-alpha. API may still shift. The math is bit-exact-tested against the upstream
//! C++ reference (`tests/test_modifier_regression.cpp` values pinned at `1e-3`).
//! See `docs/notes/lensfun-rs.md` for the porting plan and `docs/notes/handoff-*.md`
//! for the latest checkpoint.
//!
//! # Quick start
//!
//! ```no_run
//! use lensfun::{Database, Modifier};
//!
//! let db = Database::load_bundled()?;
//! let cameras = db.find_cameras(Some("Canon"), "EOS R5");
//! let camera = cameras.first().expect("camera in bundled DB");
//! let lenses = db.find_lenses(Some(camera), "Canon EF 24-70mm f/2.8L II USM");
//! let lens = lenses.first().expect("lens in bundled DB");
//!
//! let (width, height) = (6720_u32, 4480_u32);
//! let mut modifier = Modifier::new(lens, 35.0, camera.crop_factor, width, height, true);
//! modifier.enable_distortion_correction(lens);
//! modifier.enable_tca_correction(lens);
//! modifier.enable_vignetting_correction(lens, 4.0, 5.0);
//!
//! // Per-row coordinate transform (one row of `width` pixels).
//! let mut coords = vec![0.0_f32; (width as usize) * 2];
//! modifier.apply_geometry_distortion(0.0, 0.0, width as usize, 1, &mut coords);
//! # Ok::<(), lensfun::Error>(())
//! ```
//!
//! # Module map
//!
//! Each module corresponds to one upstream C++ source file:
//!
//! | Module | Upstream |
//! |---|---|
//! | [`db`] | `libs/lensfun/database.cpp` |
//! | [`lens`] | `libs/lensfun/lens.cpp` |
//! | [`camera`] | `libs/lensfun/camera.cpp` |
//! | [`mount`] | `libs/lensfun/mount.cpp` |
//! | [`modifier`] | `libs/lensfun/modifier.cpp` |
//! | [`mod_coord`] | `libs/lensfun/mod-coord.cpp` |
//! | [`mod_pc`] | `libs/lensfun/mod-pc.cpp` |
//! | [`mod_color`] | `libs/lensfun/mod-color.cpp` |
//! | [`mod_subpix`] | `libs/lensfun/mod-subpix.cpp` |
//! | [`auxfun`] | `libs/lensfun/auxfun.cpp` |

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![allow(clippy::too_many_arguments)]

pub mod auxfun;
pub mod calib;
pub mod camera;
pub mod db;
pub mod error;
pub mod lens;
pub mod mod_color;
pub mod mod_coord;
pub mod mod_pc;
pub mod mod_subpix;
pub mod modifier;
pub mod mount;

pub use auxfun::{FuzzyStrCmp, fuzzy_str_cmp};
pub use calib::{
    CalibDistortion, CalibTca, CalibVignetting, DistortionModel, TcaModel, VignettingModel,
};
pub use camera::Camera;
pub use db::Database;
pub use error::Error;
pub use lens::{Lens, LensType};
pub use modifier::Modifier;
pub use mount::Mount;
