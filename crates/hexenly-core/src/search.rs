#[derive(Debug, Clone)]
pub enum SearchPattern {
    HexBytes(Vec<u8>),
    Text(String),
}

impl SearchPattern {
    pub fn from_hex_string(s: &str) -> Option<Self> {
        let hex: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if hex.len() % 2 != 0 {
            return None;
        }
        let bytes: Result<Vec<u8>, _> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect();
        bytes.ok().map(SearchPattern::HexBytes)
    }

    pub fn from_text(s: &str) -> Self {
        SearchPattern::Text(s.to_string())
    }

    fn needle(&self) -> &[u8] {
        match self {
            SearchPattern::HexBytes(bytes) => bytes,
            SearchPattern::Text(text) => text.as_bytes(),
        }
    }
}

pub fn find_next(data: &[u8], pattern: &SearchPattern, start: usize) -> Option<usize> {
    let needle = pattern.needle();
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

pub fn find_prev(data: &[u8], pattern: &SearchPattern, start: usize) -> Option<usize> {
    let needle = pattern.needle();
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

pub fn find_all(data: &[u8], pattern: &SearchPattern, limit: usize) -> Vec<usize> {
    let needle = pattern.needle();
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
