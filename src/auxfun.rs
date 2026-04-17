//! Auxiliary helpers: fuzzy string compare + Catmull-Rom spline interpolation.
//!
//! Port of `libs/lensfun/auxfun.cpp`.

// fuzzy_str_cmp (v0.4): port from auxfun.cpp:360-540.
//   Split pattern into words, score = matched / mean(word_count_a, word_count_b) × 100.
//   Verify bit-exact against tests/test_lffuzzystrcmp.cpp.
//
// interpolate (v0.2): port from auxfun.cpp:335.
//   Catmull-Rom spline across focal-length axis (and aperture axis for vignetting).
//   ~25 LoC.
