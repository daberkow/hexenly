use crate::schema::{FieldRole, FieldType};

/// GUI-agnostic color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TemplateColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a "#RRGGBB" hex string into a color.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }
}

impl Default for TemplateColor {
    fn default() -> Self {
        Self {
            r: 100,
            g: 150,
            b: 200,
        }
    }
}

/// A fully-resolved template ready for rendering.
#[derive(Debug, Clone)]
pub struct ResolvedTemplate {
    pub name: String,
    pub description: String,
    pub regions: Vec<ResolvedRegion>,
}

/// A region with concrete absolute byte offsets.
#[derive(Debug, Clone)]
pub struct ResolvedRegion {
    pub id: String,
    pub label: String,
    pub color: TemplateColor,
    pub offset: u64,
    pub length: u64,
    pub group: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<ResolvedField>,
}

impl ResolvedRegion {
    /// Whether the given byte offset falls within this region.
    pub fn contains(&self, byte_offset: u64) -> bool {
        byte_offset >= self.offset && byte_offset < self.offset + self.length
    }

    /// Exclusive end offset (one past the last byte).
    pub fn end_exclusive(&self) -> u64 {
        self.offset + self.length
    }
}

/// A field with concrete absolute byte offset and display value.
#[derive(Debug, Clone)]
pub struct ResolvedField {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    pub offset: u64,
    pub length: u64,
    pub role: Option<FieldRole>,
    pub description: Option<String>,
    pub raw_bytes: Vec<u8>,
    pub display_value: String,
    /// Optional per-field color — overrides region color in hex view.
    pub color: Option<TemplateColor>,
}
