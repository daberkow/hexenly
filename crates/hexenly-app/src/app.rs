use std::path::PathBuf;
use std::time::Instant;

use eframe::App;
use egui::{CentralPanel, Color32, Context, Key, Layout, RichText, SidePanel, TopBottomPanel};
use hexenly_core::{Bookmark, EditBuffer, EditMode, HexFile, SearchPattern, Selection, find_all};
use hexenly_templates::engine::{self, ResolveResult};
use hexenly_templates::loader::TemplateRegistry;
use hexenly_templates::resolved::ResolvedTemplate;

use crate::panels::bookmarks::{self, BookmarkAction};
use crate::panels::hex_view::{self, HexPane, HexViewAction, HexViewState};
use crate::panels::inspector;
use crate::panels::structure::{self, StructureAction};
use crate::panels::templates::{self, TemplateBrowserAction};
use crate::theme::{HexColors, ThemeMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Ascii,
    Utf8,
}

#[derive(Debug, Clone)]
enum NotificationLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
struct Notification {
    message: String,
    level: NotificationLevel,
    created: Instant,
}

const NOTIFICATION_DURATION_SECS: f32 = 5.0;
const DEFAULT_COLUMNS: usize = 16;
const PAGE_SCROLL_ROWS: usize = 16;
const MAX_SEARCH_RESULTS: usize = 10_000;
const MAX_RECENT_FILES: usize = 10;

struct PanelVisibility {
    inspector: bool,
    ascii_pane: bool,
    goto: bool,
    search: bool,
    replace: bool,
    template_browser: bool,
    structure: bool,
    bookmarks: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            inspector: true,
            ascii_pane: true,
            goto: false,
            search: false,
            replace: false,
            template_browser: false,
            structure: false,
            bookmarks: false,
        }
    }
}

#[derive(Default)]
struct SearchState {
    input: String,
    hex_mode: bool,
    matches: Vec<usize>,
    match_idx: Option<usize>,
    error: Option<String>,
    replace_input: String,
    focus: bool,
}

#[derive(Default)]
struct GotoState {
    input: String,
}

pub struct HexenlyApp {
    file: Option<HexFile>,
    edit_buffer: Option<EditBuffer>,
    cursor_offset: usize,
    selection: Option<Selection>,
    selection_anchor: Option<usize>,
    selection_pane: HexPane,
    pending_copy: bool,
    columns: usize,
    auto_columns: bool,
    text_encoding: TextEncoding,

    panels: PanelVisibility,
    search: SearchState,
    goto: GotoState,

    hex_view_state: HexViewState,
    theme_applied: bool,
    theme_mode: ThemeMode,
    hex_colors: HexColors,

    // File to open on first frame (from CLI args)
    pending_open: Option<String>,

    // Template state
    template_registry: TemplateRegistry,
    active_template_index: Option<usize>,
    resolved_template: Option<ResolvedTemplate>,
    template_filter: String,
    notifications: Vec<Notification>,

    bookmarks: Vec<Bookmark>,
    recent_files: Vec<PathBuf>,

    /// True = waiting for high nibble (first digit), false = waiting for low nibble.
    nibble_high: bool,
    /// Which pane has edit focus for keyboard input.
    edit_focus: HexPane,

    /// When true, the next close request will not be cancelled (force quit).
    force_closing: bool,
}

impl HexenlyApp {
    pub fn new(path: Option<String>) -> Self {
        let mut registry = TemplateRegistry::new();

        // Load built-in templates
        registry.load_builtin(
            "images",
            "PNG",
            include_str!("../../../templates/images/png.toml"),
        );
        registry.load_builtin(
            "images",
            "BMP",
            include_str!("../../../templates/images/bmp.toml"),
        );
        registry.load_builtin(
            "executables",
            "ELF",
            include_str!("../../../templates/executables/elf.toml"),
        );
        registry.load_builtin(
            "archives",
            "ZIP",
            include_str!("../../../templates/archives/zip.toml"),
        );
        registry.load_builtin(
            "filesystems",
            "ISO 9660",
            include_str!("../../../templates/filesystems/iso9660.toml"),
        );
        registry.load_builtin(
            "filesystems",
            "FAT32",
            include_str!("../../../templates/filesystems/fat32.toml"),
        );
        registry.load_builtin(
            "filesystems",
            "Cybiko CFS",
            include_str!("../../../templates/filesystems/cybiko-cfs.toml"),
        );
        registry.load_builtin(
            "filesystems",
            "MBR",
            include_str!("../../../templates/filesystems/mbr.toml"),
        );
        registry.load_builtin(
            "filesystems",
            "GPT",
            include_str!("../../../templates/filesystems/gpt.toml"),
        );
        registry.load_builtin(
            "images",
            "GIF",
            include_str!("../../../templates/images/gif.toml"),
        );
        registry.load_builtin(
            "images",
            "JPEG",
            include_str!("../../../templates/images/jpeg.toml"),
        );
        registry.load_builtin(
            "media",
            "WAV",
            include_str!("../../../templates/media/wav.toml"),
        );
        registry.load_builtin(
            "executables",
            "PE",
            include_str!("../../../templates/executables/pe.toml"),
        );
        registry.load_builtin(
            "archives",
            "TAR",
            include_str!("../../../templates/archives/tar.toml"),
        );
        registry.load_builtin(
            "archives",
            "GZIP",
            include_str!("../../../templates/archives/gzip.toml"),
        );

        let mut notifications = Vec::new();
        for (name, err) in &registry.load_errors {
            tracing::error!("Failed to load template {name}: {err}");
            notifications.push(Notification {
                message: format!("Failed to load template {name}: {err}"),
                level: NotificationLevel::Error,
                created: Instant::now(),
            });
        }

        Self {
            file: None,
            edit_buffer: None,
            cursor_offset: 0,
            selection: None,
            selection_anchor: None,
            selection_pane: HexPane::Hex,
            pending_copy: false,
            columns: DEFAULT_COLUMNS,
            auto_columns: true,
            text_encoding: TextEncoding::Ascii,
            panels: PanelVisibility::default(),
            search: SearchState::default(),
            goto: GotoState::default(),
            hex_view_state: HexViewState::default(),
            theme_applied: false,
            theme_mode: ThemeMode::Dark,
            hex_colors: HexColors::dark(),
            pending_open: path,
            template_registry: registry,
            active_template_index: None,
            resolved_template: None,
            template_filter: String::new(),
            notifications,
            bookmarks: Vec::new(),
            recent_files: load_recent_files(),
            nibble_high: true,
            edit_focus: HexPane::Hex,
            force_closing: false,
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_path(&path);
        }
    }

