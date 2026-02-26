//! TOML-backed schema types for binary file templates.
//!
//! Key types: [`Template`], [`Region`], [`Field`], and the expression enums
//! ([`OffsetExpr`], [`LengthExpr`]) with custom serde deserialization.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

/// Top-level template describing a binary file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Magic bytes as hex string, e.g. "89504E47"
    #[serde(default)]
    pub magic: Option<String>,
    #[serde(default)]
    pub magic_offset: u64,
    #[serde(default)]
    pub endian: Endianness,
    pub regions: Vec<Region>,
}

/// A contiguous region of bytes in the file (e.g., "PNG Header", "IHDR Chunk").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: String,
    pub label: String,
    /// Color as #RRGGBB hex string
    #[serde(default)]
    pub color: Option<String>,
    pub offset: OffsetExpr,
    #[serde(default)]
    pub length: Option<LengthExpr>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub fields: Vec<Field>,
    #[serde(default)]
    pub repeat: Option<RepeatMode>,
    /// Condition to evaluate — region is skipped if false
    #[serde(default)]
    pub condition: Option<ConditionExpr>,
    /// Field ID whose numeric value gives the repeat count (for `RepeatMode::Count`)
    #[serde(default)]
    pub repeat_count: Option<String>,
    /// Hex byte string sentinel to stop repeating (for `RepeatMode::UntilMagic`)
    #[serde(default)]
    pub repeat_until: Option<String>,
}

/// How a region should be repeated when resolving.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// Repeat a fixed number of times (requires `repeat_count` field reference).
    Count,
    /// Repeat until the end of the file.
    UntilEof,
    /// Repeat until a sentinel byte sequence is found (requires `repeat_until`).
    UntilMagic,
}

/// Binary arithmetic expression: `left op right`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArithExpr {
    pub left: Operand,
    pub op: ArithOp,
    pub right: Operand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operand {
    Literal(u64),
    FieldRef(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Condition expression: `field_id op value`.
#[derive(Debug, Clone, Serialize)]
pub struct ConditionExpr {
    pub field_id: String,
    pub op: CompareOp,
    pub value: u64,
}

/// Comparison operators for condition expressions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// A single field within a region (e.g., "width: u32le").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    pub length: LengthExpr,
    /// Relative offset within region (fields are sequential if omitted)
    #[serde(default)]
    pub offset: Option<u64>,
    pub role: Option<FieldRole>,
    #[serde(default)]
    pub description: Option<String>,
    /// Condition to evaluate — field is skipped if false
    #[serde(default)]
    pub condition: Option<ConditionExpr>,
    /// Map of numeric value → display label (e.g. "8" → "Deflated")
    #[serde(default)]
    pub enum_values: Option<HashMap<String, String>>,
    /// Map of bit index → flag name (e.g. "0" → "Encrypted")
    #[serde(default)]
    pub bit_flags: Option<HashMap<String, String>>,
    /// Optional color as #RRGGBB hex string — overrides region color in hex view
    #[serde(default)]
    pub color: Option<String>,
    /// Arithmetic expression for computed fields (e.g. "expr:field_a * 512")
    #[serde(default)]
    pub expression: Option<String>,
    /// Template name to auto-apply at the computed value offset
    #[serde(default)]
    pub apply_template: Option<String>,
}

/// Offset expression: integer for absolute, "after:id" for AfterField, "from:id" for FromField,
/// "expr:a * b" for arithmetic.
#[derive(Debug, Clone, Serialize)]
pub enum OffsetExpr {
    Absolute(u64),
    AfterField(String),
    FromField(String),
    Expr(ArithExpr),
}

impl<'de> Deserialize<'de> for OffsetExpr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = toml::Value::deserialize(deserializer)?;
        match &value {
            toml::Value::Integer(n) => {
                Ok(OffsetExpr::Absolute(*n as u64))
            }
            toml::Value::String(s) => {
                if let Some(id) = s.strip_prefix("after:") {
                    Ok(OffsetExpr::AfterField(id.to_string()))
                } else if let Some(id) = s.strip_prefix("from:") {
                    Ok(OffsetExpr::FromField(id.to_string()))
                } else if let Some(body) = s.strip_prefix("expr:") {
                    parse_arith_expr(body).map(OffsetExpr::Expr).map_err(serde::de::Error::custom)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "invalid offset expression: {s}"
                    )))
                }
            }
            _ => Err(serde::de::Error::custom("offset must be integer or string")),
        }
    }
}

/// Length expression: integer for fixed, "to_end" for ToEnd, "from:id" for FromField,
/// "expr:a * b" for arithmetic.
#[derive(Debug, Clone, Serialize)]
pub enum LengthExpr {
    Fixed(u64),
    FromField(String),
    ToEnd,
    Expr(ArithExpr),
}

