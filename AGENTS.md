# lensfun-rs

Pure-Rust port of [LensFun](https://github.com/lensfun/lensfun) — camera lens correction (distortion, transverse chromatic aberration, vignetting) with no C dependencies. Read-only port of the upstream `libs/lensfun` C++ core.

## Quick commands

| Command          | Description                                    |
|------------------|------------------------------------------------|
| `just`           | Run all fast checks: format, lint, test, doc   |
| `just fix`       | Auto-fix formatting and clippy warnings        |
| `just check-all` | Include MSRV check, security audit, deny      |
| `cargo test`     | Run unit + integration tests                   |

## Project structure

```
src/
  db.rs          # XML database loader (port of database.cpp)
  lens.rs        # Lens type + interpolation (port of lens.cpp)
  camera.rs      # Camera type
  mount.rs       # Mount type
  calib.rs       # CalibDistortion, CalibTca, CalibVignetting
  modifier.rs    # Modifier — composition of corrections
  mod_coord.rs   # Distortion + geometry transforms (port of mod-coord.cpp)
  mod_color.rs   # Vignetting (port of mod-color.cpp)
  mod_subpix.rs  # TCA per-channel correction (port of mod-subpix.cpp)
  auxfun.rs      # lfFuzzyStrCmp + Catmull-Rom interp (port of auxfun.cpp)
  error.rs
  lib.rs

tests/
  integration/   # Ports of upstream tests/test_*.cpp — these ARE the spec.

related-repos/   # Local-only clone of upstream lensfun (gitignored).
data/db/         # Bundled XML database from upstream.
docs/            # Architecture, style guide, design notes.
```

## Architecture

```
db::Database  →  finds Camera + Lens + Calibration profiles
                   ↓
                 Modifier  ←  composes distortion + TCA + vignetting passes
                   ↓
            mod_coord / mod_color / mod_subpix (pure float math)
```

**Entry points:** `Database::load_dir(path)`, `Database::find_lens(...)`, `Modifier::for_lens(...)`.

## Source of truth

This is a **read-only port** of upstream LensFun. When a behavior question comes up, the answer is in the upstream C++ source under `related-repos/lensfun/libs/lensfun/`, and the upstream tests `related-repos/lensfun/tests/test_*.cpp` are the executable spec. Port test files 1:1 to `tests/integration/<name>.rs` and don't simplify the heuristics.

Pinned upstream commit: see `docs/notes/upstream-pin.md` (TBD).

## License

- This crate: **LGPL-3.0-or-later** (derivative of upstream LensFun, can't relicense).
- Bundled XML database: **CC-BY-SA 3.0** (separate from the code).
- Do **not** copy from upstream `apps/` — that's GPL-3.0.

## Things to avoid

- Don't simplify upstream heuristics (`MatchScore`, `lfFuzzyStrCmp`) — port as-is, drive changes from test evidence.
- Don't rearrange float algebra "for clarity" — bit-exact float match against upstream tests matters.
- No SIMD for v1. Defer `mod-coord-sse.cpp` and `mod-color-sse*.cpp` to post-v1.
- No `SaveXML` / database authoring. Read-only.
- No CLI / GUI in this crate.

## Code style

Run `just check` before committing. `cargo fmt`, `cargo clippy -D warnings`, tests for new functionality, doc comments for public APIs.

## References

- [docs/architecture.md](docs/architecture.md), [docs/style-guide.md](docs/style-guide.md), [docs/design-principles.md](docs/design-principles.md)
- [docs/notes/lensfun-rs.md](docs/notes/lensfun-rs.md) — original porting spec
- [Upstream LensFun](https://github.com/lensfun/lensfun)
