use std::collections::HashSet;

use crate::resolved::TemplateColor;
use crate::schema::Template;

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub message: String,
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Validate a parsed template and return non-fatal warnings.
pub fn validate(template: &Template) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    // Check duplicate region IDs
    let mut region_ids = HashSet::new();
    for region in &template.regions {
        if !region_ids.insert(&region.id) {
            warnings.push(ValidationWarning {
                message: format!("duplicate region ID: '{}'", region.id),
            });
        }

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
        }

        // Validate color format
        if let Some(color) = &region.color {
            if TemplateColor::from_hex(color).is_none() {
                warnings.push(ValidationWarning {
                    message: format!(
                        "invalid color '{}' in region '{}' (expected #RRGGBB)",
                        color, region.id
                    ),
                });
            }
        }
    }

    warnings
}