    fn open_path(&mut self, path: &std::path::Path) {
        match HexFile::open(path) {
            Ok(f) => {
                self.file = Some(f);
                let file_ref = self.file.as_ref().unwrap();

                // Warn if file is larger than 100 MB
                const LARGE_FILE_THRESHOLD: usize = 100 * 1024 * 1024;
                if file_ref.len() > LARGE_FILE_THRESHOLD {
                    let size_str = format_size(file_ref.len());
                    tracing::warn!("Large file ({size_str}) — editing buffer may use significant memory");
                    self.notifications.push(Notification {
                        message: format!("Large file ({size_str}) — editing buffer may use significant memory"),
                        level: NotificationLevel::Warning,
                        created: Instant::now(),
                    });
                }

                self.edit_buffer = Some(EditBuffer::from_file(file_ref));
                self.cursor_offset = 0;
                self.selection = None;
                self.nibble_high = true;
                self.search.matches.clear();
                self.search.match_idx = None;
                self.active_template_index = None;
                self.resolved_template = None;
                self.bookmarks = load_bookmarks(path);

                // Track in recent files
                let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                self.recent_files.retain(|p| p != &canonical);
                self.recent_files.insert(0, canonical);
                self.recent_files.truncate(MAX_RECENT_FILES);
                save_recent_files(&self.recent_files);

                // Auto-detect template
                self.auto_detect_template(path);
            }
            Err(e) => {
                tracing::error!("Failed to open file: {e}");
            }
        }
    }

    fn data_bytes(&self) -> Option<&[u8]> {
        if let Some(buf) = &self.edit_buffer {
            Some(buf.data())
        } else {
            self.file.as_ref().map(|f| f.as_bytes())
        }
    }

    fn data_len(&self) -> usize {
        self.edit_buffer
            .as_ref()
            .map(|b| b.len())
            .or_else(|| self.file.as_ref().map(|f| f.len()))
            .unwrap_or(0)
    }

    fn auto_detect_template(&mut self, path: &std::path::Path) {
        let Some(bytes) = self.data_bytes() else { return };

        // Try magic bytes first
        let matches = self.template_registry.detect_for_file(bytes);
        if let Some(entry) = matches.first() {
            let idx = self
                .template_registry
                .entries
                .iter()
                .position(|e| std::ptr::eq(e, *entry));
            if let Some(idx) = idx {
                self.active_template_index = Some(idx);
                self.resolve_active_template();
                self.panels.structure = true;
                return;
            }
        }

        // Fall back to extension matching
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let matches = self.template_registry.detect_for_extension(ext);
            if let Some(entry) = matches.first() {
                let idx = self
                    .template_registry
                    .entries
                    .iter()
                    .position(|e| std::ptr::eq(e, *entry));
                if let Some(idx) = idx {
                    self.active_template_index = Some(idx);
                    self.resolve_active_template();
                    self.panels.structure = true;
                    return;
                }
            }
        }

