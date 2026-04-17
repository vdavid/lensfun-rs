# Style guide

Standard Rust style with a few project-specific notes.

## Formatting

- `cargo fmt` is law. CI fails on any deviation.
- 100-column max width.
- Unix line endings.

## Linting

- `cargo clippy --all-targets --all-features -- -D warnings`. CI fails on any warning.
- Project-specific overrides live in `clippy.toml`.

## Naming

- Mirror upstream type and field names where it doesn't fight Rust convention. `lfLens` becomes `Lens`; `Cropfactor` becomes `crop_factor`; `CalibDistortion` stays `CalibDistortion`.
- Public free functions in math modules can mirror upstream names: `mod_coord::un_dist_poly3`, `auxfun::fuzzy_str_cmp`.

## Comments

- Default to **no comments**. Well-named identifiers explain *what* the code does.
- Add a comment when the *why* is non-obvious: a workaround, a hidden constraint, a port-specific quirk.
- For ports of upstream code, leave a short reference: `// Port of mod-coord.cpp:560-613`. This pays for itself when chasing a divergence.

## Tests

- **Integration tests (`tests/integration/`) are ports of upstream `test_*.cpp` files** — name them after the upstream file (e.g., `test_modifier_coord_distortion.cpp` → `tests/integration/modifier_coord_distortion.rs`). Treat the upstream expected values as gospel.
- **Property tests (`proptest`)** for round-trip identity, monotonicity, per-channel independence.
- Use `approx::assert_relative_eq!` for float comparisons, with the same tolerance the upstream test uses.
- Mark unimplemented ports `#[ignore = "...reason..."]`. Document why in the ignore reason.

## Float math

- Don't rearrange algebra "for clarity." Bit-exact match against upstream tests matters.
- Use `f64` everywhere upstream uses `double`. Use `f32` only where upstream uses `float` (mostly the per-pixel passes in `mod-color.cpp` and `mod-subpix.cpp`).
- Avoid `+= small_term_at_a_time` patterns where upstream computes `total = a + b + c` in one expression. The accumulation order matters for the last bit.

## Error handling

- One crate-level `Error` enum in `src/error.rs`.
- Math functions don't return `Result` — they panic on invalid inputs in debug builds via `debug_assert!`, return finite floats otherwise.
- I/O and parsing return `Result<T, Error>`.

## Module conventions

- One module per upstream `.cpp` file (`database.cpp` → `src/db.rs`, etc.).
- The mapping is documented in each module's top-level doc comment.
