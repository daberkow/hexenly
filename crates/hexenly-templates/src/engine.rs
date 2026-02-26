//! Template resolution engine — resolves parsed templates against file bytes.
//!
//! Walks the template's regions and fields, evaluates dynamic expressions
//! (field references, arithmetic, conditions, repeats), and produces a
//! [`ResolvedTemplate`] with concrete absolute offsets and formatted display values.

use std::collections::HashMap;

use crate::resolved::{ResolvedField, ResolvedRegion, ResolvedTemplate, TemplateColor, TemplateLink};
use crate::schema::{
    ArithExpr, ArithOp, CompareOp, ConditionExpr, FieldType, LengthExpr, OffsetExpr, Operand,
    RepeatMode, Template,
};

/// The result of resolving a template: the resolved overlay plus any warnings.
#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub template: ResolvedTemplate,
    pub warnings: Vec<ResolveWarning>,
    pub template_links: Vec<TemplateLink>,
}

/// A non-fatal issue encountered during resolution (e.g., out-of-bounds offset).
#[derive(Debug, Clone)]
pub struct ResolveWarning {
    pub message: String,
}

impl std::fmt::Display for ResolveWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Tracked info about a resolved field or region, used for cross-references.
#[derive(Debug, Clone)]
struct ResolvedFieldInfo {
    offset: u64,
    length: u64,
    numeric_value: Option<u64>,
}

/// Extract a numeric value from raw bytes based on field type.
fn extract_numeric_value(field_type: &FieldType, raw: &[u8]) -> Option<u64> {
    match field_type {
        FieldType::U8 => raw.first().map(|&b| b as u64),
        FieldType::I8 => raw.first().map(|&b| b as i8 as u64),
        FieldType::U16Le if raw.len() >= 2 => {
            Some(u16::from_le_bytes([raw[0], raw[1]]) as u64)
        }
        FieldType::U16Be if raw.len() >= 2 => {
            Some(u16::from_be_bytes([raw[0], raw[1]]) as u64)
        }
        FieldType::I16Le if raw.len() >= 2 => {
            Some(i16::from_le_bytes([raw[0], raw[1]]) as u64)
        }
        FieldType::I16Be if raw.len() >= 2 => {
            Some(i16::from_be_bytes([raw[0], raw[1]]) as u64)
        }
        FieldType::U32Le if raw.len() >= 4 => {
            Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64)
        }
        FieldType::U32Be if raw.len() >= 4 => {
            Some(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64)
        }
        FieldType::I32Le if raw.len() >= 4 => {
            Some(i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64)
        }
        FieldType::I32Be if raw.len() >= 4 => {
            Some(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64)
        }
        FieldType::U64Le if raw.len() >= 8 => {
            Some(u64::from_le_bytes(raw[..8].try_into().unwrap()))
        }
        FieldType::U64Be if raw.len() >= 8 => {
            Some(u64::from_be_bytes(raw[..8].try_into().unwrap()))
        }
        FieldType::I64Le if raw.len() >= 8 => {
            Some(i64::from_le_bytes(raw[..8].try_into().unwrap()) as u64)
        }
        FieldType::I64Be if raw.len() >= 8 => {
            Some(i64::from_be_bytes(raw[..8].try_into().unwrap()) as u64)
        }
        _ => None,
    }
}

/// Parse a hex string like "504B0304" into bytes.
fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Evaluate an arithmetic expression using resolved field values.
fn eval_arith_expr(expr: &ArithExpr, field_map: &HashMap<String, ResolvedFieldInfo>) -> Option<u64> {
    let resolve_operand = |op: &Operand| -> Option<u64> {
        match op {
            Operand::Literal(n) => Some(*n),
            Operand::FieldRef(id) => field_map.get(id.as_str()).and_then(|info| info.numeric_value),
        }
    };
    let left = resolve_operand(&expr.left)?;
    let right = resolve_operand(&expr.right)?;
    match expr.op {
        ArithOp::Add => left.checked_add(right),
        ArithOp::Sub => left.checked_sub(right),
        ArithOp::Mul => left.checked_mul(right),
        ArithOp::Div => {
            if right == 0 {
                None
            } else {
                left.checked_div(right)
            }
        }
    }
}

/// Evaluate a condition expression against resolved field values.
fn eval_condition(
    cond: &ConditionExpr,
    field_map: &HashMap<String, ResolvedFieldInfo>,
) -> Option<bool> {
    let info = field_map.get(cond.field_id.as_str())?;
    let val = info.numeric_value?;
    Some(match cond.op {
        CompareOp::Eq => val == cond.value,
        CompareOp::Ne => val != cond.value,
        CompareOp::Lt => val < cond.value,
        CompareOp::Gt => val > cond.value,
        CompareOp::Le => val <= cond.value,
        CompareOp::Ge => val >= cond.value,
    })
}

/// Enrich a display value with enum label if the numeric value matches a key.
fn format_enum_display(
    base: &str,
    numeric_value: Option<u64>,
    enum_values: &std::collections::HashMap<String, String>,
) -> String {
    if let Some(val) = numeric_value {
        let key = val.to_string();
        if let Some(label) = enum_values.get(&key) {
            return format!("{base} ({label})");
        }
    }
    base.to_string()
}

/// Enrich a display value with bit flag names for set bits.
fn format_bitflags_display(
    base: &str,
    numeric_value: Option<u64>,
    bit_flags: &std::collections::HashMap<String, String>,
) -> String {
    if let Some(val) = numeric_value {
        let mut names: Vec<(u8, &str)> = Vec::new();
        for (key, name) in bit_flags {
            if let Ok(bit_idx) = key.parse::<u8>()
                && bit_idx < 64 && (val >> bit_idx) & 1 == 1
            {
                names.push((bit_idx, name.as_str()));
            }
        }
        if !names.is_empty() {
            names.sort_by_key(|(idx, _)| *idx);
            let flag_list: Vec<&str> = names.iter().map(|(_, name)| *name).collect();
            return format!("{base} [{}]", flag_list.join(", "));
        }
    }
    base.to_string()
}

