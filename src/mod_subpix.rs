//! Sub-pixel pass: transverse chromatic aberration (TCA) correction.
//!
//! Port of `libs/lensfun/mod-subpix.cpp`. Per-channel distortion: red and blue
//! planes get independent radial corrections relative to green.

// TCA kernels (v0.3):
//   - linear:  pure radial scale per channel.
//   - poly3:   adds a cubic term per channel.
