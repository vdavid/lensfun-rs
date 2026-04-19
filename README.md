# lensfun

[![CI](https://github.com/vdavid/lensfun-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/vdavid/lensfun-rs/actions/workflows/ci.yml)

Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction without C dependencies.

> **Verified equivalent to upstream LensFun.** Across 1,640 A/B test cases (6 lenses × 4-5 focal lengths × 30 coordinates × forward/reverse), the Rust port matches the C++ original within a max delta of 4.9 × 10⁻⁴ pixels — about 2,000× under the 1 × 10⁻³ tolerance the upstream regression suite uses. Vignetting is faster in Rust (1.4×); distortion and TCA are slower per-call (~2.5×) but fast enough for real-world image work. See [`docs/comparison-with-c-library.md`](docs/comparison-with-c-library.md) for the methodology, results, and how to reproduce the comparison locally.

## What it does

Given a camera body, lens model, and shooting parameters (focal length, aperture, distance), return correction profiles for:

- **Distortion** — radial barrel/pincushion (`ptlens`, `poly3`, `poly5` models).
- **Transverse chromatic aberration (TCA)** — per-channel radial shift (`linear`, `poly3`).
- **Vignetting** — radial brightness falloff (`pa` model).
- **Geometry** — convert between rectilinear, fisheye, equirectangular, panoramic.
- **Perspective correction** — port of `mod-pc.cpp` with hand-rolled Jacobi SVD.

The pixel passes are scalar Rust — no SIMD yet (planned post-1.0).

## Why pure Rust

- No `-sys` crates, no system libraries, no C toolchain for cross-compilation.
- Clean static-linking under LGPL-3.0 with public source.
- Same correction math as upstream LensFun, verified by [an automated A/B harness](docs/comparison-with-c-library.md).

## Quick start

```rust
use lensfun::{Database, Modifier};

let db = Database::load_bundled()?;
let cameras = db.find_cameras(Some("Canon"), "EOS R5");
let camera = cameras.first().expect("camera in bundled DB");
let lenses = db.find_lenses(Some(camera), "Canon EF 24-70mm f/2.8L II USM");
let lens = lenses.first().expect("lens in bundled DB");

let (width, height) = (6720_u32, 4480_u32);
let mut modifier = Modifier::new(lens, 35.0, camera.crop_factor, width, height, true);
modifier.enable_distortion_correction(lens);
modifier.enable_tca_correction(lens);
modifier.enable_vignetting_correction(lens, 4.0, 5.0);

// Per-row coordinate transform (one row of `width` pixels).
let mut coords = vec![0.0_f32; (width as usize) * 2];
modifier.apply_geometry_distortion(0.0, 0.0, width as usize, 1, &mut coords);
# Ok::<(), lensfun::Error>(())
```

The lens calibration database is bundled (~574 KB gzipped, ~5 MB after decompression). For consumers who want to load from disk instead, use `Database::load_dir(path)`.

## Status

Pre-alpha. The math is bit-equivalent to the upstream C++ reference. The public API is small and stable-ish but not yet locked. v0.1 - v0.6 are functionally complete; v1.0 needs license + crates.io polish + cross-platform CI runs.

See [`docs/notes/lensfun-rs.md`](docs/notes/lensfun-rs.md) for the porting plan and [`docs/notes/handoff-2026-04-18.md`](docs/notes/handoff-2026-04-18.md) for the latest checkpoint.

## License

- **Code:** LGPL-3.0-or-later (derivative of upstream LensFun). See [`LICENSE-LGPL-3.0`](LICENSE-LGPL-3.0) and [`LICENSE-GPL-3.0`](LICENSE-GPL-3.0) (which the LGPL incorporates by reference).
- **Bundled XML database:** CC-BY-SA 3.0 from upstream LensFun contributors.
- See [`NOTICE`](NOTICE) for full attribution.

## Acknowledgements

This crate is a port of [LensFun](https://github.com/lensfun/lensfun) by Andrew Zabolotny and the LensFun contributors. All correction algorithms, calibration data, and the XML schema are theirs. The Rust port is by [@vdavid](https://github.com/vdavid).