        // No template matched — open the template browser so the user can pick one
        self.panels.template_browser = true;
    }

    fn resolve_active_template(&mut self) {
        let Some(data) = self.data_bytes() else {
            self.resolved_template = None;
            return;
        };
        let Some(idx) = self.active_template_index else {
            self.resolved_template = None;
            return;
        };
        let Some(entry) = self.template_registry.entries.get(idx) else {
            self.resolved_template = None;
            return;
        };

        let result: ResolveResult = engine::resolve(&entry.template, data);

        for warning in &result.warnings {
            tracing::warn!("Template resolve: {warning}");
            self.notifications.push(Notification {
                message: format!("Template: {warning}"),
                level: NotificationLevel::Warning,
                created: Instant::now(),
            });
        }

        self.resolved_template = Some(result.template);
    }

    fn do_search(&mut self) {
        self.search.error = None;
        let Some(data) = self.data_bytes() else { return };
        let pattern = if self.search.hex_mode {
            match SearchPattern::from_hex_string(&self.search.input) {
                Some(p) => p,
                None => {
                    self.search.error = Some("Invalid hex pattern".into());
                    return;
                }
            }
        } else {
            SearchPattern::from_text(&self.search.input)
        };

        self.search.matches = find_all(data, &pattern, MAX_SEARCH_RESULTS);
        if let Some(&first) = self.search.matches.first() {
            self.search.match_idx = Some(0);
            self.cursor_offset = first;
            self.scroll_to_cursor();
        } else {
            self.search.match_idx = None;
        }
    }

    fn search_next(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let idx = self
            .search
            .match_idx
            .map(|i| (i + 1) % self.search.matches.len())
            .unwrap_or(0);
        self.search.match_idx = Some(idx);
        self.cursor_offset = self.search.matches[idx];
        self.scroll_to_cursor();
    }

    fn search_prev(&mut self) {
        if self.search.matches.is_empty() {
            return;
        }
        let len = self.search.matches.len();
        let idx = self
            .search
            .match_idx
            .map(|i| (i + len - 1) % len)
            .unwrap_or(len - 1);
        self.search.match_idx = Some(idx);
        self.cursor_offset = self.search.matches[idx];
        self.scroll_to_cursor();
    }

    fn build_replace_bytes(&self) -> Option<Vec<u8>> {
        if self.search.hex_mode {
            SearchPattern::from_hex_string(&self.search.replace_input)
                .map(|p| p.as_bytes().to_vec())
        } else {
            Some(self.search.replace_input.as_bytes().to_vec())
        }
    }

    fn replace_current(&mut self) {
        let Some(idx) = self.search.match_idx else { return };
        let Some(&match_offset) = self.search.matches.get(idx) else { return };
        let Some(replace_bytes) = self.build_replace_bytes() else { return };
        let Some(buf) = &mut self.edit_buffer else { return };

        // Determine the length of the search pattern
        let old_len = if self.search.hex_mode {
            SearchPattern::from_hex_string(&self.search.input)
                .map(|p| p.as_bytes().len())
                .unwrap_or(0)
        } else {
            self.search.input.len()
        };
        if old_len == 0 {
            return;
        }

        buf.replace_range(match_offset, old_len, &replace_bytes);

        // Clamp cursor
        if !buf.is_empty() {
            self.cursor_offset = self.cursor_offset.min(buf.len() - 1);
        } else {
            self.cursor_offset = 0;
        }

        // Re-run search to update matches
        self.do_search();
    }

    fn replace_all_matches(&mut self) {
        let Some(replace_bytes) = self.build_replace_bytes() else { return };
        let Some(buf) = &mut self.edit_buffer else { return };
        if self.search.matches.is_empty() {
            return;
        }

        let old_len = if self.search.hex_mode {
            SearchPattern::from_hex_string(&self.search.input)
                .map(|p| p.as_bytes().len())
                .unwrap_or(0)
        } else {
            self.search.input.len()
        };
        if old_len == 0 {
            return;
        }

        // Clone matches and iterate in reverse so offsets stay valid
        let matches: Vec<usize> = self.search.matches.clone();
        for &match_offset in matches.iter().rev() {
            buf.replace_range(match_offset, old_len, &replace_bytes);
        }

        // Clamp cursor
        if !buf.is_empty() {
            self.cursor_offset = self.cursor_offset.min(buf.len() - 1);
        } else {
            self.cursor_offset = 0;
        }

        // Re-run search
        self.do_search();
    }

    fn goto_offset(&mut self) {
        let text = self.goto.input.trim();
        let offset = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).ok()
        } else {
            text.parse::<usize>().ok()
        };
        let len = self.data_len();
        if let Some(off) = offset
            && len > 0
            && off < len
        {
            self.cursor_offset = off;
            self.selection = None;
            self.scroll_to_cursor();
        }
        self.panels.goto = false;
        self.goto.input.clear();
    }

    fn show_notifications(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.notifications.retain(|n| now.duration_since(n.created).as_secs_f32() < NOTIFICATION_DURATION_SECS);

        if self.notifications.is_empty() {
            return;
        }

        egui::Area::new(egui::Id::new("notifications"))
            .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for notification in &self.notifications {
                        let elapsed = now.duration_since(notification.created).as_secs_f32();
                        let alpha = if elapsed > NOTIFICATION_DURATION_SECS - 1.0 {
                            ((NOTIFICATION_DURATION_SECS - elapsed).max(0.0) * 255.0) as u8
                        } else {
                            255
                        };
                        let color = match notification.level {
                            NotificationLevel::Error => Color32::from_rgba_unmultiplied(220, 80, 80, alpha),
                            NotificationLevel::Warning => Color32::from_rgba_unmultiplied(220, 180, 60, alpha),
                        };
                        ui.label(RichText::new(&notification.message).color(color));
                    }
                });
            });

        ctx.request_repaint();
    }

    fn scroll_to_cursor(&mut self) {
        let row = self.cursor_offset / self.columns;
        self.hex_view_state.scroll_to_row = Some(row);
    }

    /// Move cursor by a signed delta, clamping to valid range.
    fn move_cursor(&mut self, delta: isize) {
        let len = self.data_len();
        if len == 0 {
            return;
        }
        let max = len - 1;
        let new_offset = if delta < 0 {
            self.cursor_offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.cursor_offset.saturating_add(delta as usize).min(max)
        };
        self.cursor_offset = new_offset;
        self.selection = None;
        self.selection_anchor = None;
        self.nibble_high = true;
        self.scroll_to_cursor();
    }

    /// Set cursor to an absolute offset, clamping to valid range.
    fn set_cursor_abs(&mut self, offset: usize) {
        let len = self.data_len();
        if len == 0 {
            return;
        }
        self.cursor_offset = offset.min(len - 1);
        self.selection = None;
        self.selection_anchor = None;
        self.nibble_high = true;
        self.scroll_to_cursor();
    }

    /// Move cursor by a signed delta, extending the selection from the anchor.
    fn move_cursor_select(&mut self, delta: isize) {
        let len = self.data_len();
        if len == 0 {
            return;
        }
        let max = len.saturating_sub(1);
        let anchor = self.selection_anchor.unwrap_or(self.cursor_offset);
        self.selection_anchor = Some(anchor);
        if delta < 0 {
            self.cursor_offset = self.cursor_offset.saturating_sub(delta.unsigned_abs());
        } else {
            self.cursor_offset = self.cursor_offset.saturating_add(delta as usize).min(max);
        }
        self.selection = Some(Selection::new(anchor, self.cursor_offset));
        self.nibble_high = true;
        self.scroll_to_cursor();
    }

    /// Set cursor to an absolute offset, extending the selection from the anchor.
    fn set_cursor_select(&mut self, offset: usize) {
        let len = self.data_len();
        if len == 0 {
            return;
        }
        let anchor = self.selection_anchor.unwrap_or(self.cursor_offset);
        self.selection_anchor = Some(anchor);
        self.cursor_offset = offset.min(len.saturating_sub(1));
        self.selection = Some(Selection::new(anchor, self.cursor_offset));
        self.nibble_high = true;
        self.scroll_to_cursor();
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        #[allow(clippy::struct_excessive_bools)]
        struct NavKeys {
            left: bool,
            right: bool,
            up: bool,
            down: bool,
            page_up: bool,
            page_down: bool,
            home: bool,
            end: bool,
            ctrl_home: bool,
            ctrl_end: bool,
        }

        #[allow(clippy::struct_excessive_bools)]
        struct ShiftNavKeys {
            left: bool,
            right: bool,
            up: bool,
            down: bool,
            page_up: bool,
            page_down: bool,
            home: bool,
            end: bool,
            ctrl_home: bool,
            ctrl_end: bool,
        }

        let (open, save, save_as, undo, redo, select_all, insert_key, goto, find, find_replace, escape, copy, nav, shift_nav, add_bookmark, prev_bookmark, next_bookmark, delete, backspace, force_quit) = ctx.input_mut(|i| {
            let force_quit = i.consume_key(egui::Modifiers::COMMAND, Key::Q);

            // Ctrl+Shift+S must be consumed BEFORE Ctrl+S
            let save_as = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::S,
            );
            let save = i.consume_key(egui::Modifiers::COMMAND, Key::S);

            // Ctrl+Shift+Z must be consumed BEFORE Ctrl+Z
            let redo = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::Z,
            );
            let undo = i.consume_key(egui::Modifiers::COMMAND, Key::Z);

            let select_all = i.consume_key(egui::Modifiers::COMMAND, Key::A);
            let insert_key = i.consume_key(egui::Modifiers::NONE, Key::Insert);

            let open = i.consume_key(egui::Modifiers::COMMAND, Key::O);
            let goto = i.consume_key(egui::Modifiers::COMMAND, Key::G);
            let find = i.consume_key(egui::Modifiers::COMMAND, Key::F);
            let find_replace = i.consume_key(egui::Modifiers::COMMAND, Key::H);
            let escape = i.consume_key(egui::Modifiers::NONE, Key::Escape);
            // eframe converts Ctrl+C into Event::Copy, not a key event
            let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy));

            // Ctrl+B to add bookmark
            let add_bookmark = i.consume_key(egui::Modifiers::COMMAND, Key::B);

            // Ctrl+Home/End must be consumed before plain Home/End
            let ctrl_home = i.consume_key(egui::Modifiers::COMMAND, Key::Home);
            let ctrl_end = i.consume_key(egui::Modifiers::COMMAND, Key::End);

            // Shift+Ctrl+Home/End must be consumed before Shift+Home/End
            let shift_ctrl_home = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::Home,
            );
            let shift_ctrl_end = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::End,
            );

            // Ctrl+Shift+Up/Down for bookmark navigation — must be consumed
            // BEFORE Shift+Up/Down to prevent the simpler modifier match
            // from swallowing it.
            let prev_bookmark = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::ArrowUp,
            );
            let next_bookmark = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::ArrowDown,
            );

            let left = i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft);
            let right = i.consume_key(egui::Modifiers::NONE, Key::ArrowRight);
            let up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
            let down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
            let page_up = i.consume_key(egui::Modifiers::NONE, Key::PageUp);
            let page_down = i.consume_key(egui::Modifiers::NONE, Key::PageDown);
            let home = i.consume_key(egui::Modifiers::NONE, Key::Home);
            let end = i.consume_key(egui::Modifiers::NONE, Key::End);

            let shift_left = i.consume_key(egui::Modifiers::SHIFT, Key::ArrowLeft);
            let shift_right = i.consume_key(egui::Modifiers::SHIFT, Key::ArrowRight);
            let shift_up = i.consume_key(egui::Modifiers::SHIFT, Key::ArrowUp);
            let shift_down = i.consume_key(egui::Modifiers::SHIFT, Key::ArrowDown);
            let shift_page_up = i.consume_key(egui::Modifiers::SHIFT, Key::PageUp);
            let shift_page_down = i.consume_key(egui::Modifiers::SHIFT, Key::PageDown);
            let shift_home = i.consume_key(egui::Modifiers::SHIFT, Key::Home);
            let shift_end = i.consume_key(egui::Modifiers::SHIFT, Key::End);

            let delete = i.consume_key(egui::Modifiers::NONE, Key::Delete);
            let backspace = i.consume_key(egui::Modifiers::NONE, Key::Backspace);

            let nav = NavKeys {
                left,
                right,
                up,
                down,
                page_up,
                page_down,
                home,
                end,
                ctrl_home,
                ctrl_end,
            };

            let shift_nav = ShiftNavKeys {
                left: shift_left,
                right: shift_right,
                up: shift_up,
                down: shift_down,
                page_up: shift_page_up,
                page_down: shift_page_down,
                home: shift_home,
                end: shift_end,
                ctrl_home: shift_ctrl_home,
                ctrl_end: shift_ctrl_end,
            };

            (open, save, save_as, undo, redo, select_all, insert_key, goto, find, find_replace, escape, copy, nav, shift_nav, add_bookmark, prev_bookmark, next_bookmark, delete, backspace, force_quit)
        });

        if force_quit {
            self.force_closing = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if open {
            self.open_file_dialog();
        }
        if save {
            self.save_file();
        }
        if save_as {
            self.save_file_as();
        }
        if undo
            && let Some(buf) = &mut self.edit_buffer
        {
            buf.undo();
        }
        if redo
            && let Some(buf) = &mut self.edit_buffer
        {
            buf.redo();
        }
        if select_all && self.data_len() > 0 {
            let max = self.data_len() - 1;
            self.selection = Some(Selection::new(0, max));
            self.selection_anchor = Some(0);
        }
        if insert_key
            && let Some(buf) = &mut self.edit_buffer
        {
            buf.toggle_mode();
        }
        if goto {
            self.panels.goto = !self.panels.goto;
            self.panels.search = false;
        }
        if find {
            self.panels.search = !self.panels.search;
            self.search.focus = self.panels.search;
            self.panels.goto = false;
        }
        if find_replace {
            self.panels.search = true;
            self.panels.replace = true;
            self.search.focus = true;
            self.panels.goto = false;
        }
        if escape {
            self.panels.goto = false;
            self.panels.search = false;
            self.panels.replace = false;
        }
        // copy is handled at end of update() so nothing overwrites the clipboard
        if copy && self.selection.is_some() {
            self.pending_copy = true;
        }

        // Bookmark shortcuts
        if add_bookmark && self.file.is_some() {
            let (offset, end) = match &self.selection {
                Some(sel) => (sel.start, Some(sel.end)),
                None => (self.cursor_offset, None),
            };
            self.bookmarks.push(Bookmark {
                name: format!("Bookmark {}", self.bookmarks.len() + 1),
                offset,
                end,
                note: String::new(),
            });
            self.bookmarks.sort_by_key(|b| b.offset);
            if let Some(file) = &self.file {
                save_bookmarks(file.path(), &self.bookmarks);
            }
            self.panels.bookmarks = true;
        }
        if prev_bookmark
            && let Some(bm) = self.bookmarks.iter().rev().find(|b| b.offset < self.cursor_offset)
        {
            let off = bm.offset;
            self.set_cursor_abs(off);
        }
        if next_bookmark
            && let Some(bm) = self.bookmarks.iter().find(|b| b.offset > self.cursor_offset)
        {
            let off = bm.offset;
            self.set_cursor_abs(off);
        }

        // Delete / Backspace
        if (delete || backspace) && self.edit_buffer.is_some() {
            self.handle_delete(delete);
        }

        // Keyboard navigation (only when data is available)
        if self.data_len() > 0 {
            let data_len = self.data_len();

            if nav.left {
                self.move_cursor(-1);
            }
            if nav.right {
                self.move_cursor(1);
            }
            if nav.up {
                self.move_cursor(-(self.columns as isize));
            }
            if nav.down {
                self.move_cursor(self.columns as isize);
            }
            if nav.page_up {
                self.move_cursor(-((self.columns * PAGE_SCROLL_ROWS) as isize));
            }
            if nav.page_down {
                self.move_cursor((self.columns * PAGE_SCROLL_ROWS) as isize);
            }
            if nav.home {
                let row_start = (self.cursor_offset / self.columns) * self.columns;
                self.set_cursor_abs(row_start);
            }
            if nav.end {
                let row_start = (self.cursor_offset / self.columns) * self.columns;
                let row_end = (row_start + self.columns - 1).min(data_len.saturating_sub(1));
                self.set_cursor_abs(row_end);
            }
            if nav.ctrl_home {
                self.set_cursor_abs(0);
            }
            if nav.ctrl_end {
                self.set_cursor_abs(data_len.saturating_sub(1));
            }

            // Shift+key selection
            if shift_nav.left {
                self.move_cursor_select(-1);
            }
            if shift_nav.right {
                self.move_cursor_select(1);
            }
            if shift_nav.up {
                self.move_cursor_select(-(self.columns as isize));
            }
            if shift_nav.down {
                self.move_cursor_select(self.columns as isize);
            }
            if shift_nav.page_up {
                self.move_cursor_select(-((self.columns * PAGE_SCROLL_ROWS) as isize));
            }
            if shift_nav.page_down {
                self.move_cursor_select((self.columns * PAGE_SCROLL_ROWS) as isize);
            }
            if shift_nav.home {
                let row_start = (self.cursor_offset / self.columns) * self.columns;
                self.set_cursor_select(row_start);
            }
            if shift_nav.end {
                let row_start = (self.cursor_offset / self.columns) * self.columns;
                let row_end = (row_start + self.columns - 1).min(data_len.saturating_sub(1));
                self.set_cursor_select(row_end);
            }
            if shift_nav.ctrl_home {
                self.set_cursor_select(0);
            }
            if shift_nav.ctrl_end {
                self.set_cursor_select(data_len.saturating_sub(1));
            }
        }
    }

    fn handle_edit_input(&mut self, ctx: &Context) {
        let Some(buf) = &self.edit_buffer else { return };
        if buf.is_empty() {
            return;
        }

        // Collect text events (typed characters)
        let typed_chars: Vec<char> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Text(s) = e {
                        s.chars().next()
                    } else {
                        None
                    }
                })
                .collect()
        });

        for ch in typed_chars {
            match self.edit_focus {
                HexPane::Hex => self.handle_hex_input(ch),
                HexPane::Ascii => self.handle_ascii_input(ch),
            }
        }
    }

    fn handle_hex_input(&mut self, ch: char) {
        let Some(digit) = ch.to_digit(16) else { return };
        let digit = digit as u8;
        let Some(buf) = &mut self.edit_buffer else { return };

        // If selection exists, delete it first (insert mode) or start from selection start
        if let Some(sel) = self.selection.take() {
            if buf.mode() == EditMode::Insert {
                buf.delete_range(sel.start, sel.end);
            }
            self.cursor_offset = sel.start.min(buf.len().saturating_sub(1));
            self.selection_anchor = None;
            self.nibble_high = true;
        }

        let offset = self.cursor_offset;

        if buf.mode() == EditMode::Insert && self.nibble_high {
            // Insert a new 0x00 byte, then we'll set the high nibble
            buf.insert_byte(offset, 0x00);
        }

        if offset >= buf.len() {
            return;
        }
        let current = buf.data()[offset];
        let new_byte = if self.nibble_high {
            (digit << 4) | (current & 0x0F)
        } else {
            (current & 0xF0) | digit
        };
        buf.overwrite_byte(offset, new_byte);

        if self.nibble_high {
            self.nibble_high = false;
        } else {
            self.nibble_high = true;
            // Advance cursor
            let max = buf.len().saturating_sub(1);
            if self.cursor_offset < max {
                self.cursor_offset += 1;
            }
        }
    }

    fn handle_ascii_input(&mut self, ch: char) {
        if !ch.is_ascii() || ch.is_ascii_control() {
            return;
        }
        let Some(buf) = &mut self.edit_buffer else { return };

        // If selection exists, delete it first (insert mode) or start from selection start
        if let Some(sel) = self.selection.take() {
            if buf.mode() == EditMode::Insert {
                buf.delete_range(sel.start, sel.end);
            }
            self.cursor_offset = sel.start.min(buf.len().saturating_sub(1));
            self.selection_anchor = None;
        }

        let offset = self.cursor_offset;

        if buf.mode() == EditMode::Insert {
            buf.insert_byte(offset, ch as u8);
        } else {
            buf.overwrite_byte(offset, ch as u8);
        }

        // Advance cursor
        let max = buf.len().saturating_sub(1);
        if self.cursor_offset < max {
            self.cursor_offset += 1;
        }
        self.nibble_high = true;
    }

    fn handle_delete(&mut self, is_forward: bool) {
        let Some(buf) = &mut self.edit_buffer else { return };
        if buf.is_empty() {
            return;
        }

        // If there's a selection, operate on the range
        if let Some(sel) = self.selection.take() {
            if buf.mode() == EditMode::Insert {
                buf.delete_range(sel.start, sel.end);
                self.cursor_offset = sel.start.min(buf.len().saturating_sub(1));
            } else {
                // Overwrite mode: zero the selected bytes
                let zeros = vec![0u8; sel.len()];
                buf.overwrite_range(sel.start, &zeros);
                self.cursor_offset = sel.start;
            }
            self.selection_anchor = None;
            self.nibble_high = true;
            return;
        }

        // No selection -- single byte operation
        if buf.mode() == EditMode::Insert {
            if is_forward {
                // Delete key: remove byte at cursor
                buf.delete_byte(self.cursor_offset);
                if self.cursor_offset >= buf.len() && !buf.is_empty() {
                    self.cursor_offset = buf.len() - 1;
                }
            } else {
                // Backspace: remove byte before cursor
                if self.cursor_offset > 0 {
                    self.cursor_offset -= 1;
                    buf.delete_byte(self.cursor_offset);
                }
            }
        } else {
            // Overwrite mode: zero the byte
            buf.overwrite_byte(self.cursor_offset, 0x00);
        }
        self.nibble_high = true;
    }

    fn selected_bytes(&self) -> Option<&[u8]> {
        let sel = self.selection.as_ref()?;
        let bytes = self.data_bytes()?;
        let start = sel.start.min(bytes.len());
        let end = (sel.end + 1).min(bytes.len());
        Some(&bytes[start..end])
    }

    fn copy_selection_hex(&self, ctx: &Context) {
        let Some(selected) = self.selected_bytes() else { return };
        let hex: Vec<String> = selected.iter().map(|b| format!("{b:02X}")).collect();
        ctx.copy_text(hex.join(" "));
    }

    fn copy_selection_text(&self, ctx: &Context) {
        let Some(selected) = self.selected_bytes() else { return };
        let text = match self.text_encoding {
            TextEncoding::Ascii => {
                selected
                    .iter()
                    .map(|&b| {
                        if (0x20..=0x7E).contains(&b) { b as char } else { '.' }
                    })
                    .collect::<String>()
            }
            TextEncoding::Utf8 => {
                String::from_utf8_lossy(selected).into_owned()
            }
        };
        ctx.copy_text(text);
    }

    fn show_menu_bar(&mut self, ui: &mut egui::Ui) {
        let file_open = self.edit_buffer.is_some();
        let is_dirty = self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty());
        let can_undo = self.edit_buffer.as_ref().is_some_and(|b| b.can_undo());
        let can_redo = self.edit_buffer.as_ref().is_some_and(|b| b.can_redo());
        let data_len = self.data_len();

        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(egui::Button::new("Open").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    ui.close();
                    self.open_file_dialog();
                }
                if ui
                    .add_enabled(
                        file_open && is_dirty,
                        egui::Button::new("Save").shortcut_text("Ctrl+S"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.save_file();
                }
                if ui
                    .add_enabled(
                        file_open,
                        egui::Button::new("Save As...").shortcut_text("Ctrl+Shift+S"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.save_file_as();
                }
                ui.separator();
                ui.menu_button("Recent Files", |ui| {
                    if self.recent_files.is_empty() {
                        ui.add_enabled(false, egui::Label::new("No recent files"));
                    } else {
                        let mut open_path = None;
                        for path in &self.recent_files {
                            let label = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.to_string_lossy().to_string());
                            if ui
                                .button(&label)
                                .on_hover_text(path.to_string_lossy())
                                .clicked()
                            {
                                open_path = Some(path.clone());
                                ui.close();
                            }
                        }
                        if let Some(path) = open_path {
                            self.open_path(&path);
                        }
                    }
                });
                ui.separator();
                if ui
                    .add(egui::Button::new("Exit").shortcut_text("Ctrl+Q"))
                    .clicked()
                {
                    ui.close();
                    self.force_closing = true;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new("Undo").shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    ui.close();
                    if let Some(buf) = &mut self.edit_buffer {
                        buf.undo();
                    }
                }
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new("Redo").shortcut_text("Ctrl+Shift+Z"),
                    )
                    .clicked()
                {
                    ui.close();
                    if let Some(buf) = &mut self.edit_buffer {
                        buf.redo();
                    }
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Find").shortcut_text("Ctrl+F"))
                    .clicked()
                {
                    ui.close();
                    self.panels.search = !self.panels.search;
                    self.search.focus = self.panels.search;
                    self.panels.goto = false;
                }
                if ui
                    .add(egui::Button::new("Find & Replace").shortcut_text("Ctrl+H"))
                    .clicked()
                {
                    ui.close();
                    self.panels.search = true;
                    self.panels.replace = true;
                    self.search.focus = true;
                    self.panels.goto = false;
                }
                if ui
                    .add(egui::Button::new("Go to Offset").shortcut_text("Ctrl+G"))
                    .clicked()
                {
                    ui.close();
                    self.panels.goto = !self.panels.goto;
                    self.panels.search = false;
                }
                ui.separator();
                if ui
                    .add_enabled(
                        data_len > 0,
                        egui::Button::new("Select All").shortcut_text("Ctrl+A"),
                    )
                    .clicked()
                {
                    ui.close();
                    let max = data_len - 1;
                    self.selection = Some(Selection::new(0, max));
                    self.selection_anchor = Some(0);
                }
            });

            ui.menu_button("View", |ui| {
                ui.menu_button("Columns", |ui| {
                    if ui.selectable_label(self.auto_columns, "Auto").clicked() {
                        self.auto_columns = true;
                        ui.close();
                    }
                    for &n in &[8, 16, 24, 32, 48] {
                        if ui
                            .selectable_label(!self.auto_columns && self.columns == n, format!("{n}"))
                            .clicked()
                        {
                            self.columns = n;
                            self.auto_columns = false;
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Encoding", |ui| {
                    if ui
                        .selectable_label(self.text_encoding == TextEncoding::Ascii, "ASCII")
                        .clicked()
                    {
                        self.text_encoding = TextEncoding::Ascii;
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.text_encoding == TextEncoding::Utf8, "UTF-8")
                        .clicked()
                    {
                        self.text_encoding = TextEncoding::Utf8;
                        ui.close();
                    }
                });
                ui.menu_button("Theme", |ui| {
                    if ui
                        .selectable_label(self.theme_mode == ThemeMode::Dark, "Dark")
                        .clicked()
                    {
                        self.theme_mode = ThemeMode::Dark;
                        self.hex_colors = HexColors::dark();
                        self.theme_applied = false;
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.theme_mode == ThemeMode::Light, "Light")
                        .clicked()
                    {
                        self.theme_mode = ThemeMode::Light;
                        self.hex_colors = HexColors::light();
                        self.theme_applied = false;
                        ui.close();
                    }
                });
                ui.separator();
                if ui
                    .selectable_label(self.panels.ascii_pane, "ASCII Pane")
                    .clicked()
                {
                    self.panels.ascii_pane = !self.panels.ascii_pane;
                    ui.close();
                }
                if ui
                    .selectable_label(self.panels.inspector, "Inspector")
                    .clicked()
                {
                    self.panels.inspector = !self.panels.inspector;
                    ui.close();
                }
                if ui
                    .selectable_label(self.panels.template_browser, "Templates")
                    .clicked()
                {
                    self.panels.template_browser = !self.panels.template_browser;
                    ui.close();
                }
                if ui
                    .selectable_label(self.panels.structure, "Structure")
                    .clicked()
                {
                    self.panels.structure = !self.panels.structure;
                    ui.close();
                }
                if ui
                    .selectable_label(self.panels.bookmarks, "Bookmarks")
                    .clicked()
                {
                    self.panels.bookmarks = !self.panels.bookmarks;
                    ui.close();
                }
            });
        });
    }

    fn save_file(&mut self) {
        if let Some(buf) = &mut self.edit_buffer
            && let Err(e) = buf.save()
        {
            self.notifications.push(Notification {
                message: format!("Save failed: {e}"),
                level: NotificationLevel::Error,
                created: Instant::now(),
            });
        }
    }

    fn save_file_as(&mut self) {
        let Some(buf) = &mut self.edit_buffer else { return };
        let mut dialog = rfd::FileDialog::new();
        if let Some(path) = buf.file_path() {
            if let Some(dir) = path.parent() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy().to_string());
            }
        }
        if let Some(path) = dialog.save_file()
            && let Err(e) = buf.save_as(&path)
        {
            self.notifications.push(Notification {
                message: format!("Save failed: {e}"),
                level: NotificationLevel::Error,
                created: Instant::now(),
            });
        }
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.panels.search {
            return;
        }
        // Row 1: Search
        ui.horizontal(|ui| {
            ui.label("Search:");
            let re = ui.text_edit_singleline(&mut self.search.input);
            if re.changed() {
                self.search.error = None;
            }
            if self.search.focus {
                re.request_focus();
                self.search.focus = false;
            }
            if re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.do_search();
            }
            ui.toggle_value(&mut self.search.hex_mode, "Hex");
            if ui.button("Find").clicked() {
                self.do_search();
            }
            if ui.button("Prev").clicked() {
                self.search_prev();
            }
            if ui.button("Next").clicked() {
                self.search_next();
            }
            if let Some(idx) = self.search.match_idx {
                ui.label(format!(
                    "{}/{}",
                    idx + 1,
                    self.search.matches.len()
                ));
            }
            if let Some(err) = &self.search.error {
                ui.label(RichText::new(err).color(Color32::from_rgb(220, 80, 80)));
            }
            let replace_label = if self.panels.replace { "Replace \u{25B2}" } else { "Replace \u{25BC}" };
            if ui.button(replace_label).clicked() {
                self.panels.replace = !self.panels.replace;
            }
        });
        // Row 2: Replace (when expanded)
        if self.panels.replace {
            ui.horizontal(|ui| {
                ui.label("Replace:");
                ui.text_edit_singleline(&mut self.search.replace_input);
                let has_matches = !self.search.matches.is_empty() && self.edit_buffer.is_some();
                if ui.add_enabled(has_matches, egui::Button::new("Replace")).clicked() {
                    self.replace_current();
                }
                if ui.add_enabled(has_matches, egui::Button::new("Replace All")).clicked() {
                    self.replace_all_matches();
                }
            });
        }
    }

    fn show_goto_bar(&mut self, ui: &mut egui::Ui) {
        if !self.panels.goto {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Go to offset:");
            let re = ui.text_edit_singleline(&mut self.goto.input);
            if re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.goto_offset();
            }
            if ui.button("Go").clicked() {
                self.goto_offset();
            }
            ui.label("(prefix 0x for hex)");
        });
    }

    fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        let mut toggle_mode = false;
        ui.horizontal(|ui| {
            if let Some(file) = &self.file {
                let name = file
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                let dirty = self
                    .edit_buffer
                    .as_ref()
                    .is_some_and(|b| b.is_dirty());
                let display_name = if dirty {
                    format!("{name} *")
                } else {
                    name
                };
                ui.label(RichText::new(&display_name).strong());
                ui.label(RichText::new(format_size(self.data_len())).weak());

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(buf) = &self.edit_buffer {
                        let mode_text = match buf.mode() {
                            EditMode::Overwrite => "OVR",
                            EditMode::Insert => "INS",
                        };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(mode_text).monospace())
                                    .frame(false),
                            )
                            .clicked()
                        {
                            toggle_mode = true;
                        }
                        ui.separator();
                    }
                    if let Some(resolved) = &self.resolved_template {
                        ui.label(&resolved.name);
                        ui.label(RichText::new("Template:").weak());
                        ui.separator();
                    }
                    if let Some(sel) = &self.selection {
                        if ui.small_button("Copy Text").clicked() {
                            self.copy_selection_text(ui.ctx());
                        }
                        if ui.small_button("Copy Hex").clicked() {
                            self.copy_selection_hex(ui.ctx());
                        }
                        ui.label(format!("{} bytes", sel.len()));
                        ui.label(RichText::new("Selected:").weak());
                        ui.separator();
                    }
                    ui.label(format!("0x{:08X} ({})", self.cursor_offset, self.cursor_offset));
                    ui.label(RichText::new("Offset:").weak());
                });
            } else {
                ui.label("No file open \u{2014} Ctrl+O to open");
            }
        });
        if toggle_mode
            && let Some(buf) = &mut self.edit_buffer
        {
            buf.toggle_mode();
        }
    }
}

