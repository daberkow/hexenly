use std::path::{Path, PathBuf};

use crate::parser;
use crate::schema::Template;

/// A loaded template with metadata about its source.
#[derive(Debug, Clone)]
pub struct TemplateEntry {
    pub template: Template,
    pub source_path: Option<PathBuf>,
    pub category: String,
}

/// Registry of loaded templates with auto-detection support.
#[derive(Debug, Default)]
pub struct TemplateRegistry {
    pub entries: Vec<TemplateEntry>,
    pub load_errors: Vec<(String, String)>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all .toml templates recursively from a directory.
    /// Subdirectory names become the category.
    pub fn load_from_directory(&mut self, root: &Path) {
        self.scan_dir(root, root);
    }

    fn scan_dir(&mut self, dir: &Path, root: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir(&path, root);
            } else if path.extension().is_some_and(|e| e == "toml") {
                let category = path
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "other".into());

                match parser::parse_template(&path) {
                    Ok(template) => {
                        self.entries.push(TemplateEntry {
                            template,
                            source_path: Some(path),
                            category,
                        });
                    }
                    Err(e) => {
                        self.load_errors
                            .push((path.display().to_string(), e.to_string()));
                    }
                }
            }
        }
    }

    /// Load a built-in template from an embedded TOML string.
    pub fn load_builtin(&mut self, category: &str, name: &str, toml_str: &str) {
        match parser::parse_template_str(toml_str) {
            Ok(template) => {
                self.entries.push(TemplateEntry {
                    template,
                    source_path: None,
                    category: category.into(),
                });
            }
            Err(e) => {
                self.load_errors.push((name.into(), e.to_string()));
            }
        }
    }

    /// Find templates whose magic bytes match the start of the file.
    pub fn detect_for_file(&self, bytes: &[u8]) -> Vec<&TemplateEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                let Some(magic_hex) = &entry.template.magic else {
                    return false;
                };
                let Some(magic_bytes) = hex_str_to_bytes(magic_hex) else {
                    return false;
                };
                let offset = entry.template.magic_offset as usize;
                if offset + magic_bytes.len() > bytes.len() {
                    return false;
                }
                bytes[offset..offset + magic_bytes.len()] == magic_bytes
            })
            .collect()
    }

    /// Find templates matching a file extension.
    pub fn detect_for_extension(&self, ext: &str) -> Vec<&TemplateEntry> {
        let ext_lower = ext.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .template
                    .extensions
                    .iter()
                    .any(|e| e.to_lowercase() == ext_lower)
            })
            .collect()
    }
}

/// Convert a hex string like "89504E47" into bytes.
pub fn hex_str_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}