impl<'de> Deserialize<'de> for LengthExpr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = toml::Value::deserialize(deserializer)?;
        match &value {
            toml::Value::Integer(n) => Ok(LengthExpr::Fixed(*n as u64)),
            toml::Value::String(s) => {
                if s == "to_end" {
                    Ok(LengthExpr::ToEnd)
                } else if let Some(id) = s.strip_prefix("from:") {
                    Ok(LengthExpr::FromField(id.to_string()))
                } else if let Some(body) = s.strip_prefix("expr:") {
                    parse_arith_expr(body).map(LengthExpr::Expr).map_err(serde::de::Error::custom)
                } else {
                    Err(serde::de::Error::custom(format!(
                        "invalid length expression: {s}"
                    )))
                }
            }
            _ => Err(serde::de::Error::custom("length must be integer or string")),
        }
    }
}

/// Default byte order for the template. Individual fields can override this.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Endianness {
    #[default]
    Little,
    Big,
}

/// Primitive data type of a field, used for both byte decoding and display formatting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    U8,
    U16Le,
    U16Be,
    U32Le,
    U32Be,
    U64Le,
    U64Be,
    I8,
    I16Le,
    I16Be,
    I32Le,
    I32Be,
    I64Le,
    I64Be,
    F32Le,
    F32Be,
    F64Le,
    F64Be,
    Bytes,
    Utf8,
    Ascii,
    Computed,
}

impl FieldType {
    /// Returns the natural byte size for fixed-width types, None for variable types.
    pub fn natural_size(&self) -> Option<u64> {
        match self {
            FieldType::U8 | FieldType::I8 => Some(1),
            FieldType::U16Le | FieldType::U16Be | FieldType::I16Le | FieldType::I16Be => Some(2),
            FieldType::U32Le | FieldType::U32Be | FieldType::I32Le | FieldType::I32Be
            | FieldType::F32Le | FieldType::F32Be => Some(4),
            FieldType::U64Le | FieldType::U64Be | FieldType::I64Le | FieldType::I64Be
            | FieldType::F64Le | FieldType::F64Be => Some(8),
            FieldType::Bytes | FieldType::Utf8 | FieldType::Ascii | FieldType::Computed => None,
        }
    }
}

/// Semantic role of a field, used for display hints in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldRole {
    Magic,
    Version,
    Size,
    Offset,
    Count,
    Checksum,
    Padding,
    Reserved,
    Data,
}

/// Parse an operand: integer literal (decimal or 0x hex) or field ID string.
fn parse_operand(s: &str) -> Result<Operand, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty operand".into());
    }
    // Try hex literal
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16)
            .map(Operand::Literal)
            .map_err(|e| format!("invalid hex literal '{s}': {e}"));
    }
    // Try decimal literal (starts with digit)
    if s.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
        return s
            .parse::<u64>()
            .map(Operand::Literal)
            .map_err(|e| format!("invalid integer literal '{s}': {e}"));
    }
    // Otherwise it's a field reference
    Ok(Operand::FieldRef(s.to_string()))
}

/// Parse an arithmetic expression like `"field_a * 2048"` or `"field + field_b"`.
/// Operators must be space-delimited: ` + `, ` - `, ` * `, ` / `.
fn parse_arith_expr(s: &str) -> Result<ArithExpr, String> {
    let s = s.trim();
    // Try each operator (space-delimited to avoid ambiguity with field names containing `-`)
    for (token, op) in [(" * ", ArithOp::Mul), (" / ", ArithOp::Div), (" + ", ArithOp::Add), (" - ", ArithOp::Sub)] {
        if let Some(pos) = s.find(token) {
            let left = &s[..pos];
            let right = &s[pos + token.len()..];
            return Ok(ArithExpr {
                left: parse_operand(left)?,
                op,
                right: parse_operand(right)?,
            });
        }
    }
    Err(format!("no space-delimited operator found in expression: '{s}'"))
}

/// Public entry point for parsing arithmetic expressions.
pub fn parse_arith_expr_public(s: &str) -> Result<ArithExpr, String> {
    parse_arith_expr(s)
}

/// Custom deserialize for ConditionExpr from strings like `"color_type == 3"` or `"version >= 0x02"`.
impl<'de> Deserialize<'de> for ConditionExpr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        parse_condition_expr(&s).map_err(serde::de::Error::custom)
    }
}

fn parse_condition_expr(s: &str) -> Result<ConditionExpr, String> {
    // Try operators longest-first to avoid `<` matching before `<=`
    for (token, op) in [
        (" <= ", CompareOp::Le),
        (" >= ", CompareOp::Ge),
        (" != ", CompareOp::Ne),
        (" == ", CompareOp::Eq),
        (" < ", CompareOp::Lt),
        (" > ", CompareOp::Gt),
    ] {
        if let Some(pos) = s.find(token) {
            let field_id = s[..pos].trim().to_string();
            let value_str = s[pos + token.len()..].trim();
            let value = if let Some(hex) = value_str.strip_prefix("0x").or_else(|| value_str.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16)
                    .map_err(|e| format!("invalid hex value '{value_str}': {e}"))?
            } else {
                value_str
                    .parse::<u64>()
                    .map_err(|e| format!("invalid integer value '{value_str}': {e}"))?
            };
            return Ok(ConditionExpr { field_id, op, value });
        }
    }
    Err(format!("no comparison operator found in condition: '{s}'"))
}
