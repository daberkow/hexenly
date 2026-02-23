#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

impl Selection {
    pub fn new(start: usize, end: usize) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self { start, end }
    }

    pub fn single(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

use serde::{Deserialize, Serialize};

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
    pub fn len(&self) -> usize {
        match self.end {
            Some(end) => end - self.offset + 1,
            None => 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        false // a bookmark always covers at least one byte
    }

    pub fn contains(&self, offset: usize) -> bool {
        match self.end {
            Some(end) => offset >= self.offset && offset <= end,
            None => offset == self.offset,
        }
    }
}
