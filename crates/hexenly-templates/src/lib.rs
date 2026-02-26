//! Binary template engine for structured file overlays.
//!
//! Templates are defined in TOML and describe the layout of binary file formats
//! (regions, fields, offsets, lengths). The engine resolves a parsed template
//! against actual file bytes to produce [`resolved::ResolvedTemplate`] with
//! concrete offsets and display values.
//!
//! This crate is GUI-agnostic — it uses [`resolved::TemplateColor`] instead of
//! framework-specific color types.

pub mod schema;
pub mod resolved;
pub mod parser;
pub mod engine;
pub mod loader;
pub mod validator;
