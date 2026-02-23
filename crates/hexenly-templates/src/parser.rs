use std::path::Path;

use crate::schema::Template;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

/// Parse a template from a TOML file on disk.
pub fn parse_template(path: &Path) -> Result<Template, ParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_template_str(&content)
}

/// Parse a template from an in-memory TOML string.
pub fn parse_template_str(toml_str: &str) -> Result<Template, ParseError> {
    let template: Template = toml::from_str(toml_str)?;
    basic_validate(&template)?;
    Ok(template)
}

fn basic_validate(t: &Template) -> Result<(), ParseError> {
    if t.name.trim().is_empty() {
        return Err(ParseError::Validation("template name must not be empty".into()));
    }
    if t.regions.is_empty() {
        return Err(ParseError::Validation("template must have at least one region".into()));
    }
    for region in &t.regions {
        if region.id.trim().is_empty() {
            return Err(ParseError::Validation("region ID must not be empty".into()));
        }
    }
    Ok(())
}
