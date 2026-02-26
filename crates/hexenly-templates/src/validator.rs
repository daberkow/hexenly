//! Static validation for parsed templates.
//!
//! Checks for duplicate IDs, unknown references, forward references,
//! circular dependencies, and inconsistent repeat/condition configurations.

use std::collections::{HashMap, HashSet};

use crate::resolved::TemplateColor;
use crate::schema::{LengthExpr, OffsetExpr, Operand, RepeatMode, Template};

/// A non-fatal validation issue (the template can still be loaded, but may not resolve correctly).
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub message: String,
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Validate field references within an arithmetic expression operand.
fn validate_operand_ref<'a>(
    operand: &'a Operand,
    context: &str,
    all_ids: &HashSet<&str>,
    seen_ids: &HashSet<&str>,
    warnings: &mut Vec<ValidationWarning>,
    deps: &mut Vec<&'a str>,
) {
    if let Operand::FieldRef(id) = operand {
        if !all_ids.contains(id.as_str()) {
            warnings.push(ValidationWarning {
                message: format!("{context}: expression references unknown ID '{id}'"),
            });
        } else if !seen_ids.contains(id.as_str()) {
            warnings.push(ValidationWarning {
                message: format!(
                    "{context}: expression references '{id}' which is defined later (forward reference)"
                ),
            });
        }
        deps.push(id.as_str());
    }
}

