# tests/cpp-vs-rust

This directory contains the C++ probe binary and the Rust harness for A/B-testing
the Rust port of lensfun against the upstream C++ library.

## Contents

| File | Purpose |
|---|---|
| `probe.cpp` | Thin C++ shim over upstream lensfun. Reads commands from stdin, writes results to stdout. |
| `build.sh` | Compiles `probe.cpp` into the `probe` binary. Run once before testing. |
| `probe` | Compiled binary (not committed). |

## Building the probe

```bash
bash tests/cpp-vs-rust/build.sh
```

Requires upstream lensfun built at `related-repos/lensfun/build/` (Step A).

## Protocol

`probe` reads TAB-separated commands from stdin, one per line. EOF → exit 0.
String fields (`maker`, `model`) are quote-delimited so they can contain spaces.

### distortion

```
distortion  "<maker>"  "<model>"  <focal>  <crop>  <width>  <height>  <reverse>  <x>  <y>
→  <x_out>\t<y_out>
```

### tca

```
tca  "<maker>"  "<model>"  <focal>  <crop>  <width>  <height>  <reverse>  <x>  <y>
→  <x_red>\t<y_red>\t<x_blue>\t<y_blue>
```

### vignetting

```
vignetting  "<maker>"  "<model>"  <focal>  <aperture>  <distance>  <crop>  <width>  <height>  <x>  <y>
→  <gain>
```

`<reverse>` is `0` or `1`. Floats are printed with `%.10g`.
If no lens matches, `probe` writes `nan\t...` to stdout and an error to stderr, then continues.

## Smoke test

```bash
printf 'distortion\t"Sony"\t"E PZ 16-50mm f/3.5-5.6 OSS"\t35\t1.5\t6000\t4000\t1\t100\t100\n' \
  | tests/cpp-vs-rust/probe
# → 118.5959167   112.182373
```

## Throughput benchmark

Pass `--bench` to run two benchmarks back-to-back:

1. **Row-batched throughput (lead measurement, production-shape)** — calls `apply_*` once per row of `width` pixels, mirroring how Prvw, darktable, and other consumers actually invoke the API. Reports M pixels/s per kernel and a combined full-stack rate (1 / sum of reciprocals) plus the implied ms-for-20-MP-image budget. **This is the number to use for capacity planning.**

2. **Single-pixel call overhead (diagnostic)** — calls `apply_*(x, y, 1, 1, ...)` per pixel. Pays per-call overhead (slice bounds checks, coordinate normalization) on every pixel, so it measures call overhead, not kernel throughput. Useful for spotting overhead regressions in either side. **Do not use for capacity planning — production code never calls this shape.**

Both modifiers are built once outside the timed loops, with a warmup pass before timing. Hoisting is prevented via `std::hint::black_box` (Rust) and `asm volatile("" : : "r"(...) : "memory")` (C++).

```bash
cargo run --release --example ab_compare -- --bench
# Or via just:
just --justfile tests/cpp-vs-rust/justfile bench
```

### Why two modes

The previous single-mode bench reported "Rust is 2.84× slower at distortion" — true for the per-call overhead path, but misleading as a real-world performance claim because production callers process whole rows, not individual pixels. The row-batched mode reflects production reality: under per-row calls the Rust port is actually faster than upstream on every kernel.

## Running the A/B harness

The harness lives in `tests/cpp-vs-rust/harness.rs` and is registered as a Cargo example
(`ab_compare`). It spawns one long-running probe process, feeds it commands, runs the same
inputs through the Rust crate, and compares the results within a 1e-3 tolerance.

```bash
# From the repo root:
cargo run --release --example ab_compare
```

Or via `just` (from the repo root or from this directory with the `--justfile` flag):

```bash
just --justfile tests/cpp-vs-rust/justfile compare
# Or rebuild the probe first:
just --justfile tests/cpp-vs-rust/justfile probe compare
```

### Example output

```
Running 1640 cases...

── lensfun-rs vs upstream LensFun C++ ──
distortion: 1080 cases, 1080 ok, 0 fail, max abs delta = 4.883e-4
tca       : 420 cases, 420 ok, 0 fail, max abs delta = 4.883e-4
vignetting: 140 cases, 140 ok, 0 fail, max abs delta = 2.146e-6

✓ all under 1e-3 tolerance
```

The sweep covers 6 lenses (Sony E PZ 16-50mm, Canon EF 16-35mm f/2.8L III, Canon EF 24-70mm
f/2.8L II, Pentax DA 50-200mm, Canon EF 50mm f/1.4, Samyang 7.5mm fisheye MFT) across
4-5 focal lengths each, 30 coordinates per focal (5×5 grid + 4 corners + center), with
forward and reverse distortion. Total: 1640 cases in under 2 seconds.

Exit code 0 means all cases passed. Nonzero means at least one case exceeded tolerance,
with failing case details printed to stdout.
