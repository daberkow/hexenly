/// An inclusive byte range selection in the hex view.
///
/// `start` is always <= `end`; the constructor normalizes swapped values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// First selected byte offset.
    pub start: usize,
    /// Last selected byte offset (inclusive).
    pub end: usize,
}

impl Selection {
    /// Create a selection, normalizing so `start <= end`.
    pub fn new(start: usize, end: usize) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self { start, end }
    }

    /// A single-byte selection (start == end).
    pub fn single(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Whether `offset` falls within this selection (inclusive on both ends).
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }

    /// Number of selected bytes.
    pub fn len(&self) -> usize {
        self.end - self.start + 1
    }

    /// Always `false` — a selection covers at least one byte.
    pub fn is_empty(&self) -> bool {
        false
    }
}

use serde::{Deserialize, Serialize};

/// A named marker at a byte offset or range, persisted across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub offset: usize,
    /// End of range (inclusive). If `None`, bookmark is a single offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    #[serde(default)]
    pub note: String,
}

impl Bookmark {
    /// Number of bytes covered by this bookmark.
    pub fn len(&self) -> usize {
        match self.end {
            Some(end) => end - self.offset + 1,
            None => 1,
        }
    }

    /// Always `false` — a bookmark covers at least one byte.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Whether `offset` falls within this bookmark's range.
    pub fn contains(&self, offset: usize) -> bool {
        match self.end {
            Some(end) => offset >= self.offset && offset <= end,
            None => offset == self.offset,
        }
    }
}
