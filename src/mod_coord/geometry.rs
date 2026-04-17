//! Geometry projection conversions (port of `mod-coord.cpp` geometry section).
//!
//! Convert between rectilinear, fisheye (equidistant / orthographic / equisolid /
//! stereographic / Thoby), equirectangular, and panoramic projections. Pure per-pixel
//! functions — the higher-level `Modifier` composes them with focal-length and image-
//! dimension scaling.

// Implementation lands here in v0.2 work.