/// Validate a parsed template and return non-fatal warnings.
pub fn validate(template: &Template) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    // Collect all known IDs (regions and fields)
    let mut all_ids = HashSet::new();
    let mut region_ids = HashSet::new();

    for region in &template.regions {
        // Check duplicate region IDs
        if !region_ids.insert(&region.id) {
            warnings.push(ValidationWarning {
                message: format!("duplicate region ID: '{}'", region.id),
            });
        }
        all_ids.insert(region.id.as_str());

        // Check duplicate field IDs within each region
        let mut field_ids = HashSet::new();
        for field in &region.fields {
            if !field_ids.insert(&field.id) {
                warnings.push(ValidationWarning {
                    message: format!(
                        "duplicate field ID '{}' in region '{}'",
                        field.id, region.id
                    ),
                });
            }
            all_ids.insert(field.id.as_str());
        }

        // Validate color format
        if let Some(color) = &region.color
            && TemplateColor::from_hex(color).is_none()
        {
            warnings.push(ValidationWarning {
                message: format!(
                    "invalid color '{}' in region '{}' (expected #RRGGBB)",
                    color, region.id
                ),
            });
        }
    }

    // Reference validation and forward reference detection
    let mut seen_ids: HashSet<&str> = HashSet::new();
    // Adjacency list for cycle detection: id -> list of ids it depends on
    let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();

    for region in &template.regions {
        let mut region_deps = Vec::new();

        // Check region offset references
        match &region.offset {
            OffsetExpr::AfterField(id) | OffsetExpr::FromField(id) => {
                if !all_ids.contains(id.as_str()) {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "region '{}': offset references unknown ID '{}'",
                            region.id, id
                        ),
                    });
                } else if !seen_ids.contains(id.as_str()) {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "region '{}': offset references '{}' which is defined later (forward reference)",
                            region.id, id
                        ),
                    });
                }
                region_deps.push(id.as_str());
            }
            OffsetExpr::Expr(expr) => {
                let ctx = format!("region '{}'", region.id);
                validate_operand_ref(
                    &expr.left,
                    &ctx,
                    &all_ids,
                    &seen_ids,
                    &mut warnings,
                    &mut region_deps,
                );
                validate_operand_ref(
                    &expr.right,
                    &ctx,
                    &all_ids,
                    &seen_ids,
                    &mut warnings,
                    &mut region_deps,
                );
            }
            OffsetExpr::Absolute(_) => {}
        }

        // Check region length references
        if let Some(len_expr) = &region.length {
            match len_expr {
                LengthExpr::FromField(id) => {
                    if !all_ids.contains(id.as_str()) {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "region '{}': length references unknown ID '{}'",
                                region.id, id
                            ),
                        });
                    } else if !seen_ids.contains(id.as_str()) {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "region '{}': length references '{}' which is defined later (forward reference)",
                                region.id, id
                            ),
                        });
                    }
                    region_deps.push(id.as_str());
                }
                LengthExpr::Expr(expr) => {
                    let ctx = format!("region '{}'", region.id);
                    validate_operand_ref(
                        &expr.left,
                        &ctx,
                        &all_ids,
                        &seen_ids,
                        &mut warnings,
                        &mut region_deps,
                    );
                    validate_operand_ref(
                        &expr.right,
                        &ctx,
                        &all_ids,
                        &seen_ids,
                        &mut warnings,
                        &mut region_deps,
                    );
                }
                LengthExpr::Fixed(_) | LengthExpr::ToEnd => {}
            }
        }

        // Check region condition reference
        if let Some(cond) = &region.condition {
            if !all_ids.contains(cond.field_id.as_str()) {
                warnings.push(ValidationWarning {
                    message: format!(
                        "region '{}': condition references unknown field '{}'",
                        region.id, cond.field_id
                    ),
                });
            } else if !seen_ids.contains(cond.field_id.as_str()) {
                warnings.push(ValidationWarning {
                    message: format!(
                        "region '{}': condition references '{}' which is defined later (forward reference)",
                        region.id, cond.field_id
                    ),
                });
            }
            region_deps.push(cond.field_id.as_str());
        }

        // Check field-level references
        for field in &region.fields {
            match &field.length {
                LengthExpr::FromField(id) => {
                    if !all_ids.contains(id.as_str()) {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "field '{}': length references unknown ID '{}'",
                                field.id, id
                            ),
                        });
                    } else if !seen_ids.contains(id.as_str()) {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "field '{}': length references '{}' which is defined later (forward reference)",
                                field.id, id
                            ),
                        });
                    }
                    region_deps.push(id.as_str());
                }
                LengthExpr::Expr(expr) => {
                    let ctx = format!("field '{}'", field.id);
                    validate_operand_ref(
                        &expr.left,
                        &ctx,
                        &all_ids,
                        &seen_ids,
                        &mut warnings,
                        &mut region_deps,
                    );
                    validate_operand_ref(
                        &expr.right,
                        &ctx,
                        &all_ids,
                        &seen_ids,
                        &mut warnings,
                        &mut region_deps,
                    );
                }
                LengthExpr::Fixed(_) | LengthExpr::ToEnd => {}
            }

            // Check field condition reference
            if let Some(cond) = &field.condition {
                if !all_ids.contains(cond.field_id.as_str()) {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "field '{}': condition references unknown field '{}'",
                            field.id, cond.field_id
                        ),
                    });
                } else if !seen_ids.contains(cond.field_id.as_str()) {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "field '{}': condition references '{}' which is defined later (forward reference)",
                            field.id, cond.field_id
                        ),
                    });
                }
                region_deps.push(cond.field_id.as_str());
            }

            // Validate enum_values keys
            if let Some(enum_values) = &field.enum_values {
                for key in enum_values.keys() {
                    if key.parse::<u64>().is_err() {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "field '{}': enum_values key '{}' is not a valid integer",
                                field.id, key
                            ),
                        });
                    }
                }
            }

            // Validate bit_flags keys
            if let Some(bit_flags) = &field.bit_flags {
                for key in bit_flags.keys() {
                    match key.parse::<u8>() {
                        Ok(n) if n < 64 => {}
                        _ => {
                            warnings.push(ValidationWarning {
                                message: format!(
                                    "field '{}': bit_flags key '{}' is not a valid bit index (0-63)",
                                    field.id, key
                                ),
                            });
                        }
                    }
                }
            }

            // Warn if both enum_values and bit_flags specified
            if field.enum_values.is_some() && field.bit_flags.is_some() {
                warnings.push(ValidationWarning {
                    message: format!(
                        "field '{}': both enum_values and bit_flags specified (enum_values takes precedence)",
                        field.id
                    ),
                });
            }

            seen_ids.insert(field.id.as_str());
        }

        // Repeat consistency checks
        match &region.repeat {
            Some(RepeatMode::Count) => {
                if region.repeat_count.is_none() {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "region '{}': repeat mode 'count' requires repeat_count field",
                            region.id
                        ),
                    });
                } else if let Some(count_id) = &region.repeat_count {
                    if !all_ids.contains(count_id.as_str()) {
                        warnings.push(ValidationWarning {
                            message: format!(
                                "region '{}': repeat_count references unknown ID '{}'",
                                region.id, count_id
                            ),
                        });
                    }
                    region_deps.push(count_id.as_str());
                }
            }
            Some(RepeatMode::UntilMagic) => {
                if region.repeat_until.is_none() {
                    warnings.push(ValidationWarning {
                        message: format!(
                            "region '{}': repeat mode 'until_magic' requires repeat_until field",
                            region.id
                        ),
                    });
                }
            }
            Some(RepeatMode::UntilEof) | None => {}
        }

        if !region_deps.is_empty() {
            deps.insert(region.id.as_str(), region_deps);
        }

        seen_ids.insert(region.id.as_str());
    }

    // Circular dependency detection via DFS
    if let Some(cycle) = detect_cycle(&deps) {
        warnings.push(ValidationWarning {
            message: format!("circular dependency detected: {}", cycle.join(" -> ")),
        });
    }

    warnings
}

