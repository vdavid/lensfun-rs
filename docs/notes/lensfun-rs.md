# lensfun-rs — pure-Rust port spec

A spec for porting [LensFun](https://github.com/lensfun/lensfun) (C++, LGPL-3.0) to
pure Rust as a standalone crate. Intended to be consumed by a lead agent that
decomposes the work into subagent tasks and delivers it in a new repo at
`github.com/vdavid/lensfun-rs`.

The destination repo should match the structure and tooling of
`~/projects-git/vdavid/mtp-rs`. Copy its `scripts/check.sh`, `AGENTS.md`
template, `CONTRIBUTING.md`, `justfile`, `clippy.toml`, `rustfmt.toml`,
`deny.toml`, `.github/` workflows, and `docs/` layout. This spec covers the
actual rewrite; the surrounding project setup is "do what mtp-rs does".

## Purpose

Add camera lens correction (distortion, TCA, vignetting) to Rust applications
without pulling in C/C++ dependencies. First consumer: the Prvw image viewer's
Phase 5 roadmap — see `raw-roadmap.md` in the Prvw repo.

## Why port instead of bind

- **Zero Rust bindings exist today.** Not on crates.io, not on lib.rs. If we
  don't port, we still have to hand-write `bindgen`-generated `lensfun-sys`
  bindings, which drags LGPL-3.0 C runtime into the binary.
- **Pure Rust is a positioning advantage.** Deterministic builds, no C
  toolchain for cross-compilation, no system libraries to locate on install,
  cleaner static-linking story under LGPL-3.0 with public source.
- **The C++ core is thin.** 7,756 LoC of procedural code with no inheritance
  (one exception type), confined external deps (glib in 2 files, `std::regex`
  in one), and a 4,495-LoC test suite that serves as an executable
  specification.
- **The code is frozen; the data is alive.** 22 core commits in 5 years vs.
  1,102 database commits. Port once, re-import the XML periodically, done.

## Scope

### Must-have for v1 (covers Prvw's use case)

- **XML database loader** — read LensFun's `data/db/*.xml` files, produce
  queryable structs for mounts, cameras, lenses, and calibration data.
- **Distortion correction** — all three models: `ptlens`, `poly3`, `poly5`.
  Inverse warp with Newton iteration for poly3/poly5.
- **TCA correction** — `linear` and `poly3` models.
- **Vignetting correction** — `pa` model (three coefficients).
- **Geometry conversions** — rectilinear, fisheye, panoramic, equirectangular.
  Textbook trigonometry.
- **Catmull-Rom spline interpolation** across focal-length and aperture axes,
  with nearest-neighbor fallback when calibration points are sparse.
- **Matching logic** — `MatchScore` for pairing a camera body + lens model
  string with the right profile.
- **Fuzzy string matcher** — `lfFuzzyStrCmp` port.

### Nice-to-have for v1.x

- **Perspective correction** (`mod-pc.cpp`) — requires SVD. `nalgebra` has
  SVD; or port the hand-rolled Jacobi from Rasmussen 1996 directly.
- **Build-time data bundling** — a `build.rs` that reads the XML and emits
  compressed `&'static [u8]` or `phf` maps, so consumers don't need the XML
  at runtime.

### Explicitly out of scope for v1

- **SIMD-accelerated appliers.** The C++ has `mod-coord-sse.cpp` and
  `mod-color-sse*.cpp`. Skip for v1, revisit with `wide` or `std::simd`
  later.
- **`SaveXML` / database authoring.** Consumers read, not write.
- **The `lenstool` CLI.** That's GPL-3.0; leave it to a downstream wrapper.

## Source of truth

1. **Upstream LensFun repo**: https://github.com/lensfun/lensfun
2. **Upstream core**: `libs/lensfun/` (C++ source)
3. **Upstream API**: `include/lensfun/lensfun.h.in` — the public surface to
   mirror.
4. **Upstream tests**: `tests/test_*.cpp` — port 1:1 as Rust integration
   tests. These ARE the spec.
5. **Upstream database**: `data/db/*.xml` plus `data/db/lensfun-database.dtd`
   and `data/db/lensfun-database.xsd`.

Pin to a specific upstream tag or commit. Periodically rebase and sync database
changes.

## Architecture — what's actually in the 7,756-LoC C++ core

| File | LoC | Purpose |
|---|---:|---|
| `database.cpp` | 1,654 | glib SAX XML parser, `Find*` queries, `MatchScore` |
| `lens.cpp` | 1,499 | `lfLens` accessors, `Interpolate*` (4D spline) |
| `mod-coord.cpp` | 1,250 | 28 coordinate transforms (distortion + geometry) |
| `mod-pc.cpp` | 766 | perspective correction + hand-rolled SVD |
| `auxfun.cpp` | 523 | `lfFuzzyStrCmp`, `_lf_interpolate`, MLstr helpers |
| `mod-coord-sse.cpp` | (skip) | SSE variants — skip for v1 |
| `mod-color.cpp` | 374 | vignetting pixel pass, scalar + templated |
| `mod-subpix.cpp` | 406 | TCA sub-pixel correction |
| `mod-color-sse*.cpp` | (skip) | SSE variants — skip for v1 |
| `modifier.cpp`, `camera.cpp`, `mount.cpp`, `cpuid.cpp` | ~570 | glue + constructors |

**Zero virtual functions. Minimal use of `class`.** The public types (`lfLens`,
`lfCamera`, `lfMount`, `lfLensCalibDistortion`, etc.) are plain structs with
external functions. Port target: idiomatic Rust structs plus free functions or
inherent impls.

## External C++ deps — what to replace with

| C++ dep | Used in | Rust replacement |
|---|---|---|
| **glib 2.x** (SAX parser, `GString`, `GPtrArray`, `g_build_filename`) | `database.cpp`, `auxfun.cpp`, trivial use in `camera.cpp` | `quick-xml` (SAX) or `roxmltree` (DOM), `std::path`, `dirs` crate |
| **`std::regex`** (3 fixed patterns) | `lens.cpp:152-169` | `regex` crate, `once_cell::sync::Lazy` |
| **`<math.h>`** | everywhere | `f32::sqrt` etc. in `std` |
| libxml2, boost, icu, sqlite | not used | — |

## Database format

- **XML files** in `data/db/`, DTD at `data/db/lensfun-database.dtd`, schema at
  `data/db/lensfun-database.xsd`.
- **59 files, 5.0 MB uncompressed, 580 KB gzipped.**
- **Schema is flat**: `<mount>`, `<camera>`, `<lens>` with `<calibration>`
  children containing `<distortion>`, `<tca>`, `<vignetting>` entries.
  Multi-language strings via `lang="xx"` attribute.
- **Counts**: 1,041 cameras, 1,543 lenses, 6,377 distortion entries, 3,717 TCA
  entries, 29,085 vignetting entries.
- **Distortion models supported**: exactly 3 — `ptlens`, `poly3`, `poly5`.
- **TCA models**: 2 — `linear`, `poly3`.
- **Vignetting model**: 1 — `pa` with 3 coefficients.

Two valid runtime strategies; pick one per milestone:

1. **Runtime parsing**: ship the XML alongside the crate or let consumers
   supply a path. Parse on first use, cache in memory.
2. **Build-time bundling**: `build.rs` reads the XML and emits compressed
   static Rust data. Consumers get zero-I/O lookup. Size on disk: ~580 KB
   gzipped, fits comfortably in a crate.

Start with runtime parsing (matches upstream behavior). Add build-time
bundling in v1.1 if consumers request it.

## Algorithm details

All procedural float math. Reference implementations in the C++ source by line
number.

### Distortion models (`mod-coord.cpp`)

- **poly3**: `ModifyCoord_UnDist_Poly3` lines 560-613. Newton iteration
  solving `k1·Ru³ + Ru = Rd`.
- **poly5**: lines 634-693. Newton iteration on a 5th-order polynomial.
- **ptlens**: lines 694-758. 4th-order polynomial, closed-form.

### Vignetting (`mod-color.cpp:318`)

Simple: `gain = 1 + k1·r² + k2·r⁴ + k3·r⁶`. Multiply the pixel by `gain` across
the image plane.

### TCA (`mod-subpix.cpp`)

Per-channel distortion model: red and blue planes get independent radial +
tangential corrections relative to green. The `linear` model is a pure radial
scale; `poly3` adds a cubic term.

### Geometry (`mod-coord.cpp`)

Rectilinear, fisheye (equidistant, orthographic, equisolid), equirectangular,
panoramic. Textbook spherical trig.

### Spline interpolation (`auxfun.cpp:335`, `_lf_interpolate`)

Catmull-Rom across the focal-length axis (and aperture axis for vignetting).
Roughly 25 LoC.

### 4-dimensional calibration interpolation (`lens.cpp:910-1292`, `Interpolate*`)

**The tricky one.** Given a sparse set of calibration entries indexed by
(crop-factor, focal, aperture, distance), find the right interpolant. Falls
back to nearest-neighbor when no clean spline is available. Port
test-by-test — `tests/test_modifier_coord_distortion.cpp` and its cousins
pin down the expected behavior.

### Perspective correction SVD (`mod-pc.cpp:104`)

One-sided Jacobi SVD from Rasmussen 1996, ~80 LoC. Either reuse `nalgebra`'s
SVD or port directly. Not exotic — just needs care and test coverage.

## Matching logic

### `lfDatabase::MatchScore` (`database.cpp:1252-1384`)

Score-based combining:

- Crop-factor bucketing (8 tiers)
- Numeric range checks on focal length and aperture
- Mount-compatibility scan
- Fuzzy string match on model name

**Caveat**: the weighting has ~30 magic numbers. Port test-driven; don't try to
"simplify" the heuristic.

### `lfFuzzyStrCmp` (`auxfun.cpp:360-540`)

- Split pattern into words.
- Score = `matched_words / mean(word_count_a, word_count_b) × 100`.
- No locale-awareness, no unicode normalization, no complex regex.
- Uses glib UTF-8 helpers — port against `test_lffuzzystrcmp.cpp` for
  bit-exact fidelity.

### `lfLens::GuessParameters` (`lens.cpp:171`)

3 fixed regexes extract focal length and aperture from model-name strings like
"24-70mm f/2.8". Port straight to the `regex` crate.

## Testing strategy

**Port the upstream `tests/test_*.cpp` files 1:1 to Rust integration tests.**
These are 4,495 LoC of pure math tests that serve as the reference spec. If
your port passes them, you have a reference-consistent implementation.

Key test files (all in upstream `tests/`):

- `test_modifier_coord_distortion.cpp` (205 LoC)
- `test_modifier_coord_geometry.cpp` (205)
- `test_modifier_coord_scale.cpp` (176)
- `test_modifier_subpix.cpp` (199)
- `test_modifier_perspective_correction.cpp` (252)
- `test_modifier_regression.cpp` (365)
- `test_modifier_color.cpp` (265)
- `test_lffuzzystrcmp.cpp`
- `test_database.cpp`

Port each file as `tests/integration/<name>.rs`. Use the same input values; assert
the same output values within documented float tolerance.

Add native Rust property tests (`proptest` — mtp-rs uses it already) on top:
- Round-trip identity (forward then inverse distortion returns the original
  within tolerance).
- Monotonicity of the radial distortion correction.
- Per-plane independence of TCA (modifying red doesn't affect blue).

## License

- **Upstream core code**: LGPL-3.0-or-later. A Rust port is a derivative work
  and **must remain LGPL-3.0-or-later**. You cannot relicense.
- **Upstream database**: CC-BY-SA 3.0. Separate from the code license.
  Attribution required; ShareAlike applies if you modify the DB.
- **Upstream `apps/` directory**: GPL-3.0 — do NOT read or derive from these
  (the `lenstool` CLI, for example). Stick to `libs/` and `include/` in the
  upstream source tree.

Practical implication (matches the rawler story from Prvw): the Rust port is
LGPL-3.0. Downstream consumers like Prvw, which ship public BSL source,
already satisfy LGPL's relinkability requirement.

Add a `NOTICE` file listing:
- Original LensFun attribution (copyright holders from `COPYING.LESSER`)
- CC-BY-SA attribution for the bundled DB
- Any third-party crates under non-MIT/Apache licenses

## Milestones / phases

Rough decomposition for a team lead. Each phase ends in a green CI run and a
clean tag.

### v0.1 — skeleton + database (1 week)

- Project scaffold matching mtp-rs layout.
- Port the public type surface: `Lens`, `Camera`, `Mount`, `CalibDistortion`,
  `CalibTca`, `CalibVignetting`, `Modifier`, plus their builders.
- XML parser consuming `data/db/*.xml`. Emit structured data.
- Port `test_database.cpp` scenarios.
- No correction math yet. The crate is a data loader.

### v0.2 — core distortion (2 weeks)

- Port all three distortion models (ptlens, poly3, poly5) with Newton
  iteration.
- Port `mod-coord` geometry conversions.
- Catmull-Rom spline interpolation.
- Pass `test_modifier_coord_*` tests.

### v0.3 — TCA and vignetting (1 week)

- `mod-subpix` (linear + poly3).
- `mod-color` vignetting.
- Pass `test_modifier_subpix`, `test_modifier_color`, `test_modifier_regression`.

### v0.4 — matching (1 week)

- `MatchScore`, `lfFuzzyStrCmp`, `GuessParameters`.
- End-to-end query: given camera + lens strings + focal + aperture, produce a
  ready-to-apply `Modifier`.
- Pass `test_lffuzzystrcmp` and end-to-end matching tests.

### v1.0 — polish + docs (1 week)

- Public API cleanup and documentation.
- `cargo publish` readiness (metadata, README, examples).
- Performance pass: measure against upstream C++, document results.
- NOTICE + license audit.

### Stretch — v1.1+

- Perspective correction (port `mod-pc.cpp` + SVD).
- Build-time database bundling.
- SIMD appliers (`wide` or `std::simd`).
- WASM target for web consumers.

**Total realistic budget: 6-8 focused weeks** for v1.0. Port of perspective
correction adds ~2 weeks if wanted later.

## Known risks and hidden complexity

1. **`lfLens::Interpolate*` (`lens.cpp:910-1292`)** — 4D spline with
   nearest-neighbor fallback. Easy to write, hard to get bit-exact with the
   C++ reference. Budget extra time and port `test_modifier_coord_distortion`
   scenarios early; they'll catch divergences.
2. **`lfFuzzyStrCmp` UTF-8 handling** — uses glib helpers. Test against
   `test_lffuzzystrcmp.cpp` with bit-exact expected output.
3. **`MatchScore` magic numbers** — 30+ ad-hoc weights. Don't simplify; port
   as-is, drive changes from test evidence only.
4. **Database schema quirks** — sparse calibration sets, conflicting entries
   across files, language tags. The XML itself has historical edge cases;
   mirror upstream's tolerance behaviors.
5. **Float determinism across platforms** — intermediate orderings in the C++
   may matter. Use the same FP operations in the same order; avoid
   rearranging algebra "for clarity" when the test expects a specific value.

## Non-goals

- Not a C ABI compatibility layer. If consumers need that, they wrap our Rust
  crate.
- Not a UI / CLI / GUI — separate projects.
- Not a DCP reader. Prvw handles DCP in its own pipeline.
- Not a replacement for `libraw` / `rawler` — lens correction only.

## Project setup

All scaffolding (check script, CI, clippy config, justfile, docs structure,
testing infrastructure, property-test scaffolds, release workflow) mirrors
`~/projects-git/vdavid/mtp-rs`. Copy from there and adapt. Specifically:

- `scripts/check.sh` — copy as-is, same output format and discipline.
- `AGENTS.md` — adapt the project description; keep the rule structure.
- `CONTRIBUTING.md` — adapt.
- `justfile` — adapt.
- `.github/workflows/` — copy CI, adapt paths.
- `clippy.toml`, `rustfmt.toml`, `deny.toml` — copy as-is.
- `docs/` layout — `architecture.md`, `style-guide.md`, `design-principles.md`,
  `notes/` subfolder — copy structure, populate with this-project content.
- Dependency style: pin to latest stable per `~/.claude/rules/use-latest-dep-versions.md`.

Do NOT copy anything from `src/` or `tests/` in mtp-rs — those are MTP-specific.

## Integration back into Prvw

Prvw's Phase 3.3+ roadmap entry ("Lens correction for non-DNG") becomes
"depend on `lensfun` crate from crates.io". The integration itself is a small
task:

1. Add `lensfun = "0.1"` (or whatever's published) to `apps/desktop/Cargo.toml`.
2. In `decoding/raw.rs`, after demosaic, look up the body + lens via rawler's
   metadata, fetch a Modifier from lensfun-rs, apply distortion + TCA +
   vignetting in-place.
3. Add a Settings toggle to let power users disable the correction.

Prvw's own scope stays small; the reusable work lives in the lensfun-rs crate
for the whole Rust ecosystem.

## Summary

- 7,756 LoC C++ core, no inheritance, few deps. Port is tractable.
- 4,495 LoC test suite is the executable spec. Port tests 1:1.
- 580 KB gzipped database. Ship with the crate or fetch at runtime.
- 6-8 weeks to v1.0. Perspective correction adds 2 more weeks.
- LGPL-3.0 on the port; Prvw's public BSL handles the relinkability.
- No existing Rust bindings anywhere. We'd be first.

Ship a solid v1, then the wider Rust imaging ecosystem benefits.
