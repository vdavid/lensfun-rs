# Contributing to lensfun-rs

Thanks for considering contributing! This is a pure-Rust port of [LensFun](https://github.com/lensfun/lensfun). The project is read-only relative to upstream — we mirror the upstream library's behavior, not invent new heuristics.

## Getting started

```bash
git clone https://github.com/vdavid/lensfun-rs
cd lensfun-rs
cargo build
cargo test
```

For working on the port, also clone upstream into `related-repos/` (gitignored):

```bash
mkdir -p related-repos
git clone --depth 1 https://github.com/lensfun/lensfun.git related-repos/lensfun
```

The upstream C++ source lives in `related-repos/lensfun/libs/lensfun/`. The upstream tests in `related-repos/lensfun/tests/test_*.cpp` are the executable spec — port them 1:1 to `tests/integration/<name>.rs`.

### Dev tools

We use [`just`](https://github.com/casey/just) as a command runner:

```bash
just            # Run all checks: format, lint, test, doc
just fix        # Auto-fix formatting and clippy warnings
just check-all  # Include MSRV check, security audit, license check
```

Run `just --list` to see all available commands.

### MSRV

We support Rust 1.85. Before submitting PRs, verify MSRV compatibility:

```bash
rustup toolchain install 1.85.0   # One-time setup
just msrv                          # Check MSRV compatibility
```

## How to contribute a port

The porting process for any C++ source file:

1. **Find the upstream source** in `related-repos/lensfun/libs/lensfun/`.
2. **Find the matching test** in `related-repos/lensfun/tests/`.
3. **Port the test first** to `tests/integration/<name>.rs`. Mark unimplemented assertions with `#[ignore]`.
4. **Port the implementation** to `src/<module>.rs`. Mirror the upstream function names and signatures where it makes sense; idiomatic Rust where it doesn't.
5. **Run the test until it passes.** Don't simplify the upstream heuristic — bit-exact match is the goal.

When in doubt, check the upstream test's expected outputs. If you can't match them, raise a question — don't paper over it.

## What we want

- Faithful ports of upstream modules, in milestone order (see `docs/notes/lensfun-rs.md`).
- Test coverage that mirrors `related-repos/lensfun/tests/test_*.cpp`.
- Property tests on top of the ports (round-trip identity, monotonicity, per-channel independence).

## What we don't want

- "Improvements" to upstream heuristics (the magic numbers in `MatchScore` are load-bearing).
- SIMD in v1 — defer to a later milestone.
- `SaveXML` / database authoring — this crate is read-only.
- A CLI / GUI — separate projects.

## License headers

Source files do **not** need a per-file license header. The crate is LGPL-3.0-or-later, recorded in `Cargo.toml` and (when added) the top-level license file.

## Submitting changes

1. Fork and create a branch.
2. Make your changes.
3. Run `just` (format, clippy, test, doc).
4. Run `just msrv` to verify Rust 1.85 compatibility.
5. Open a PR with a clear description, including the upstream source and test files you mirrored.

For non-trivial changes, open an issue first to discuss the approach.

## Questions?

Open an issue. Happy to chat.