const REPEAT_SAFETY_CAP: usize = 10_000;

/// Resolve a parsed template against file bytes, producing concrete offsets and display values.
pub fn resolve(template: &Template, file_bytes: &[u8]) -> ResolveResult {
    let file_len = file_bytes.len() as u64;
    let mut warnings = Vec::new();
    let mut template_links = Vec::new();
    let mut regions = Vec::new();
    let mut field_map: HashMap<String, ResolvedFieldInfo> = HashMap::new();

    for region in &template.regions {
        // Determine iteration count based on repeat mode
        let repeat_mode = &region.repeat;

        // We need to resolve the region offset before we can determine repeat behavior,
        // but for repeating regions the offset is resolved per-iteration.
        // For the first iteration, resolve the base offset.
        let base_offset = match &region.offset {
            OffsetExpr::Absolute(off) => Some(*off),
            OffsetExpr::AfterField(id) => {
                if let Some(info) = field_map.get(id.as_str()) {
                    Some(info.offset + info.length)
                } else {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "region '{}': 'after:{}' references unknown field, skipping",
                            region.id, id
                        ),
                    });
                    None
                }
            }
            OffsetExpr::FromField(id) => {
                if let Some(info) = field_map.get(id.as_str()) {
                    if let Some(val) = info.numeric_value {
                        Some(val)
                    } else {
                        warnings.push(ResolveWarning {
                            message: format!(
                                "region '{}': 'from:{}' field has no numeric value, skipping",
                                region.id, id
                            ),
                        });
                        None
                    }
                } else {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "region '{}': 'from:{}' references unknown field, skipping",
                            region.id, id
                        ),
                    });
                    None
                }
            }
            OffsetExpr::Expr(expr) => {
                match eval_arith_expr(expr, &field_map) {
                    Some(val) => Some(val),
                    None => {
                        warnings.push(ResolveWarning {
                            message: format!(
                                "region '{}': arithmetic expression could not be evaluated, skipping",
                                region.id
                            ),
                        });
                        None
                    }
                }
            }
        };

        let Some(mut iter_offset) = base_offset else {
            continue;
        };

        // Check region-level condition
        if let Some(cond) = &region.condition {
            match eval_condition(cond, &field_map) {
                Some(true) => {} // condition met, proceed
                Some(false) => continue, // condition not met, skip region
                None => {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "region '{}': condition field '{}' not found or has no numeric value, skipping",
                            region.id, cond.field_id
                        ),
                    });
                    continue;
                }
            }
        }

        // Determine max iterations
        let max_iterations = match repeat_mode {
            None => 1,
            Some(RepeatMode::Count) => {
                if let Some(count_field_id) = &region.repeat_count {
                    if let Some(info) = field_map.get(count_field_id.as_str()) {
                        if let Some(val) = info.numeric_value {
                            (val as usize).min(REPEAT_SAFETY_CAP)
                        } else {
                            warnings.push(ResolveWarning {
                                message: format!(
                                    "region '{}': repeat_count field '{}' has no numeric value, skipping",
                                    region.id, count_field_id
                                ),
                            });
                            continue;
                        }
                    } else {
                        warnings.push(ResolveWarning {
                            message: format!(
                                "region '{}': repeat_count field '{}' not found, skipping",
                                region.id, count_field_id
                            ),
                        });
                        continue;
                    }
                } else {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "region '{}': repeat mode 'count' but no repeat_count specified, skipping",
                            region.id
                        ),
                    });
                    continue;
                }
            }
            Some(RepeatMode::UntilEof) => REPEAT_SAFETY_CAP,
            Some(RepeatMode::UntilMagic) => REPEAT_SAFETY_CAP,
        };

        // Parse sentinel bytes for UntilMagic
        let sentinel_bytes = if matches!(repeat_mode, Some(RepeatMode::UntilMagic)) {
            if let Some(sentinel_hex) = &region.repeat_until {
                match parse_hex_bytes(sentinel_hex) {
                    Some(bytes) => Some(bytes),
                    None => {
                        warnings.push(ResolveWarning {
                            message: format!(
                                "region '{}': invalid repeat_until hex '{}', skipping",
                                region.id, sentinel_hex
                            ),
                        });
                        continue;
                    }
                }
            } else {
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': repeat mode 'until_magic' but no repeat_until specified, skipping",
                        region.id
                    ),
                });
                continue;
            }
        } else {
            None
        };

        for iteration in 0..max_iterations {
            // Check termination conditions for repeating regions
            if iter_offset >= file_len {
                if matches!(repeat_mode, Some(RepeatMode::UntilEof)) {
                    break; // Normal termination
                }
                if repeat_mode.is_some() {
                    break; // Out of bounds for any repeat mode
                }
                // Non-repeating: warn and skip
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': offset {} beyond file length {}",
                        region.id, iter_offset, file_len
                    ),
                });
                break;
            }

            // Check UntilMagic sentinel
            if let Some(sentinel) = &sentinel_bytes {
                let end = (iter_offset as usize + sentinel.len()).min(file_bytes.len());
                if iter_offset as usize + sentinel.len() <= file_bytes.len()
                    && &file_bytes[iter_offset as usize..end] == sentinel.as_slice()
                {
                    break;
                }
            }

            // Generate IDs for this iteration
            let (region_iter_id, field_id_prefix) = if repeat_mode.is_some() {
                (format!("{}.{}", region.id, iteration), Some(iteration))
            } else {
                (region.id.clone(), None)
            };

            // Resolve fields sequentially
            let mut resolved_fields = Vec::new();
            let mut field_cursor = iter_offset;

            for field in &region.fields {
                // Check field-level condition
                if let Some(cond) = &field.condition {
                    match eval_condition(cond, &field_map) {
                        Some(true) => {} // condition met, proceed
                        Some(false) => continue, // skip field, cursor doesn't advance
                        None => {
                            warnings.push(ResolveWarning {
                                message: format!(
                                    "field '{}': condition field '{}' not found or has no numeric value, skipping",
                                    field.id, cond.field_id
                                ),
                            });
                            continue;
                        }
                    }
                }

                // Apply explicit relative offset if present
                let field_offset = if let Some(rel) = field.offset {
                    iter_offset + rel
                } else {
                    field_cursor
                };

                // Resolve field length
                let field_length = match &field.length {
                    LengthExpr::Fixed(n) => *n,
                    LengthExpr::ToEnd => file_len.saturating_sub(field_offset),
                    LengthExpr::FromField(id) => {
                        if let Some(info) = field_map.get(id.as_str()) {
                            if let Some(val) = info.numeric_value {
                                val
                            } else {
                                warnings.push(ResolveWarning {
                                    message: format!(
                                        "field '{}': 'from:{}' field has no numeric value, skipping",
                                        field.id, id
                                    ),
                                });
                                continue;
                            }
                        } else {
                            warnings.push(ResolveWarning {
                                message: format!(
                                    "field '{}': 'from:{}' references unknown field, skipping",
                                    field.id, id
                                ),
                            });
                            continue;
                        }
                    }
                    LengthExpr::Expr(expr) => {
                        match eval_arith_expr(expr, &field_map) {
                            Some(val) => val,
                            None => {
                                warnings.push(ResolveWarning {
                                    message: format!(
                                        "field '{}': arithmetic length expression could not be evaluated, skipping",
                                        field.id
                                    ),
                                });
                                continue;
                            }
                        }
                    }
                };

                // Bounds check for field
                if field_offset + field_length > file_len {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "field '{}': extends beyond file (offset {} + length {} > {})",
                            field.id, field_offset, field_length, file_len
                        ),
                    });
                    continue;
                }

                let raw_bytes =
                    file_bytes[field_offset as usize..(field_offset + field_length) as usize]
                        .to_vec();
                let mut display_value = format_field_value(&field.field_type, &raw_bytes);
                let numeric_value = extract_numeric_value(&field.field_type, &raw_bytes);

                // Enrich display with enum or bitflag labels
                if let Some(enum_values) = &field.enum_values {
                    display_value = format_enum_display(&display_value, numeric_value, enum_values);
                } else if let Some(bit_flags) = &field.bit_flags {
                    display_value =
                        format_bitflags_display(&display_value, numeric_value, bit_flags);
                }

                let field_iter_id = if let Some(n) = field_id_prefix {
                    format!("{}.{}", field.id, n)
                } else {
                    field.id.clone()
                };

                resolved_fields.push(ResolvedField {
                    id: field_iter_id.clone(),
                    label: field.label.clone(),
                    field_type: field.field_type.clone(),
                    offset: field_offset,
                    length: field_length,
                    role: field.role.clone(),
                    description: field.description.clone(),
                    raw_bytes,
                    display_value,
                    color: field.color.as_deref().and_then(TemplateColor::from_hex),
                    computed_value: None,
                });

                // Register in field_map — suffixed ID and base ID (base always points to latest)
                let info = ResolvedFieldInfo {
                    offset: field_offset,
                    length: field_length,
                    numeric_value,
                };
                field_map.insert(field_iter_id, info.clone());
                field_map.insert(field.id.clone(), info);

                field_cursor = field_offset + field_length;
            }

            // Resolve region length
            let region_length = if let Some(len_expr) = &region.length {
                match len_expr {
                    LengthExpr::Fixed(n) => *n,
                    LengthExpr::ToEnd => file_len.saturating_sub(iter_offset),
                    LengthExpr::FromField(id) => {
                        if let Some(info) = field_map.get(id.as_str()) {
                            if let Some(val) = info.numeric_value {
                                val
                            } else {
                                // Fall back to computing from fields
                                warnings.push(ResolveWarning {
                                    message: format!(
                                        "region '{}': 'from:{}' field has no numeric value, computing from fields",
                                        region.id, id
                                    ),
                                });
                                if let Some(last) = resolved_fields.last() {
                                    (last.offset + last.length).saturating_sub(iter_offset)
                                } else {
                                    0
                                }
                            }
                        } else {
                            warnings.push(ResolveWarning {
                                message: format!(
                                    "region '{}': 'from:{}' references unknown field, computing from fields",
                                    region.id, id
                                ),
                            });
                            if let Some(last) = resolved_fields.last() {
                                (last.offset + last.length).saturating_sub(iter_offset)
                            } else {
                                0
                            }
                        }
                    }
                    LengthExpr::Expr(expr) => {
                        match eval_arith_expr(expr, &field_map) {
                            Some(val) => val,
                            None => {
                                warnings.push(ResolveWarning {
                                    message: format!(
                                        "region '{}': arithmetic length expression could not be evaluated, computing from fields",
                                        region.id
                                    ),
                                });
                                if let Some(last) = resolved_fields.last() {
                                    (last.offset + last.length).saturating_sub(iter_offset)
                                } else {
                                    0
                                }
                            }
                        }
                    }
                }
            } else {
                // Compute from fields
                if let Some(last) = resolved_fields.last() {
                    (last.offset + last.length).saturating_sub(iter_offset)
                } else {
                    0
                }
            };

            // Bounds check for region extent
            if iter_offset + region_length > file_len {
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': extends beyond file (offset {} + length {} > {})",
                        region.id, iter_offset, region_length, file_len
                    ),
                });
                // Still add but note the warning
            }

            let color = region
                .color
                .as_deref()
                .and_then(TemplateColor::from_hex)
                .unwrap_or_default();

            let label = if repeat_mode.is_some() {
                format!("{} [{}]", region.label, iteration)
            } else {
                region.label.clone()
            };

            // Register region in field_map — suffixed and base ID
            let region_info = ResolvedFieldInfo {
                offset: iter_offset,
                length: region_length,
                numeric_value: None,
            };
            field_map.insert(region_iter_id.clone(), region_info.clone());
            field_map.insert(region.id.clone(), region_info);

            regions.push(ResolvedRegion {
                id: region_iter_id,
                label,
                color,
                offset: iter_offset,
                length: region_length,
                group: region.group.clone(),
                description: region.description.clone(),
                fields: resolved_fields,
            });

            // Advance offset for next iteration
            iter_offset += region_length;

            // Safety: if region_length is 0, stop to avoid infinite loop
            if region_length == 0 && repeat_mode.is_some() {
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': zero-length region in repeat mode, stopping after {} iterations",
                        region.id, iteration + 1
                    ),
                });
                break;
            }
        }
    }

    ResolveResult {
        template: ResolvedTemplate {
            name: template.name.clone(),
            description: template.description.clone(),
            regions,
        },
        warnings,
        template_links,
    }
}

