/// A search pattern that can match either raw hex bytes or UTF-8 text.
#[derive(Debug, Clone)]
pub enum SearchPattern {
    /// Raw byte sequence (e.g., parsed from "DE AD BE EF").
    HexBytes(Vec<u8>),
    /// UTF-8 text matched against the file's raw bytes.
    Text(String),
}

impl SearchPattern {
    /// Parse a hex string like `"DE AD BE EF"` into a byte pattern.
    /// Whitespace is stripped; returns `None` if the string has an odd number of hex digits
    /// or contains invalid hex characters.
    pub fn from_hex_string(s: &str) -> Option<Self> {
        let hex: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if !hex.len().is_multiple_of(2) {
            return None;
        }
        let bytes: Result<Vec<u8>, _> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect();
        bytes.ok().map(SearchPattern::HexBytes)
    }

    /// Create a text search pattern.
    pub fn from_text(s: &str) -> Self {
        SearchPattern::Text(s.to_string())
    }

    /// The raw bytes to search for.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            SearchPattern::HexBytes(bytes) => bytes,
            SearchPattern::Text(text) => text.as_bytes(),
        }
    }
}

/// Find the next occurrence of `pattern` in `data`, starting at `start`.
/// Wraps around to the beginning if no match is found after `start`.
pub fn find_next(data: &[u8], pattern: &SearchPattern, start: usize) -> Option<usize> {
    let needle = pattern.as_bytes();
    if needle.is_empty() || needle.len() > data.len() {
        return None;
    }
    let start = start.min(data.len());
    // Search from start to end
    for i in start..=data.len().saturating_sub(needle.len()) {
        if &data[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    // Wrap around: search from beginning to start
    let wrap_end = start.min(data.len().saturating_sub(needle.len()));
    (0..=wrap_end).find(|&i| &data[i..i + needle.len()] == needle)
}

/// Find the previous occurrence of `pattern` in `data`, searching backwards from `start`.
/// Wraps around to the end if no match is found before `start`.
pub fn find_prev(data: &[u8], pattern: &SearchPattern, start: usize) -> Option<usize> {
    let needle = pattern.as_bytes();
    if needle.is_empty() || needle.len() > data.len() {
        return None;
    }
    let max_pos = data.len() - needle.len();
    let start = start.min(max_pos);
    // Search backwards from start
    for i in (0..=start).rev() {
        if &data[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    // Wrap around: search backwards from end
    (start..=max_pos).rev().find(|&i| &data[i..i + needle.len()] == needle)
}

/// Find all non-overlapping occurrences of `pattern` in `data`, up to `limit` results.
pub fn find_all(data: &[u8], pattern: &SearchPattern, limit: usize) -> Vec<usize> {
    let needle = pattern.as_bytes();
    if needle.is_empty() || needle.len() > data.len() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut i = 0;
    while i <= data.len() - needle.len() && results.len() < limit {
        if &data[i..i + needle.len()] == needle {
            results.push(i);
            i += needle.len(); // non-overlapping
        } else {
            i += 1;
        }
    }
    results
}