impl App for HexenlyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            crate::theme::apply_theme(ctx, self.theme_mode);
            self.theme_applied = true;
        }

        // Update window title with dirty indicator
        let title = if let Some(file) = &self.file {
            let name = file.path().file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            let dirty = if self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty()) { " *" } else { "" };
            format!("{name}{dirty} - Hexenly")
        } else {
            "Hexenly".to_string()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Warn before closing with unsaved changes
        if ctx.input(|i| i.viewport().close_requested())
            && !self.force_closing
            && self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notifications.push(Notification {
                message: "Unsaved changes! Save first or press Ctrl+Q to force quit.".into(),
                level: NotificationLevel::Warning,
                created: Instant::now(),
            });
        }

        // Handle pending file open from CLI
        if let Some(path) = self.pending_open.take() {
            self.open_path(std::path::Path::new(&path));
        }

        // Handle drag-and-drop file opening
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.first()
            && let Some(path) = &file.path
        {
            self.open_path(path);
        }

        self.handle_shortcuts(ctx);

        // Only process edit input when not in a text input mode
        if !self.panels.search && !self.panels.goto {
            self.handle_edit_input(ctx);
        }

        // Top menu bar
        TopBottomPanel::top("menubar").show(ctx, |ui| {
            self.show_menu_bar(ui);
            self.show_search_bar(ui);
            self.show_goto_bar(ui);
        });

        // Bottom status bar
        TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            self.show_status_bar(ui);
        });

        // Structure panel (above status bar)
        if self.panels.structure
            && let Some(resolved) = &self.resolved_template
        {
            let resolved_clone = resolved.clone();
            let cursor = self.cursor_offset;
            TopBottomPanel::bottom("structure")
                .default_height(200.0)
                .resizable(true)
                .show(ctx, |ui| {
                    let action = structure::show(ui, &resolved_clone, cursor);
                    match action {
                        Some(StructureAction::GoToOffset(off)) => {
                            self.cursor_offset = off;
                            self.scroll_to_cursor();
                        }
                        Some(StructureAction::Close) => {
                            self.panels.structure = false;
                        }
                        None => {}
                    }
                });
        }

        // Left template browser panel
        if self.panels.template_browser {
            SidePanel::left("templates")
                .default_width(200.0)
                .show(ctx, |ui| {
                    let action = templates::show(
                        ui,
                        &self.template_registry,
                        self.active_template_index,
                        &mut self.template_filter,
                    );
                    match action {
                        Some(TemplateBrowserAction::Select(idx)) => {
                            self.active_template_index = Some(idx);
                            self.resolve_active_template();
                            self.panels.structure = true;
                        }
                        Some(TemplateBrowserAction::Deselect) => {
                            self.active_template_index = None;
                            self.resolved_template = None;
                        }
                        Some(TemplateBrowserAction::Close) => {
                            self.panels.template_browser = false;
                        }
                        None => {}
                    }
                });
        }

        // Right bookmarks panel (before inspector so it appears to its left)
        if self.panels.bookmarks {
            SidePanel::right("bookmarks")
                .default_width(250.0)
                .show(ctx, |ui| {
                    let action =
                        bookmarks::show(ui, &mut self.bookmarks, self.cursor_offset);
                    match action {
                        Some(BookmarkAction::Add) => {
                            let (offset, end) = match &self.selection {
                                Some(sel) => (sel.start, Some(sel.end)),
                                None => (self.cursor_offset, None),
                            };
                            self.bookmarks.push(Bookmark {
                                name: format!("Bookmark {}", self.bookmarks.len() + 1),
                                offset,
                                end,
                                note: String::new(),
                            });
                            self.bookmarks.sort_by_key(|b| b.offset);
                            if let Some(file) = &self.file {
                                save_bookmarks(file.path(), &self.bookmarks);
                            }
                        }
                        Some(BookmarkAction::GoToOffset(off)) => {
                            self.set_cursor_abs(off);
                        }
                        Some(BookmarkAction::Delete(idx)) => {
                            self.bookmarks.remove(idx);
                            if let Some(file) = &self.file {
                                save_bookmarks(file.path(), &self.bookmarks);
                            }
                        }
                        Some(BookmarkAction::Updated) => {
                            if let Some(file) = &self.file {
                                save_bookmarks(file.path(), &self.bookmarks);
                            }
                        }
                        Some(BookmarkAction::Close) => {
                            self.panels.bookmarks = false;
                        }
                        None => {}
                    }
                });
        }

        // Right inspector panel
        if self.panels.inspector {
            SidePanel::right("inspector")
                .default_width(260.0)
                .show(ctx, |ui| {
                    let close = if let Some(data) = self.data_bytes() {
                        inspector::show(ui, data, self.cursor_offset, &self.hex_colors)
                    } else {
                        ui.label("No file open");
                        false
                    };
                    if close {
                        self.panels.inspector = false;
                    }
                });
        }

        // Central hex view
        CentralPanel::default().show(ctx, |ui| {
            // Auto-compute columns from available width
            if self.auto_columns {
                let font = crate::theme::monospace_font();
                let char_width = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
                let available = ui.available_width();
                let chars_available = (available / char_width) as usize;
                let cols = if self.panels.ascii_pane {
                    // total_chars = 13 + 4*cols
                    chars_available.saturating_sub(13) / 4
                } else {
                    // total_chars = 11 + 3*cols
                    chars_available.saturating_sub(11) / 3
                };
                // Round down to nearest 8, clamp to [8, 128]
                let cols = (cols / 8) * 8;
                self.columns = cols.clamp(8, 128);
            }

            let data: Option<&[u8]> = if let Some(buf) = &self.edit_buffer {
                Some(buf.data())
            } else {
                self.file.as_ref().map(|f| f.as_bytes())
            };
            let data_len = data.map(|d| d.len()).unwrap_or(0);
            if let Some(data) = data {
                let action = hex_view::show(
                    ui,
                    data,
                    data_len,
                    self.columns,
                    self.cursor_offset,
                    self.selection.as_ref(),
                    &self.search.matches,
                    self.panels.ascii_pane,
                    &mut self.hex_view_state,
                    self.resolved_template.as_ref(),
                    self.nibble_high,
                    self.edit_focus,
                    &self.hex_colors,
                );
                match action {
                    Some(HexViewAction::SetCursor(off, pane)) if off < data_len => {
                        self.cursor_offset = off;
                        self.selection = None;
                        self.selection_anchor = None;
                        self.edit_focus = pane;
                        self.nibble_high = true;
                    }
                    Some(HexViewAction::Select { start, end, pane }) => {
                        let max = data_len.saturating_sub(1);
                        let s = start.min(max);
                        let e = end.min(max);
                        self.cursor_offset = e;
                        self.selection = Some(Selection::new(s, e));
                        self.selection_pane = pane;
                        self.edit_focus = pane;
                        self.nibble_high = true;
                    }
                    _ => {}
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("Drop a file or press Ctrl+O to open");
                });
            }
        });

        // Drop zone visual indicator
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let screen = ctx.input(|i| i.viewport_rect());
            let painter =
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop_overlay")));
            painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(40, 80, 160, 60));
            painter.text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                "Drop file to open",
                egui::FontId::proportional(24.0),
                Color32::WHITE,
            );
        }

        self.show_notifications(ctx);

        // Handle copy at the very end so nothing overwrites ctx.copy_text()
        if self.pending_copy {
            self.pending_copy = false;
            match self.selection_pane {
                HexPane::Hex => self.copy_selection_hex(ctx),
                HexPane::Ascii => self.copy_selection_text(ctx),
            }
        }
    }
}