/// Format raw bytes into a human-readable display string based on field type.
fn format_field_value(field_type: &FieldType, raw: &[u8]) -> String {
    let result: Option<String> = match field_type {
        FieldType::U8 => raw.first().map(|b| b.to_string()),
        FieldType::I8 => raw.first().map(|b| (*b as i8).to_string()),

        FieldType::U16Le => {
            try_read(raw, 2).map(|b| u16::from_le_bytes([b[0], b[1]]).to_string())
        }
        FieldType::U16Be => {
            try_read(raw, 2).map(|b| u16::from_be_bytes([b[0], b[1]]).to_string())
        }
        FieldType::I16Le => {
            try_read(raw, 2).map(|b| i16::from_le_bytes([b[0], b[1]]).to_string())
        }
        FieldType::I16Be => {
            try_read(raw, 2).map(|b| i16::from_be_bytes([b[0], b[1]]).to_string())
        }

        FieldType::U32Le => {
            try_read(raw, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string())
        }
        FieldType::U32Be => {
            try_read(raw, 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]).to_string())
        }
        FieldType::I32Le => {
            try_read(raw, 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string())
        }
        FieldType::I32Be => {
            try_read(raw, 4).map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]).to_string())
        }
        FieldType::F32Le => try_read(raw, 4)
            .map(|b| format!("{:.6}", f32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
        FieldType::F32Be => try_read(raw, 4)
            .map(|b| format!("{:.6}", f32::from_be_bytes([b[0], b[1], b[2], b[3]]))),

        FieldType::U64Le => {
            try_read(raw, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()).to_string())
        }
        FieldType::U64Be => {
            try_read(raw, 8).map(|b| u64::from_be_bytes(b.try_into().unwrap()).to_string())
        }
        FieldType::I64Le => {
            try_read(raw, 8).map(|b| i64::from_le_bytes(b.try_into().unwrap()).to_string())
        }
        FieldType::I64Be => {
            try_read(raw, 8).map(|b| i64::from_be_bytes(b.try_into().unwrap()).to_string())
        }
        FieldType::F64Le => {
            try_read(raw, 8).map(|b| format!("{:.6}", f64::from_le_bytes(b.try_into().unwrap())))
        }
        FieldType::F64Be => {
            try_read(raw, 8).map(|b| format!("{:.6}", f64::from_be_bytes(b.try_into().unwrap())))
        }

        FieldType::Bytes => {
            let display: Vec<String> = raw.iter().take(16).map(|b| format!("{:02X}", b)).collect();
            let mut s = display.join(" ");
            if raw.len() > 16 {
                s.push_str("...");
            }
            Some(s)
        }
        FieldType::Utf8 | FieldType::Ascii => Some(String::from_utf8_lossy(raw).to_string()),
        FieldType::Computed => Some("(computed)".to_string()),
    };
    result.unwrap_or_else(|| format!("{} bytes", raw.len()))
}

