//! Color pass: vignetting correction.
//!
//! Port of `libs/lensfun/mod-color.cpp`. The math is straightforward —
//! `gain = 1 + k1·r² + k2·r⁴ + k3·r⁶`, applied per pixel. SSE variants
//! (`mod-color-sse.cpp`, `mod-color-sse2.cpp`) are deferred to a post-v1
//! milestone.

// Vignetting kernel (v0.3): port from mod-color.cpp:318.
