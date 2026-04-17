//! Coordinate transforms: distortion correction + geometry conversions.
//!
//! Port of `libs/lensfun/mod-coord.cpp`. 28 transforms total — distortion (ptlens,
//! poly3, poly5) plus geometry conversions between rectilinear, fisheye variants,
//! equirectangular, and panoramic projections.
//!
//! The math is textbook; the value is in matching upstream's exact float output
//! against `tests/test_modifier_coord_*.cpp`.

pub mod distortion;
pub mod geometry;

pub use distortion::{
    dist_poly3, dist_poly5, dist_ptlens, undist_poly3, undist_poly5, undist_ptlens,
};
