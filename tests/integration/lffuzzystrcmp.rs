//! Port of `tests/test_lffuzzystrcmp.cpp`. Each upstream `g_assert_cmpint` becomes a Rust
//! `assert!` / `assert_eq!`. Same input strings, same expected scores.

use lensfun::FuzzyStrCmp;

#[test]
fn dot_zero_missing_in_raw() {
    // Pattern: name in RAW file. Target: name in database file.
    let cmp = FuzzyStrCmp::new("Nikkor 18mm f/4 DX", true);
    let score = cmp.compare("Nikkor 18mm f/4.0 DX");
    assert!(score > 0, "expected score > 0, got {score}");
}

#[test]
fn dot_zero_missing_in_db() {
    let cmp = FuzzyStrCmp::new("Nikkor 18mm f/4.0 DX", true);
    let score = cmp.compare("Nikkor 18mm f/4 DX");
    assert!(score > 0, "expected score > 0, got {score}");
}