fn try_read(raw: &[u8], n: usize) -> Option<&[u8]> {
    if raw.len() >= n {
        Some(&raw[..n])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn make_template(regions: Vec<Region>) -> Template {
        Template {
            name: "Test".into(),
            description: "Test template".into(),
            extensions: vec![],
            magic: None,
            magic_offset: 0,
            endian: Endianness::Little,
            regions,
        }
    }

    fn make_field(id: &str, field_type: FieldType, length: LengthExpr) -> Field {
        Field {
            id: id.into(),
            label: id.into(),
            field_type,
            length,
            offset: None,
            role: None,
            description: None,

            condition: None,
            enum_values: None,
            bit_flags: None,
            color: None,
            expression: None,
            apply_template: None,
        }
    }

    #[test]
    fn test_from_field_length() {
        // Region A has a u16le "size" field with value 5.
        // Region B has a bytes field whose length = "from:size".
        let template = make_template(vec![
            Region {
                id: "header".into(),
                label: "Header".into(),
                color: None,
                offset: OffsetExpr::Absolute(0),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("size", FieldType::U16Le, LengthExpr::Fixed(2))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
            Region {
                id: "data".into(),
                label: "Data".into(),
                color: None,
                offset: OffsetExpr::Absolute(2),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field(
                    "payload",
                    FieldType::Bytes,
                    LengthExpr::FromField("size".into()),
                )],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
        ]);

        // size = 5 as u16le = [0x05, 0x00], followed by 5 bytes of data
        let file_bytes = [0x05, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
        let result = resolve(&template, &file_bytes);

        assert_eq!(result.template.regions.len(), 2);
        let data_region = &result.template.regions[1];
        assert_eq!(data_region.fields.len(), 1);
        assert_eq!(data_region.fields[0].length, 5);
        assert_eq!(data_region.fields[0].raw_bytes, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    }

    #[test]
    fn test_after_field_offset() {
        // Region B starts "after:header"
        let template = make_template(vec![
            Region {
                id: "header".into(),
                label: "Header".into(),
                color: None,
                offset: OffsetExpr::Absolute(0),
                length: Some(LengthExpr::Fixed(4)),
                group: None,
                description: None,
                fields: vec![make_field("magic", FieldType::Bytes, LengthExpr::Fixed(4))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
            Region {
                id: "body".into(),
                label: "Body".into(),
                color: None,
                offset: OffsetExpr::AfterField("header".into()),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("value", FieldType::U8, LengthExpr::Fixed(1))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
        ]);

        let file_bytes = [0x89, 0x50, 0x4E, 0x47, 0xFF];
        let result = resolve(&template, &file_bytes);

        assert_eq!(result.template.regions.len(), 2);
        let body = &result.template.regions[1];
        assert_eq!(body.offset, 4);
        assert_eq!(body.fields[0].display_value, "255");
    }

    #[test]
    fn test_from_field_offset() {
        // A pointer field at offset 0 contains value 6 (u16le).
        // Region "target" uses offset = "from:pointer", so it starts at byte 6.
        let template = make_template(vec![
            Region {
                id: "header".into(),
                label: "Header".into(),
                color: None,
                offset: OffsetExpr::Absolute(0),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("pointer", FieldType::U16Le, LengthExpr::Fixed(2))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
            Region {
                id: "target".into(),
                label: "Target".into(),
                color: None,
                offset: OffsetExpr::FromField("pointer".into()),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("byte", FieldType::U8, LengthExpr::Fixed(1))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
        ]);

        // pointer = 6, then padding, then target at offset 6
        let file_bytes = [0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42];
        let result = resolve(&template, &file_bytes);

        assert_eq!(result.template.regions.len(), 2);
        let target = &result.template.regions[1];
        assert_eq!(target.offset, 6);
        assert_eq!(target.fields[0].raw_bytes, &[0x42]);
    }

    #[test]
    fn test_repeat_count() {
        // A count field = 3, then a repeating region with 1-byte records
        let template = make_template(vec![
            Region {
                id: "header".into(),
                label: "Header".into(),
                color: None,
                offset: OffsetExpr::Absolute(0),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("count", FieldType::U8, LengthExpr::Fixed(1))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
            Region {
                id: "record".into(),
                label: "Record".into(),
                color: None,
                offset: OffsetExpr::AfterField("header".into()),
                length: Some(LengthExpr::Fixed(2)),
                group: None,
                description: None,
                fields: vec![
                    make_field("a", FieldType::U8, LengthExpr::Fixed(1)),
                    make_field("b", FieldType::U8, LengthExpr::Fixed(1)),
                ],
                repeat: Some(RepeatMode::Count),
                repeat_count: Some("count".into()),
                repeat_until: None,
                condition: None,
            },
        ]);

        let file_bytes = [0x03, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
        let result = resolve(&template, &file_bytes);

        // 1 header + 3 repeated records
        assert_eq!(result.template.regions.len(), 4);
        assert_eq!(result.template.regions[1].id, "record.0");
        assert_eq!(result.template.regions[1].offset, 1);
        assert_eq!(result.template.regions[2].id, "record.1");
        assert_eq!(result.template.regions[2].offset, 3);
        assert_eq!(result.template.regions[3].id, "record.2");
        assert_eq!(result.template.regions[3].offset, 5);
    }

    #[test]
    fn test_repeat_until_eof() {
        let template = make_template(vec![Region {
            id: "chunk".into(),
            label: "Chunk".into(),
            color: None,
            offset: OffsetExpr::Absolute(0),
            length: Some(LengthExpr::Fixed(2)),
            group: None,
            description: None,
            fields: vec![
                make_field("lo", FieldType::U8, LengthExpr::Fixed(1)),
                make_field("hi", FieldType::U8, LengthExpr::Fixed(1)),
            ],
            repeat: Some(RepeatMode::UntilEof),
            repeat_count: None,
            repeat_until: None,
            condition: None,
        }]);

        let file_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let result = resolve(&template, &file_bytes);

        assert_eq!(result.template.regions.len(), 3);
        assert_eq!(result.template.regions[0].offset, 0);
        assert_eq!(result.template.regions[1].offset, 2);
        assert_eq!(result.template.regions[2].offset, 4);
    }

    #[test]
    fn test_forward_reference_skipped() {
        // Region A references "future_field" which hasn't been resolved yet
        let template = make_template(vec![
            Region {
                id: "first".into(),
                label: "First".into(),
                color: None,
                offset: OffsetExpr::AfterField("future_field".into()),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
            Region {
                id: "second".into(),
                label: "Second".into(),
                color: None,
                offset: OffsetExpr::Absolute(0),
                length: None,
                group: None,
                description: None,
                fields: vec![make_field("future_field", FieldType::U8, LengthExpr::Fixed(1))],
                repeat: None,
                repeat_count: None,
                repeat_until: None,
                condition: None,
            },
        ]);

        let file_bytes = [0x01, 0x02];
        let result = resolve(&template, &file_bytes);

        // "first" should be skipped, only "second" should resolve
        assert_eq!(result.template.regions.len(), 1);
        assert_eq!(result.template.regions[0].id, "second");
        assert!(result.warnings.iter().any(|w| w.message.contains("unknown field")));
    }

    fn make_region(id: &str, offset: OffsetExpr, fields: Vec<Field>) -> Region {
        Region {
            id: id.into(),
            label: id.into(),
            color: None,
            offset,
            length: None,
            group: None,
            description: None,
            fields,
            repeat: None,
            repeat_count: None,
            repeat_until: None,
            condition: None,
        }
    }

    #[test]
    fn test_arith_expr_multiply() {
        // block_num field = 16 (u16le), block_size field = 2048 (u16le)
        // Region "data" uses offset = expr:block_num * block_size => 16 * 2048 = 32768
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![
                    make_field("block_num", FieldType::U16Le, LengthExpr::Fixed(2)),
                    make_field("block_size", FieldType::U16Le, LengthExpr::Fixed(2)),
                ],
            ),
            Region {
                offset: OffsetExpr::Expr(ArithExpr {
                    left: Operand::FieldRef("block_num".into()),
                    op: ArithOp::Mul,
                    right: Operand::FieldRef("block_size".into()),
                }),
                ..make_region(
                    "data",
                    OffsetExpr::Absolute(0), // placeholder, overridden
                    vec![make_field("byte", FieldType::U8, LengthExpr::Fixed(1))],
                )
            },
        ]);

        // block_num=16 (0x10,0x00), block_size=2048 (0x00,0x08)
        // We need file_bytes to be at least 32769 bytes long. Use a small file for the test.
        // Actually let's use small values: block_num=4, block_size=3 => offset=12
        let mut file_bytes = vec![0x04, 0x00, 0x03, 0x00]; // block_num=4, block_size=3
        file_bytes.resize(13, 0x00);
        file_bytes[12] = 0x42;

        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions.len(), 2);
        assert_eq!(result.template.regions[1].offset, 12);
        assert_eq!(result.template.regions[1].fields[0].raw_bytes, &[0x42]);
    }

    #[test]
    fn test_arith_expr_add() {
        // field_a=3, field_b=5 => length = 3 + 5 = 8
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![
                    make_field("field_a", FieldType::U8, LengthExpr::Fixed(1)),
                    make_field("field_b", FieldType::U8, LengthExpr::Fixed(1)),
                ],
            ),
            make_region(
                "data",
                OffsetExpr::Absolute(2),
                vec![Field {
                    length: LengthExpr::Expr(ArithExpr {
                        left: Operand::FieldRef("field_a".into()),
                        op: ArithOp::Add,
                        right: Operand::FieldRef("field_b".into()),
                    }),
                    ..make_field("payload", FieldType::Bytes, LengthExpr::Fixed(0))
                }],
            ),
        ]);

        let mut file_bytes = vec![3, 5]; // field_a=3, field_b=5
        file_bytes.extend(vec![0xAA; 8]); // 8 bytes of payload

        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions[1].fields[0].length, 8);
    }

    #[test]
    fn test_arith_expr_div_by_zero() {
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![make_field("zero", FieldType::U8, LengthExpr::Fixed(1))],
            ),
            Region {
                offset: OffsetExpr::Expr(ArithExpr {
                    left: Operand::Literal(100),
                    op: ArithOp::Div,
                    right: Operand::FieldRef("zero".into()),
                }),
                ..make_region(
                    "data",
                    OffsetExpr::Absolute(0),
                    vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
                )
            },
        ]);

        let file_bytes = [0x00, 0xFF]; // zero=0
        let result = resolve(&template, &file_bytes);

        // data region should be skipped due to div-by-zero
        assert_eq!(result.template.regions.len(), 1);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("could not be evaluated")));
    }

    #[test]
    fn test_condition_true() {
        // version=2, region with condition "version == 2" should be included
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![make_field("version", FieldType::U8, LengthExpr::Fixed(1))],
            ),
            Region {
                condition: Some(ConditionExpr {
                    field_id: "version".into(),
                    op: CompareOp::Eq,
                    value: 2,
                }),
                ..make_region(
                    "v2_data",
                    OffsetExpr::Absolute(1),
                    vec![make_field("val", FieldType::U8, LengthExpr::Fixed(1))],
                )
            },
        ]);

        let file_bytes = [0x02, 0x42];
        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions.len(), 2);
        assert_eq!(result.template.regions[1].id, "v2_data");
    }

    #[test]
    fn test_condition_false() {
        // version=1, region with condition "version == 2" should be skipped
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![make_field("version", FieldType::U8, LengthExpr::Fixed(1))],
            ),
            Region {
                condition: Some(ConditionExpr {
                    field_id: "version".into(),
                    op: CompareOp::Eq,
                    value: 2,
                }),
                ..make_region(
                    "v2_data",
                    OffsetExpr::Absolute(1),
                    vec![make_field("val", FieldType::U8, LengthExpr::Fixed(1))],
                )
            },
        ]);

        let file_bytes = [0x01, 0x42];
        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions.len(), 1);
        assert_eq!(result.template.regions[0].id, "header");
    }

    #[test]
    fn test_condition_field_skip() {
        // Field with condition false should be skipped and cursor shouldn't advance
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![make_field("flag", FieldType::U8, LengthExpr::Fixed(1))],
            ),
            make_region(
                "data",
                OffsetExpr::Absolute(1),
                vec![
                    Field {
                        condition: Some(ConditionExpr {
                            field_id: "flag".into(),
                            op: CompareOp::Eq,
                            value: 1,
                        }),
                        ..make_field("optional_field", FieldType::U16Le, LengthExpr::Fixed(2))
                    },
                    make_field("next_field", FieldType::U8, LengthExpr::Fixed(1)),
                ],
            ),
        ]);

        // flag=0 so optional_field is skipped; next_field should start at offset 1 (not 3)
        let file_bytes = [0x00, 0xAA, 0xBB];
        let result = resolve(&template, &file_bytes);

        let data_region = &result.template.regions[1];
        // Only next_field should be resolved (optional_field skipped)
        assert_eq!(data_region.fields.len(), 1);
        assert_eq!(data_region.fields[0].id, "next_field");
        assert_eq!(data_region.fields[0].offset, 1); // cursor didn't advance past skipped field
    }

    #[test]
    fn test_enum_display() {
        use std::collections::HashMap;
        let mut enum_values = HashMap::new();
        enum_values.insert("8".to_string(), "Deflated".to_string());
        enum_values.insert("0".to_string(), "Stored".to_string());

        let template = make_template(vec![make_region(
            "header",
            OffsetExpr::Absolute(0),
            vec![Field {
                enum_values: Some(enum_values),
                ..make_field("compression", FieldType::U8, LengthExpr::Fixed(1))
            }],
        )]);

        let file_bytes = [8];
        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions[0].fields[0].display_value, "8 (Deflated)");
    }

    #[test]
    fn test_enum_display_unknown() {
        use std::collections::HashMap;
        let mut enum_values = HashMap::new();
        enum_values.insert("8".to_string(), "Deflated".to_string());

        let template = make_template(vec![make_region(
            "header",
            OffsetExpr::Absolute(0),
            vec![Field {
                enum_values: Some(enum_values),
                ..make_field("compression", FieldType::U8, LengthExpr::Fixed(1))
            }],
        )]);

        let file_bytes = [99]; // Not in enum_values
        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions[0].fields[0].display_value, "99");
    }

    #[test]
    fn test_bitflags_display() {
        use std::collections::HashMap;
        let mut bit_flags = HashMap::new();
        bit_flags.insert("0".to_string(), "Encrypted".to_string());
        bit_flags.insert("3".to_string(), "Data Descriptor".to_string());
        bit_flags.insert("11".to_string(), "UTF-8".to_string());

        let template = make_template(vec![make_region(
            "header",
            OffsetExpr::Absolute(0),
            vec![Field {
                bit_flags: Some(bit_flags),
                ..make_field("flags", FieldType::U16Le, LengthExpr::Fixed(2))
            }],
        )]);

        // value = 0x0809 = 2057 = bits 0, 3, 11 set
        let file_bytes = [0x09, 0x08]; // little-endian 0x0809
        let result = resolve(&template, &file_bytes);
        assert_eq!(
            result.template.regions[0].fields[0].display_value,
            "2057 [Encrypted, Data Descriptor, UTF-8]"
        );
    }

    #[test]
    fn test_cybiko_cfs_template() {
        // Parse the actual CFS template from TOML
        let toml_str = include_str!("../../../templates/filesystems/cybiko-cfs.toml");
        let template = crate::parser::parse_template_str(toml_str).expect("CFS template should parse");

        assert_eq!(template.name, "Cybiko CFS (Xtreme)");

        // Build a synthetic CFS image: 5 boot pages + 3 file pages
        const PAGE_SIZE: usize = 258;
        let mut image = vec![0xFFu8; 5 * PAGE_SIZE + 3 * PAGE_SIZE];

        // Page 5 (offset 1290): first block of a file (part_id = 0)
        let page5 = 5 * PAGE_SIZE;
        image[page5] = 0x00; // CRC16 high
        image[page5 + 1] = 0x00; // CRC16 low
        image[page5 + 2] = 0x80; // flags: BLOCK_USED
        image[page5 + 3] = 5; // data_size: 5 bytes
        image[page5 + 4] = 0x00; // file_id high
        image[page5 + 5] = 0x00; // file_id low = 0
        image[page5 + 6] = 0x00; // part_id high
        image[page5 + 7] = 0x00; // part_id low = 0
        image[page5 + 8] = 0x20; // type_marker: first block
        // filename at page5+9..page5+76 (67 bytes)
        let name = b"test.app\0";
        image[page5 + 9..page5 + 9 + name.len()].copy_from_slice(name);
        // timestamp at page5+76..page5+80
        image[page5 + 76] = 0x5A;
        image[page5 + 77] = 0x34;
        image[page5 + 78] = 0x5A;
        image[page5 + 79] = 0xBC;
        // file data at page5+80..page5+258 (178 bytes) - leave as 0xFF

        // Page 6 (offset 1548): continuation block (part_id = 1)
        let page6 = 6 * PAGE_SIZE;
        image[page6] = 0x00;
        image[page6 + 1] = 0x00;
        image[page6 + 2] = 0x80; // BLOCK_USED
        image[page6 + 3] = 3; // data_size
        image[page6 + 4] = 0x00;
        image[page6 + 5] = 0x00; // file_id = 0
        image[page6 + 6] = 0x00;
        image[page6 + 7] = 0x01; // part_id = 1
        // data at page6+8..page6+258 (250 bytes) - leave as 0xFF

        // Page 7 (offset 1806): unused block
        let page7 = 7 * PAGE_SIZE;
        // All 0xFF from init, but clear bit 7 of flags
        image[page7 + 2] = 0x7F; // flags: unused

        let result = resolve(&template, &image);

        // Should have: boot region + 3 file page iterations
        assert_eq!(result.template.regions.len(), 4);

        // Boot region
        assert_eq!(result.template.regions[0].id, "boot");
        assert_eq!(result.template.regions[0].offset, 0);
        assert_eq!(result.template.regions[0].length, 1290);

        // First file page (part 0): should have all header fields
        let page5_region = &result.template.regions[1];
        assert_eq!(page5_region.offset, 1290);
        // Fields: crc, flags, data_size, file_id, part_id, type_marker, filename, timestamp, first_block_data
        // (cont_block_data skipped because part_id == 0)
        assert_eq!(page5_region.fields.len(), 9);
        // Repeating regions suffix field IDs with .N (iteration index)
        assert!(page5_region.fields[0].id.starts_with("page_crc"));
        assert!(page5_region.fields[4].id.starts_with("part_id"));
        assert_eq!(page5_region.fields[4].display_value, "0");
        assert!(page5_region.fields[5].id.starts_with("type_marker"));
        assert_eq!(page5_region.fields[5].display_value, "32 (File Entry)");
        assert!(page5_region.fields[6].id.starts_with("filename"));
        assert!(page5_region.fields[8].id.starts_with("first_block_data"));

        // Continuation page (part 1): should skip first-block fields, show cont data
        let page6_region = &result.template.regions[2];
        assert_eq!(page6_region.offset, 1548);
        // Fields: crc, flags, data_size, file_id, part_id, cont_block_data
        // (type_marker, filename, timestamp, first_block_data skipped because part_id != 0)
        assert_eq!(page6_region.fields.len(), 6);
        assert!(page6_region.fields[4].id.starts_with("part_id"));
        assert_eq!(page6_region.fields[4].display_value, "1");
        assert!(page6_region.fields[5].id.starts_with("cont_block_data"));

        // Unused page (part_id = 0xFFFF): should show cont data (part_id != 0)
        let page7_region = &result.template.regions[3];
        assert_eq!(page7_region.offset, 1806);
        assert_eq!(page7_region.fields.len(), 6);
        assert!(page7_region.fields[5].id.starts_with("cont_block_data"));
    }

    #[test]
    fn test_repeat_until_magic() {
        // Region repeats until sentinel bytes "DEAD" are found
        let template = make_template(vec![Region {
            repeat: Some(RepeatMode::UntilMagic),
            repeat_until: Some("DEAD".into()),
            ..make_region(
                "entry",
                OffsetExpr::Absolute(0),
                vec![make_field("val", FieldType::U16Le, LengthExpr::Fixed(2))],
            )
        }]);

        // Three u16le values followed by sentinel 0xDE 0xAD
        let file_bytes = [0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0xDE, 0xAD];
        let result = resolve(&template, &file_bytes);

        // Should produce 3 iterations, stopping before the sentinel
        assert_eq!(result.template.regions.len(), 3);
        assert_eq!(result.template.regions[0].offset, 0);
        assert_eq!(result.template.regions[1].offset, 2);
        assert_eq!(result.template.regions[2].offset, 4);
        assert_eq!(result.template.regions[0].fields[0].display_value, "1");
        assert_eq!(result.template.regions[1].fields[0].display_value, "2");
        assert_eq!(result.template.regions[2].fields[0].display_value, "3");
    }

    #[test]
    fn test_arith_expr_sub() {
        // total_size=10, header_size=3 => payload length = 10 - 3 = 7
        let template = make_template(vec![
            make_region(
                "header",
                OffsetExpr::Absolute(0),
                vec![
                    make_field("total_size", FieldType::U8, LengthExpr::Fixed(1)),
                    make_field("header_size", FieldType::U8, LengthExpr::Fixed(1)),
                ],
            ),
            make_region(
                "data",
                OffsetExpr::Absolute(2),
                vec![Field {
                    length: LengthExpr::Expr(ArithExpr {
                        left: Operand::FieldRef("total_size".into()),
                        op: ArithOp::Sub,
                        right: Operand::FieldRef("header_size".into()),
                    }),
                    ..make_field("payload", FieldType::Bytes, LengthExpr::Fixed(0))
                }],
            ),
        ]);

        let mut file_bytes = vec![10, 3]; // total_size=10, header_size=3
        file_bytes.extend(vec![0xBB; 7]); // 7 bytes payload

        let result = resolve(&template, &file_bytes);
        assert_eq!(result.template.regions[1].fields[0].length, 7);
        assert_eq!(result.template.regions[1].fields[0].raw_bytes, vec![0xBB; 7]);
    }

    #[test]
    fn test_length_to_end() {
        // Field with ToEnd length should consume all remaining bytes
        let template = make_template(vec![make_region(
            "data",
            OffsetExpr::Absolute(4),
            vec![make_field("rest", FieldType::Bytes, LengthExpr::ToEnd)],
        )]);

        let file_bytes = [0x00, 0x01, 0x02, 0x03, 0xAA, 0xBB, 0xCC];
        let result = resolve(&template, &file_bytes);

        // Offset 4, file length 7 => ToEnd = 3 bytes
        assert_eq!(result.template.regions[0].fields[0].length, 3);
        assert_eq!(
            result.template.regions[0].fields[0].raw_bytes,
            &[0xAA, 0xBB, 0xCC]
        );
    }

    #[test]
    fn test_length_to_end_single_byte() {
        // ToEnd with only 1 byte remaining
        let template = make_template(vec![make_region(
            "data",
            OffsetExpr::Absolute(2),
            vec![make_field("rest", FieldType::Bytes, LengthExpr::ToEnd)],
        )]);

        let file_bytes = [0x00, 0x01, 0xFE];
        let result = resolve(&template, &file_bytes);

        assert_eq!(result.template.regions[0].fields[0].length, 1);
        assert_eq!(result.template.regions[0].fields[0].raw_bytes, &[0xFE]);
    }
}
