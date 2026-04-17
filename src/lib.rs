//! Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction
//! (distortion, transverse chromatic aberration, vignetting) without C dependencies.
//!
//! # Status
//!
//! Pre-alpha. API not stable. v0.1 ports the database loader and type surface; v0.2+ add the
//! correction math. See `docs/notes/lensfun-rs.md` for the porting plan.
//!
//! # Quick start (sketched — not all wired yet)
//!
//! ```ignore
//! use lensfun::Database;
//!
//! let db = Database::load_dir("/usr/share/lensfun/")?;
//! let lens = db.find_lens("Canon EF 24-70mm f/2.8L II USM", "Canon EOS R5")?;
//! let modifier = lens.modifier_for(35.0, 4.0, 5.0);
//! modifier.apply_distortion(&mut pixel_buf, width, height);
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
