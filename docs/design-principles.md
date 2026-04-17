# Design principles

## Faithful port, not a redesign

This crate is a **port** of upstream LensFun, not a re-imagining. When upstream and ergonomics conflict, upstream wins until tests prove otherwise. The 30+ magic numbers in `lfDatabase::MatchScore` are not for us to "clean up." Compatibility with upstream behavior, demonstrated by a passing port of the upstream test suite, is the v1.0 success criterion.

## Pure Rust, zero FFI

No `-sys` crates. No `bindgen`. No system libraries. The whole point is to give Rust users lens correction without dragging C++ into the build.

## Read-only

The database is a read source, not a write target. We don't port `SaveXML`. Consumers who need to author calibration data should use upstream LensFun's tooling.

## Tests are the spec

The upstream `tests/test_*.cpp` files are 4,495 LoC of pure-math reference outputs. Port them 1:1 to Rust integration tests. If your implementation passes them, you have a reference-consistent implementation. If it doesn't, it's wrong, even if "the math looks right."

## Scalar first, SIMD later

v1 ships scalar Rust. SIMD (the upstream `mod-coord-sse.cpp`, `mod-color-sse*.cpp`) is a v1.x optimization. Ship correctness first, measure, then accelerate.

## License-aware

Upstream core is **LGPL-3.0-or-later**. The Rust port stays LGPL-3.0-or-later — derivative works can't relicense. Bundled XML database is **CC-BY-SA 3.0**, separate from the code license. Do not read or derive from upstream `apps/` (GPL-3.0).

## No surprise dependencies

The dep set should stay small: an XML parser, `regex`, `thiserror`. Adding a new dep crosses a bar — what does it buy us, and what does it cost in build time and audit surface? We're a foundational crate; consumers will not thank us for a 200-crate transitive tree.
