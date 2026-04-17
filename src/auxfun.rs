//! Auxiliary helpers: fuzzy string compare + Catmull-Rom spline interpolation.
//!
//! Port of `libs/lensfun/auxfun.cpp`.

use std::cmp::Ordering;

// interpolate (v0.2): port from auxfun.cpp:335.
//   Catmull-Rom spline across focal-length axis (and aperture axis for vignetting).
//   ~25 LoC.

/// Fuzzy string comparator for lens and camera model names.
///
/// Port of `lfFuzzyStrCmp` (auxfun.cpp:360-540). At construction, the pattern is split into
/// words; each call to [`compare`](Self::compare) splits the target the same way and scores
/// the overlap in range 0-100.
///
/// `all_words = true` requires every pattern word to be present in the target (a missing word
/// short-circuits to 0). `false` accepts looser matches at a lower score.
pub struct FuzzyStrCmp {
    pattern_words: Vec<String>,
    match_all_words: bool,
}

impl FuzzyStrCmp {
    /// Build a comparator for `pattern`.
    ///
    /// If `all_words` is true, every word in the pattern must appear in the target for the
    /// match to score above 0.
    pub fn new(pattern: &str, all_words: bool) -> Self {
        let mut pattern_words = Vec::new();
        split(pattern, &mut pattern_words);
        Self {
            pattern_words,
            match_all_words: all_words,
        }
    }

    /// Score `target` against the pattern. Returns 0-100.
    ///
    /// Score = `2 * matches / (pattern_word_count + target_word_count) * 100` (integer).
    pub fn compare(&self, target: &str) -> i32 {
        let mut match_words = Vec::new();
        split(target, &mut match_words);

        if match_words.is_empty() || self.pattern_words.is_empty() {
            return 0;
        }

        let mut mi: usize = 0;
        let mut score: i32 = 0;

        for pattern_str in &self.pattern_words {
            let old_mi = mi;
            let mut found_match = false;

            while mi < match_words.len() {
                match pattern_str.as_str().cmp(match_words[mi].as_str()) {
                    Ordering::Equal => {
                        score += 1;
                        found_match = true;
                        break;
                    }
                    Ordering::Less => {
                        // Sorted arrays: pattern word smaller than current match word means no
                        // match is possible further on. Bail or reset, depending on mode.
                        if self.match_all_words {
                            return 0;
                        }
                        break;
                    }
                    Ordering::Greater => mi += 1,
                }
            }

            if self.match_all_words {
                if !found_match {
                    return 0;
                }
                mi += 1;
            } else if found_match {
                mi += 1;
            } else {
                mi = old_mi;
            }
        }

        (score * 200) / (self.pattern_words.len() + match_words.len()) as i32
    }
}

/// Convenience wrapper around [`FuzzyStrCmp`] for one-shot comparisons.
///
/// Uses `all_words = false` (looser matching). For repeated comparisons against the same
/// pattern, build a [`FuzzyStrCmp`] once and reuse it.
pub fn fuzzy_str_cmp(pattern: &str, target: &str) -> i32 {
    FuzzyStrCmp::new(pattern, false).compare(target)
}

// Mirrors `lfFuzzyStrCmp::Split` (auxfun.cpp:382). Walks `str` byte-by-byte using ASCII
// character classes (matching the C++ `(unsigned char)` casts to ctype predicates), splits
// on character-class boundaries, casefolds each word, and inserts it into `dest` in sorted
// order.
fn split(str: &str, dest: &mut Vec<String>) {
    let bytes = str.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let word_start = i;
        let first = bytes[i];
        i += 1;

        let mut strip_suffix = 0;

        if first.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i - word_start >= 2 && bytes[i - 2] == b'.' && bytes[i - 1] == b'0' {
                strip_suffix = 2;
            }
        } else if is_ascii_punct(first) {
            while i < bytes.len() && is_ascii_punct(bytes[i]) {
                i += 1;
            }
        } else {
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && !bytes[i].is_ascii_digit()
                && !is_ascii_punct(bytes[i])
            {
                i += 1;
            }
        }

        // Skip lone punctuation and a lone "f"/"F", but keep "*" and "+" since lens model
        // names use them as significant characters.
        if i - word_start == 1
            && (is_ascii_punct(first) || first.eq_ignore_ascii_case(&b'f'))
            && first != b'*'
            && first != b'+'
        {
            continue;
        }

        let raw = &str[word_start..(i - strip_suffix)];
        let folded = raw.to_lowercase();
        let pos = dest.binary_search(&folded).unwrap_or_else(|p| p);
        dest.insert(pos, folded);
    }
}

// Mirrors C `ispunct` for ASCII: any printable non-alphanumeric, non-space character.
fn is_ascii_punct(b: u8) -> bool {
    b.is_ascii_graphic() && !b.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic() {
        let mut words = Vec::new();
        split("Nikkor 18mm f/4 DX", &mut words);
        assert_eq!(words, vec!["18", "4", "dx", "mm", "nikkor"]);
    }

    #[test]
    fn split_strips_dot_zero_on_digit_run() {
        let mut words = Vec::new();
        split("Nikkor 18mm f/4.0 DX", &mut words);
        assert_eq!(words, vec!["18", "4", "dx", "mm", "nikkor"]);
    }

    #[test]
    fn split_keeps_star_and_plus() {
        let mut words = Vec::new();
        split("EF 50 *", &mut words);
        assert!(words.contains(&"*".to_string()));
        let mut words = Vec::new();
        split("X 50 +", &mut words);
        assert!(words.contains(&"+".to_string()));
    }

    #[test]
    fn empty_pattern_returns_zero() {
        let cmp = FuzzyStrCmp::new("", true);
        assert_eq!(cmp.compare("anything"), 0);
    }

    #[test]
    fn empty_target_returns_zero() {
        let cmp = FuzzyStrCmp::new("anything", true);
        assert_eq!(cmp.compare(""), 0);
    }

    #[test]
    fn perfect_match_scores_100() {
        let cmp = FuzzyStrCmp::new("Nikkor 18mm f/4 DX", true);
        assert_eq!(cmp.compare("Nikkor 18mm f/4 DX"), 100);
    }
}
