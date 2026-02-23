use crate::resolved::{ResolvedField, ResolvedRegion, ResolvedTemplate, TemplateColor};
use crate::schema::{FieldType, LengthExpr, OffsetExpr, Template};

#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub template: ResolvedTemplate,
    pub warnings: Vec<ResolveWarning>,
}

#[derive(Debug, Clone)]
pub struct ResolveWarning {
    pub message: String,
}

impl std::fmt::Display for ResolveWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Resolve a parsed template against file bytes, producing concrete offsets and display values.
pub fn resolve(template: &Template, file_bytes: &[u8]) -> ResolveResult {
    let file_len = file_bytes.len() as u64;
    let mut warnings = Vec::new();
    let mut regions = Vec::new();

    for region in &template.regions {
        // Resolve region offset
        let region_offset = match &region.offset {
            OffsetExpr::Absolute(off) => *off,
            OffsetExpr::AfterField(id) => {
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': 'after:{}' not supported in Phase 2, skipping",
                        region.id, id
                    ),
                });
                continue;
            }
            OffsetExpr::FromField(id) => {
                warnings.push(ResolveWarning {
                    message: format!(
                        "region '{}': 'from:{}' not supported in Phase 2, skipping",
                        region.id, id
                    ),
                });
                continue;
            }
        };

        // Bounds check for region offset
        if region_offset >= file_len {
            warnings.push(ResolveWarning {
                message: format!(
                    "region '{}': offset {} beyond file length {}",
                    region.id, region_offset, file_len
                ),
            });
            continue;
        }

        // Resolve fields sequentially
        let mut resolved_fields = Vec::new();
        let mut field_cursor = region_offset;

        for field in &region.fields {
            // Apply explicit relative offset if present
            let field_offset = if let Some(rel) = field.offset {
                region_offset + rel
            } else {
                field_cursor
            };

            // Resolve field length
            let field_length = match &field.length {
                LengthExpr::Fixed(n) => *n,
                LengthExpr::ToEnd => file_len.saturating_sub(field_offset),
                LengthExpr::FromField(id) => {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "field '{}': 'from:{}' length not supported in Phase 2, skipping",
                            field.id, id
                        ),
                    });
                    continue;
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

            let raw_bytes = file_bytes[field_offset as usize..(field_offset + field_length) as usize].to_vec();
            let display_value = format_field_value(&field.field_type, &raw_bytes);

            resolved_fields.push(ResolvedField {
                id: field.id.clone(),
                label: field.label.clone(),
                field_type: field.field_type.clone(),
                offset: field_offset,
                length: field_length,
                role: field.role.clone(),
                description: field.description.clone(),
                raw_bytes,
                display_value,
            });

            field_cursor = field_offset + field_length;
        }

        // Resolve region length
        let region_length = if let Some(len_expr) = &region.length {
            match len_expr {
                LengthExpr::Fixed(n) => *n,
                LengthExpr::ToEnd => file_len.saturating_sub(region_offset),
                LengthExpr::FromField(id) => {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "region '{}': 'from:{}' length not supported in Phase 2, computing from fields",
                            region.id, id
                        ),
                    });
                    // Fall back to computing from fields
                    if let Some(last) = resolved_fields.last() {
                        (last.offset + last.length).saturating_sub(region_offset)
                    } else {
                        0
                    }
                }
            }
        } else {
            // Compute from fields
            if let Some(last) = resolved_fields.last() {
                (last.offset + last.length).saturating_sub(region_offset)
            } else {
                0
            }
        };

        // Bounds check for region extent
        if region_offset + region_length > file_len {
            warnings.push(ResolveWarning {
                message: format!(
                    "region '{}': extends beyond file (offset {} + length {} > {})",
                    region.id, region_offset, region_length, file_len
                ),
            });
            // Still add the region but clamp
        }

        let color = region
            .color
            .as_deref()
            .and_then(TemplateColor::from_hex)
            .unwrap_or_default();

        regions.push(ResolvedRegion {
            id: region.id.clone(),
            label: region.label.clone(),
            color,
            offset: region_offset,
            length: region_length,
            group: region.group.clone(),
            description: region.description.clone(),
            fields: resolved_fields,
        });
    }

    ResolveResult {
        template: ResolvedTemplate {
            name: template.name.clone(),
            description: template.description.clone(),
            regions,
        },
        warnings,
    }
}

/// Format raw bytes into a human-readable display string based on field type.
fn format_field_value(field_type: &FieldType, raw: &[u8]) -> String {
    let result: Option<String> = match field_type {
        FieldType::U8 => raw.first().map(|b| b.to_string()),
        FieldType::I8 => raw.first().map(|b| (*b as i8).to_string()),

        FieldType::U16Le => try_read(raw, 2).map(|b| u16::from_le_bytes([b[0], b[1]]).to_string()),
        FieldType::U16Be => try_read(raw, 2).map(|b| u16::from_be_bytes([b[0], b[1]]).to_string()),
        FieldType::I16Le => try_read(raw, 2).map(|b| i16::from_le_bytes([b[0], b[1]]).to_string()),
        FieldType::I16Be => try_read(raw, 2).map(|b| i16::from_be_bytes([b[0], b[1]]).to_string()),

        FieldType::U32Le => try_read(raw, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string()),
        FieldType::U32Be => try_read(raw, 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]).to_string()),
        FieldType::I32Le => try_read(raw, 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string()),
        FieldType::I32Be => try_read(raw, 4).map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]).to_string()),
        FieldType::F32Le => try_read(raw, 4).map(|b| format!("{:.6}", f32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
        FieldType::F32Be => try_read(raw, 4).map(|b| format!("{:.6}", f32::from_be_bytes([b[0], b[1], b[2], b[3]]))),

        FieldType::U64Le => try_read(raw, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()).to_string()),
        FieldType::U64Be => try_read(raw, 8).map(|b| u64::from_be_bytes(b.try_into().unwrap()).to_string()),
        FieldType::I64Le => try_read(raw, 8).map(|b| i64::from_le_bytes(b.try_into().unwrap()).to_string()),
        FieldType::I64Be => try_read(raw, 8).map(|b| i64::from_be_bytes(b.try_into().unwrap()).to_string()),
        FieldType::F64Le => try_read(raw, 8).map(|b| format!("{:.6}", f64::from_le_bytes(b.try_into().unwrap()))),
        FieldType::F64Be => try_read(raw, 8).map(|b| format!("{:.6}", f64::from_be_bytes(b.try_into().unwrap()))),

        FieldType::Bytes => {
            let display: Vec<String> = raw.iter().take(16).map(|b| format!("{:02X}", b)).collect();
            let mut s = display.join(" ");
            if raw.len() > 16 {
                s.push_str("...");
            }
            Some(s)
        }
        FieldType::Utf8 | FieldType::Ascii => {
            Some(String::from_utf8_lossy(raw).to_string())
        }
    };
    result.unwrap_or_else(|| format!("{} bytes", raw.len()))
}

fn try_read(raw: &[u8], n: usize) -> Option<&[u8]> {
    if raw.len() >= n { Some(&raw[..n]) } else { None }
}
