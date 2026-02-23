use eframe::App;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};
use hexenly_core::{HexFile, SearchPattern, Selection, find_all};
use hexenly_templates::engine::{self, ResolveResult};
use hexenly_templates::loader::TemplateRegistry;
use hexenly_templates::resolved::ResolvedTemplate;

use crate::panels::hex_view::{self, HexViewAction, HexViewState};
use crate::panels::inspector;
use crate::panels::structure::{self, StructureAction};
use crate::panels::templates::{self, TemplateBrowserAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Ascii,
    Utf8,
}

pub struct HexenlyApp {
    file: Option<HexFile>,
    cursor_offset: usize,
    selection: Option<Selection>,
    columns: usize,
    show_inspector: bool,
    show_ascii_pane: bool,
    text_encoding: TextEncoding,

    // Go-to-offset dialog
    show_goto: bool,
    goto_input: String,

    // Search state
    show_search: bool,
    focus_search: bool,
    search_input: String,
    search_hex_mode: bool,
    search_matches: Vec<usize>,
    search_match_idx: Option<usize>,

    hex_view_state: HexViewState,
    theme_applied: bool,

    // File to open on first frame (from CLI args)
    pending_open: Option<String>,

    // Template state
    show_template_browser: bool,
    show_structure_panel: bool,
    template_registry: TemplateRegistry,
    active_template_index: Option<usize>,
    resolved_template: Option<ResolvedTemplate>,
    template_filter: String,
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

        for (name, err) in &registry.load_errors {
            tracing::error!("Failed to load template {name}: {err}");
        }

        Self {
            file: None,
            cursor_offset: 0,
            selection: None,
            columns: 16,
            show_inspector: true,
            show_ascii_pane: true,
            text_encoding: TextEncoding::Ascii,
            show_goto: false,
            goto_input: String::new(),
            show_search: false,
            focus_search: false,
            search_input: String::new(),
            search_hex_mode: false,
            search_matches: Vec::new(),
            search_match_idx: None,
            hex_view_state: HexViewState::default(),
            theme_applied: false,
            pending_open: path,
            show_template_browser: false,
            show_structure_panel: false,
            template_registry: registry,
            active_template_index: None,
            resolved_template: None,
            template_filter: String::new(),
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
                self.cursor_offset = 0;
                self.selection = None;
                self.search_matches.clear();
                self.search_match_idx = None;
                self.active_template_index = None;
                self.resolved_template = None;

                // Auto-detect template
                self.auto_detect_template(path);
            }
            Err(e) => {
                tracing::error!("Failed to open file: {e}");
            }
        }
    }

    fn auto_detect_template(&mut self, path: &std::path::Path) {
        let Some(file) = &self.file else { return };
        let bytes = file.as_bytes();

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
                self.show_structure_panel = true;
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
                    self.show_structure_panel = true;
                }
            }
        }
    }

    fn resolve_active_template(&mut self) {
        let Some(file) = &self.file else {
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

        let result: ResolveResult = engine::resolve(&entry.template, file.as_bytes());

        for warning in &result.warnings {
            tracing::warn!("Template resolve: {warning}");
        }

        self.resolved_template = Some(result.template);
    }

    fn do_search(&mut self) {
        let Some(file) = &self.file else { return };
        let pattern = if self.search_hex_mode {
            match SearchPattern::from_hex_string(&self.search_input) {
                Some(p) => p,
                None => return,
            }
        } else {
            SearchPattern::from_text(&self.search_input)
        };

        self.search_matches = find_all(file.as_bytes(), &pattern, 10_000);
        if let Some(&first) = self.search_matches.first() {
            self.search_match_idx = Some(0);
            self.cursor_offset = first;
            self.scroll_to_cursor();
        } else {
            self.search_match_idx = None;
        }
    }

    fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let idx = self
            .search_match_idx
            .map(|i| (i + 1) % self.search_matches.len())
            .unwrap_or(0);
        self.search_match_idx = Some(idx);
        self.cursor_offset = self.search_matches[idx];
        self.scroll_to_cursor();
    }

    fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let len = self.search_matches.len();
        let idx = self
            .search_match_idx
            .map(|i| (i + len - 1) % len)
            .unwrap_or(len - 1);
        self.search_match_idx = Some(idx);
        self.cursor_offset = self.search_matches[idx];
        self.scroll_to_cursor();
    }

    fn goto_offset(&mut self) {
        let text = self.goto_input.trim();
        let offset = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).ok()
        } else {
            text.parse::<usize>().ok()
        };
        if let Some(off) = offset {
            if let Some(file) = &self.file {
                if off < file.len() {
                    self.cursor_offset = off;
                    self.selection = None;
                    self.scroll_to_cursor();
                }
            }
        }
        self.show_goto = false;
        self.goto_input.clear();
    }

    fn scroll_to_cursor(&mut self) {
        let row = self.cursor_offset / self.columns;
        self.hex_view_state.scroll_to_row = Some(row);
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        ctx.input(|i| {
            if i.key_pressed(Key::O) && i.modifiers.command {
                self.open_file_dialog();
            }
            if i.key_pressed(Key::G) && i.modifiers.command {
                self.show_goto = !self.show_goto;
                self.show_search = false;
            }
            if i.key_pressed(Key::F) && i.modifiers.command {
                self.show_search = !self.show_search;
                self.focus_search = self.show_search;
                self.show_goto = false;
            }
            if i.key_pressed(Key::Escape) {
                self.show_goto = false;
                self.show_search = false;
            }
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open").clicked() {
                self.open_file_dialog();
            }

            ui.separator();

            ui.label("Columns:");
            for &n in &[8, 16, 24, 32, 48] {
                if ui
                    .selectable_label(self.columns == n, format!("{n}"))
                    .clicked()
                {
                    self.columns = n;
                }
            }

            ui.separator();

            if ui
                .selectable_label(self.text_encoding == TextEncoding::Ascii, "ASCII")
                .clicked()
            {
                self.text_encoding = TextEncoding::Ascii;
            }
            if ui
                .selectable_label(self.text_encoding == TextEncoding::Utf8, "UTF-8")
                .clicked()
            {
                self.text_encoding = TextEncoding::Utf8;
            }

            ui.separator();

            ui.toggle_value(&mut self.show_ascii_pane, "ASCII Pane");
            ui.toggle_value(&mut self.show_inspector, "Inspector");
            ui.toggle_value(&mut self.show_template_browser, "Templates");
            ui.toggle_value(&mut self.show_structure_panel, "Structure");
        });
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Search:");
            let re = ui.text_edit_singleline(&mut self.search_input);
            if self.focus_search {
                re.request_focus();
                self.focus_search = false;
            }
            if re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.do_search();
            }
            ui.toggle_value(&mut self.search_hex_mode, "Hex");
            if ui.button("Find").clicked() {
                self.do_search();
            }
            if ui.button("Prev").clicked() {
                self.search_prev();
            }
            if ui.button("Next").clicked() {
                self.search_next();
            }
            if let Some(idx) = self.search_match_idx {
                ui.label(format!(
                    "{}/{}",
                    idx + 1,
                    self.search_matches.len()
                ));
            }
        });
    }

    fn show_goto_bar(&mut self, ui: &mut egui::Ui) {
        if !self.show_goto {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Go to offset:");
            let re = ui.text_edit_singleline(&mut self.goto_input);
            if re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.goto_offset();
            }
            if ui.button("Go").clicked() {
                self.goto_offset();
            }
            ui.label("(prefix 0x for hex)");
        });
    }

    fn show_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(file) = &self.file {
                let name = file
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                ui.label(&name);
                ui.separator();
                ui.label(format_size(file.len()));
                ui.separator();
                ui.label(format!(
                    "Offset: 0x{:08X} ({})",
                    self.cursor_offset, self.cursor_offset
                ));
                if let Some(sel) = &self.selection {
                    ui.separator();
                    ui.label(format!("Selected: {} bytes", sel.len()));
                }
                if let Some(resolved) = &self.resolved_template {
                    ui.separator();
                    ui.label(format!("Template: {}", resolved.name));
                }
            } else {
                ui.label("No file open \u{2014} Ctrl+O to open");
            }
        });
    }
}

