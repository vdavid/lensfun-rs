# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-04-20

Initial public release. Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction without C dependencies. Verified equivalent to upstream across 1,640 A/B test cases within 4.88 × 10⁻⁴ pixels.

### Added

- XML database loader with bundled gzip-compressed upstream DB (~574 KB on disk, ~5 MB in RAM).
- Distortion correction: `ptlens`, `poly3`, `poly5` models (forward and inverse via Newton iteration).
- Geometry conversions: 20 conversions across rectilinear, fisheye variants, equirectangular, and panoramic projections.
- Transverse chromatic aberration (TCA) correction: linear and `poly3` models, per-channel.
- Vignetting correction: `pa` model for `u8`, `u16`, and `f32` pixel formats.
- 4D calibration interpolation across (focal, aperture, distance) with nearest-neighbor fallback.
- Catmull-Rom spline interpolation for smooth parameter blending.
- Fuzzy lens-name matcher (port of `lfFuzzyStrCmp`).
- `MatchScore`-based `find_cameras` and `find_lenses` lookup.
- `GuessParameters` extractor from lens model strings (3-regex pipeline).
- `Modifier` with row-batched `apply_geometry_distortion`, `apply_subpixel_distortion`, and `apply_color_modification_*` for `u8` / `u16` / `f32` buffers.
- Perspective correction (port of `mod-pc.cpp`) with hand-rolled Jacobi SVD — no `nalgebra` dependency.
- Build-time gzip bundling of the XML database via `build.rs`.
- 1,640-case A/B harness vs upstream LensFun C++ with documented methodology and reproducible scripts (excluded from the published crate).
- 162 unit and integration tests.

[Unreleased]: https://github.com/vdavid/lensfun-rs/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/vdavid/lensfun-rs/releases/tag/v0.7.0
