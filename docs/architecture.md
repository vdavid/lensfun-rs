# Architecture

`lensfun-rs` is a port of the upstream LensFun C++ core (`libs/lensfun/`). The architecture mirrors upstream because upstream's structure is already lean: plain-data structs plus free functions, zero virtual dispatch, no inheritance beyond a single exception type.

## Module layout

```
src/
├── db.rs            # XML database loader + queries (port of database.cpp)
├── lens.rs          # Lens type + 4D calibration interpolation (port of lens.cpp)
├── camera.rs        # Camera type (port of camera.cpp)
├── mount.rs         # Mount type (port of mount.cpp)
├── calib.rs         # CalibDistortion / CalibTca / CalibVignetting structs
├── modifier.rs      # Modifier — composes correction passes (port of modifier.cpp)
├── mod_coord.rs     # Distortion + geometry transforms (port of mod-coord.cpp)
├── mod_color.rs     # Vignetting (port of mod-color.cpp)
├── mod_subpix.rs    # TCA per-channel correction (port of mod-subpix.cpp)
├── auxfun.rs        # lfFuzzyStrCmp + Catmull-Rom interp (port of auxfun.cpp)
├── error.rs
└── lib.rs
```

## Data flow

```
data/db/*.xml
     │
     ▼
db::Database  ─── find_lens / find_camera / find_mount ──►  Lens, Camera, Mount, Calibration*
     │
     ▼
modifier::Modifier
     │
     ├─► mod_coord (distortion: ptlens, poly3, poly5; geometry conversions)
     ├─► mod_color (vignetting: gain = 1 + k1·r² + k2·r⁴ + k3·r⁶)
     └─► mod_subpix (TCA: per-channel radial + tangential)
```

## Design choices

### Pure data + free functions

Upstream is procedural C++ with public structs (`lfLens`, `lfCamera`, ...). We mirror that with public Rust structs and inherent impls or free functions. **No traits with virtual dispatch, no generics for things that aren't generic.**

### Error handling

A single `Error` enum (`thiserror`-derived) covers parse errors, lookup failures, and I/O. Math functions don't fail — they return finite floats or panic on caller-supplied invalid inputs (negative focal length, etc.) via debug assertions.

### Float determinism

Upstream tests pin specific float values. We do **not** rearrange algebra "for clarity." Operation order matters when you're chasing bit-exact match.

### Database storage strategy

Two paths, both supported as of v0.5:

- **Runtime parsing**: `Database::load_dir(path)` parses XML from disk. Use this when consumers ship the DB out-of-band or want a custom location.
- **Build-time bundling (default)**: `Database::load_bundled()` decompresses the gzipped XML embedded by `build.rs` (~574 KB on disk, ~5 MB in RAM after parse). Zero-I/O. No feature flag — `flate2` is always pulled in.

### Why no traits for `Modifier`

Upstream's `lfModifier` is a struct, not a hierarchy. The "different correction kinds" are conditionals on which calibration is present. A trait-per-correction would be cosmetic abstraction that adds dispatch cost without buying anything.

## Risk areas

See `docs/notes/lensfun-rs.md` for the full risk register. Headlines:

1. **`lens::Interpolate*` (`lens.cpp:910-1292`)** — 4D spline with nearest-neighbor fallback. Bit-exact match against upstream is hard.
2. **`auxfun::fuzzy_str_cmp`** — UTF-8 word splitting via glib in upstream. Our port uses `str::split_whitespace`; verify against `test_lffuzzystrcmp.cpp`.
3. **`db::match_score`** — 30+ ad-hoc weights. Don't simplify; port test-driven.
4. **Float determinism across platforms** — operation order matters; don't rearrange.
