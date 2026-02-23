use serde::{Deserialize, Deserializer, Serialize};

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
}

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
    /// ID of another field whose value gives the size of this field
    #[serde(default)]
    pub size_target: Option<String>,
}

/// Offset expression: integer for absolute, "after:id" for AfterField, "from:id" for FromField.
#[derive(Debug, Clone, Serialize)]
pub enum OffsetExpr {
    Absolute(u64),
    AfterField(String),
    FromField(String),
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

/// Length expression: integer for fixed, "to_end" for ToEnd, "from:id" for FromField.
#[derive(Debug, Clone, Serialize)]
pub enum LengthExpr {
    Fixed(u64),
    FromField(String),
    ToEnd,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Endianness {
    #[default]
    Little,
    Big,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            FieldType::Bytes | FieldType::Utf8 | FieldType::Ascii => None,
        }
    }
}

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
