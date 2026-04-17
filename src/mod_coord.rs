//! Coordinate transforms: distortion correction + geometry conversions.
//!
//! Port of `libs/lensfun/mod-coord.cpp`. 28 transforms total — distortion (ptlens,
//! poly3, poly5) plus geometry conversions between rectilinear, fisheye variants,
//! equirectangular, and panoramic projections.
//!
//! The math is textbook; the value is in matching upstream's exact float output
//! against `tests/test_modifier_coord_*.cpp`.

// Distortion models (v0.2): port from mod-coord.cpp lines 560-758.
//   - poly3:   ModifyCoord_UnDist_Poly3, lines 560-613.
//   - poly5:   ModifyCoord_UnDist_Poly5, lines 634-693.
//   - ptlens:  ModifyCoord_UnDist_Ptlens, lines 694-758.
//
// Geometry conversions (v0.2):
//   - rectilinear ↔ fisheye (equidistant, orthographic, equisolid, stereographic).
//   - equirectangular, panoramic.
