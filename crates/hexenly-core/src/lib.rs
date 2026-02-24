pub mod edit_buffer;
pub mod file;
pub mod interpret;
pub mod search;
pub mod selection;

pub use edit_buffer::{EditBuffer, EditMode};
pub use file::HexFile;
pub use interpret::{ByteClass, ByteInterpreter, Interpretation, classify_byte};
pub use search::{SearchPattern, find_all, find_next, find_prev};
pub use selection::{Bookmark, Selection};

#[derive(Debug, thiserror::Error)]
pub enum HexError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("File is empty")]
    EmptyFile,
    #[error("No file path set")]
    NoFilePath,
}