fn bookmarks_sidecar_path(file_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = file_path.parent()?;
    let name = file_path.file_name()?.to_str()?;
    Some(dir.join(format!(".{name}.hexenly.json")))
}

fn load_bookmarks(file_path: &std::path::Path) -> Vec<Bookmark> {
    let Some(sidecar) = bookmarks_sidecar_path(file_path) else {
        return vec![];
    };
    let Ok(content) = std::fs::read_to_string(&sidecar) else {
        return vec![];
    };

    #[derive(serde::Deserialize)]
    struct Sidecar {
        bookmarks: Vec<Bookmark>,
    }

    serde_json::from_str::<Sidecar>(&content)
        .map(|s| s.bookmarks)
        .unwrap_or_default()
}

fn save_bookmarks(file_path: &std::path::Path, bookmarks: &[Bookmark]) {
    let Some(sidecar) = bookmarks_sidecar_path(file_path) else {
        return;
    };

    #[derive(serde::Serialize)]
    struct Sidecar<'a> {
        bookmarks: &'a [Bookmark],
    }

    let json = serde_json::to_string_pretty(&Sidecar { bookmarks }).unwrap_or_default();
    if let Err(e) = std::fs::write(&sidecar, json) {
        tracing::error!("Failed to save bookmarks: {e}");
    }
}

fn recent_files_path() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("hexenly");
    Some(dir.join("recent.json"))
}

fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recent_files_path() else {
        return vec![];
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    serde_json::from_str::<Vec<PathBuf>>(&content).unwrap_or_default()
}

fn save_recent_files(files: &[PathBuf]) {
    let Some(path) = recent_files_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let json = serde_json::to_string_pretty(files).unwrap_or_default();
    if let Err(e) = std::fs::write(&path, json) {
        tracing::error!("Failed to save recent files: {e}");
    }
}

fn format_size(bytes: usize) -> String {
    const KIB: usize = 1024;
    const MIB: usize = 1024 * KIB;
    const GIB: usize = 1024 * MIB;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}
