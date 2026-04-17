# lensfun

[![CI](https://github.com/vdavid/lensfun-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/vdavid/lensfun-rs/actions/workflows/ci.yml)

Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction without C dependencies.

**Status:** Pre-alpha. API not stable. v0.1 ports the database loader and type surface; v0.2+ add the correction math. See [`docs/notes/lensfun-rs.md`](docs/notes/lensfun-rs.md) for the porting plan.

## What it does

Given a camera body, lens model, and shooting parameters (focal length, aperture, distance), return correction profiles for:

- **Distortion** — radial barrel/pincushion (`ptlens`, `poly3`, `poly5` models).
- **Transverse chromatic aberration (TCA)** — per-channel radial shift (`linear`, `poly3`).
- **Vignetting** — radial brightness falloff (`pa` model).
- **Geometry** — convert between rectilinear, fisheye, equirectangular, panoramic.

The actual pixel passes are scalar Rust — no SIMD in v1.

## Why pure Rust

- No `-sys` crates, no system libraries, no C toolchain for cross-compilation.
- Clean static-linking under LGPL-3.0 with public source.
- Same correction math as upstream LensFun (the C++ tests are ported 1:1 as the spec).

## Quick start

```rust
// API sketch — not yet implemented.
use lensfun::Database;

let db = Database::load_dir("/usr/share/lensfun/")?;
let lens = db.find_lens("Canon EF 24-70mm f/2.8L II USM", "Canon EOS R5")?;
let modifier = lens.modifier_for(35.0 /* focal */, 4.0 /* aperture */, 5.0 /* distance */);
modifier.apply_distortion(&mut pixel_buf, width, height);
```

## License

- **Code:** LGPL-3.0-or-later (derivative of upstream LensFun).
- **Bundled XML database:** CC-BY-SA 3.0 from upstream LensFun contributors.

## Acknowledgements

This crate is a port of [LensFun](https://github.com/lensfun/lensfun) by Andrew Zabolotny and the LensFun contributors. All correction algorithms, calibration data, and the XML schema are theirs. The Rust port is by [@vdavid](https://github.com/vdavid).