impl App for HexenlyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            crate::theme::apply_theme(ctx);
            self.theme_applied = true;
        }

        // Handle pending file open from CLI
        if let Some(path) = self.pending_open.take() {
            self.open_path(std::path::Path::new(&path));
        }

        self.handle_shortcuts(ctx);

        // Top toolbar
        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.show_toolbar(ui);
            self.show_search_bar(ui);
            self.show_goto_bar(ui);
        });

        // Bottom status bar
        TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            self.show_status_bar(ui);
        });

        // Structure panel (above status bar)
        if self.show_structure_panel {
            if let Some(resolved) = &self.resolved_template {
                let resolved_clone = resolved.clone();
                let cursor = self.cursor_offset;
                TopBottomPanel::bottom("structure")
                    .default_height(200.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        let action = structure::show(ui, &resolved_clone, cursor);
                        if let Some(StructureAction::GoToOffset(off)) = action {
                            self.cursor_offset = off;
                            self.scroll_to_cursor();
                        }
                    });
            }
        }

        // Left template browser panel
        if self.show_template_browser {
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
                            self.show_structure_panel = true;
                        }
                        Some(TemplateBrowserAction::Deselect) => {
                            self.active_template_index = None;
                            self.resolved_template = None;
                        }
                        None => {}
                    }
                });
        }

        // Right inspector panel
        if self.show_inspector {
            SidePanel::right("inspector")
                .default_width(220.0)
                .show(ctx, |ui| {
                    if let Some(file) = &self.file {
                        inspector::show(ui, file, self.cursor_offset);
                    } else {
                        ui.label("No file open");
                    }
                });
        }

        // Central hex view
        CentralPanel::default().show(ctx, |ui| {
            if let Some(file) = &self.file {
                let action = hex_view::show(
                    ui,
                    file,
                    self.columns,
                    self.cursor_offset,
                    self.selection.as_ref(),
                    &self.search_matches,
                    self.show_ascii_pane,
                    &mut self.hex_view_state,
                    self.resolved_template.as_ref(),
                );
                if let Some(HexViewAction::SetCursor(off)) = action {
                    if off < file.len() {
                        self.cursor_offset = off;
                    }
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("Drop a file or press Ctrl+O to open");
                });
            }
        });
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