/// Detect cycles in a dependency graph using iterative DFS.
fn detect_cycle<'a>(deps: &HashMap<&'a str, Vec<&'a str>>) -> Option<Vec<String>> {
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    for &node in deps.keys() {
        if visited.contains(node) {
            continue;
        }

        // Iterative DFS with explicit stack: (node, iterator_index)
        let mut stack: Vec<(&str, usize)> = vec![(node, 0)];
        let mut path: Vec<&str> = vec![node];
        in_stack.insert(node);

        while let Some((current, idx)) = stack.last_mut() {
            let neighbors = deps.get(*current).map(|v| v.as_slice()).unwrap_or(&[]);

            if *idx >= neighbors.len() {
                // Done with this node
                in_stack.remove(*current);
                visited.insert(*current);
                path.pop();
                stack.pop();
                continue;
            }

            let next = neighbors[*idx];
            *idx += 1;

            if in_stack.contains(next) {
                // Found a cycle — build the cycle path
                let mut cycle: Vec<String> = path
                    .iter()
                    .skip_while(|&&n| n != next)
                    .map(|s| s.to_string())
                    .collect();
                cycle.push(next.to_string());
                return Some(cycle);
            }

            if !visited.contains(next) {
                in_stack.insert(next);
                path.push(next);
                stack.push((next, 0));
            }
        }
    }

    None
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
        }
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
    fn test_validator_circular_dep() {
        // Region A offset depends on B, Region B offset depends on A
        let template = make_template(vec![
            make_region(
                "a",
                OffsetExpr::AfterField("b".into()),
                vec![make_field("fa", FieldType::U8, LengthExpr::Fixed(1))],
            ),
            make_region(
                "b",
                OffsetExpr::AfterField("a".into()),
                vec![make_field("fb", FieldType::U8, LengthExpr::Fixed(1))],
            ),
        ]);

        let warnings = validate(&template);
        assert!(
            warnings.iter().any(|w| w.message.contains("circular dependency")),
            "Expected circular dependency warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_repeat_consistency() {
        // repeat = "count" but no repeat_count field
        let template = make_template(vec![Region {
            repeat: Some(RepeatMode::Count),
            repeat_count: None,
            ..make_region(
                "r",
                OffsetExpr::Absolute(0),
                vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
            )
        }]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("requires repeat_count")),
            "Expected repeat_count warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_unknown_reference() {
        let template = make_template(vec![make_region(
            "r",
            OffsetExpr::AfterField("nonexistent".into()),
            vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
        )]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("unknown ID 'nonexistent'")),
            "Expected unknown ID warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_until_magic_no_sentinel() {
        let template = make_template(vec![Region {
            repeat: Some(RepeatMode::UntilMagic),
            repeat_count: None,
            ..make_region(
                "r",
                OffsetExpr::Absolute(0),
                vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
            )
        }]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("requires repeat_until")),
            "Expected repeat_until warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_arith_unknown_ref() {
        let template = make_template(vec![make_region(
            "r",
            OffsetExpr::Expr(ArithExpr {
                left: Operand::FieldRef("nonexistent".into()),
                op: ArithOp::Mul,
                right: Operand::Literal(2048),
            }),
            vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
        )]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("unknown ID 'nonexistent'")),
            "Expected unknown ID warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_condition_unknown_ref() {
        let template = make_template(vec![Region {
            condition: Some(ConditionExpr {
                field_id: "nonexistent".into(),
                op: CompareOp::Eq,
                value: 1,
            }),
            ..make_region(
                "r",
                OffsetExpr::Absolute(0),
                vec![make_field("x", FieldType::U8, LengthExpr::Fixed(1))],
            )
        }]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("condition references unknown field 'nonexistent'")),
            "Expected condition unknown field warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_enum_invalid_key() {
        use std::collections::HashMap;
        let mut enum_values = HashMap::new();
        enum_values.insert("not_a_number".to_string(), "Bad".to_string());

        let template = make_template(vec![make_region(
            "r",
            OffsetExpr::Absolute(0),
            vec![Field {
                enum_values: Some(enum_values),
                ..make_field("x", FieldType::U8, LengthExpr::Fixed(1))
            }],
        )]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("enum_values key 'not_a_number' is not a valid integer")),
            "Expected enum key warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_bitflag_invalid_key() {
        use std::collections::HashMap;
        let mut bit_flags = HashMap::new();
        bit_flags.insert("99".to_string(), "Out of Range".to_string());

        let template = make_template(vec![make_region(
            "r",
            OffsetExpr::Absolute(0),
            vec![Field {
                bit_flags: Some(bit_flags),
                ..make_field("x", FieldType::U8, LengthExpr::Fixed(1))
            }],
        )]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("bit_flags key '99' is not a valid bit index")),
            "Expected bit_flags key warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validator_both_enum_and_bitflags() {
        use std::collections::HashMap;
        let mut enum_values = HashMap::new();
        enum_values.insert("0".to_string(), "Zero".to_string());
        let mut bit_flags = HashMap::new();
        bit_flags.insert("0".to_string(), "Bit 0".to_string());

        let template = make_template(vec![make_region(
            "r",
            OffsetExpr::Absolute(0),
            vec![Field {
                enum_values: Some(enum_values),
                bit_flags: Some(bit_flags),
                ..make_field("x", FieldType::U8, LengthExpr::Fixed(1))
            }],
        )]);

        let warnings = validate(&template);
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("both enum_values and bit_flags")),
            "Expected dual-spec warning, got: {:?}",
            warnings
        );
    }
}
