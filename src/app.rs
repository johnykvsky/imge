use std::path::{Path, PathBuf};
use eframe::egui::{self, Key};
use image::{DynamicImage, RgbaImage};

use crate::dialogs::{ImageInfoModal, LosslessJpegModal, QualityModal, ResizeModal};
use crate::image_engine::{ImageEngine, JpegLosslessOp, LosslessJpegConfig};
use crate::viewer::ImageViewer;

const SUPPORTED_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "gif", "tiff", "tif", "ico",
];

pub struct ImgeApp {
    pub engine: ImageEngine,
    pub viewer: ImageViewer,
    pub resize_modal: ResizeModal,
    pub quality_modal: QualityModal,
    pub info_modal: ImageInfoModal,
    pub lossless_modal: LosslessJpegModal,

    texture: Option<egui::TextureHandle>,
    dir_files: Vec<PathBuf>,
    dir_index: Option<usize>,
    status_message: String,
    needs_fit: bool,
    focus_frames: u8,
    first_frame: bool,
}

impl ImgeApp {
    pub fn new(initial_path: Option<PathBuf>, cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            engine: ImageEngine::new(),
            viewer: ImageViewer::new(),
            resize_modal: ResizeModal::default(),
            quality_modal: QualityModal::default(),
            info_modal: ImageInfoModal::default(),
            lossless_modal: LosslessJpegModal::default(),
            texture: None,
            dir_files: Vec::new(),
            dir_index: None,
            status_message: "Ready".to_string(),
            needs_fit: true,
            focus_frames: 20,
            first_frame: true,
        };

        if let Some(path) = initial_path {
            if path.exists() {
                app.load_file(&path, &cc.egui_ctx);
            }
        }

        app
    }

    pub fn load_file(&mut self, path: &Path, ctx: &egui::Context) {
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };

        match self.engine.load_file(&abs_path) {
            Ok(()) => {
                self.update_directory_list(&abs_path);
                self.update_texture(ctx);
                self.needs_fit = true;
                let index_info = match (self.dir_index, self.dir_files.len()) {
                    (Some(idx), total) if total > 1 => format!(" [{}/{}]", idx + 1, total),
                    _ => String::new(),
                };
                self.status_message = format!("Loaded {}{}", self.engine.file_name(), index_info);
            }
            Err(e) => {
                self.status_message = format!("Error: {e}");
            }
        }
    }

    fn update_directory_list(&mut self, current_path: &Path) {
        self.dir_files.clear();
        self.dir_index = None;

        let abs_path = if current_path.is_absolute() {
            current_path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(current_path))
                .unwrap_or_else(|_| current_path.to_path_buf())
        };

        if let Some(parent) = abs_path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let mut files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let is_file = e
                            .file_type()
                            .map(|ft| ft.is_file() || ft.is_symlink())
                            .unwrap_or(false);
                        if !is_file {
                            return false;
                        }
                        let p = e.path();
                        p.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| SUPPORTED_EXTS.contains(&ext.to_lowercase().as_str()))
                            .unwrap_or(false)
                    })
                    .map(|e| e.path())
                    .collect();

                files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

                // Fast O(1) in-memory comparison by file_name first
                if let Some(target_name) = abs_path.file_name() {
                    self.dir_index = files.iter().position(|p| p.file_name() == Some(target_name));
                }
                // Fallback to canonical resolution only if not matched by filename (e.g. symlinks)
                if self.dir_index.is_none() {
                    let canon_target = abs_path.canonicalize().unwrap_or_else(|_| abs_path.clone());
                    self.dir_index = files.iter().position(|p| {
                        p.canonicalize().unwrap_or_else(|_| p.clone()) == canon_target
                    });
                }
                self.dir_files = files;
            }
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(color_img) = self.engine.to_egui_image() {
            self.texture = Some(ctx.load_texture(
                "active_image",
                color_img,
                egui::TextureOptions::LINEAR,
            ));
        } else {
            self.texture = None;
        }
        self.viewer.clear_selection();
    }

    pub fn paste_from_clipboard(&mut self, ctx: &egui::Context) {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            self.status_message = "Failed to access clipboard".to_string();
            return;
        };

        // 1. Try reading direct raw image bitmap from clipboard
        if let Ok(img_data) = clipboard.get_image() {
            let w = img_data.width as u32;
            let h = img_data.height as u32;
            if let Some(rgba) = RgbaImage::from_raw(w, h, img_data.bytes.into_owned()) {
                let dyn_img = DynamicImage::ImageRgba8(rgba);
                self.engine.load_dynamic_image(dyn_img, None);
                self.dir_files.clear();
                self.dir_index = None;
                self.update_texture(ctx);
                self.needs_fit = true;
                self.status_message = "Pasted image from clipboard".to_string();
                return;
            }
        }

        // 2. Fallback: Check if clipboard text contains an image file path or file:// URL
        // (common when copying an image file from Double Commander, Dolphin, or terminal)
        if let Ok(text) = clipboard.get_text() {
            for line in text.lines() {
                let clean = line.trim();
                let path_str = if let Some(stripped) = clean.strip_prefix("file://") {
                    stripped
                } else {
                    clean
                };
                let path = PathBuf::from(path_str);
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if SUPPORTED_EXTS.contains(&ext.as_str()) {
                        self.load_file(&path, ctx);
                        self.status_message = format!("Pasted from clipboard: {}", self.engine.file_name());
                        return;
                    }
                }
            }
        }

        self.status_message = "No image or image file found in clipboard".to_string();
    }

    pub fn copy_to_clipboard(&mut self) {
        if let Some(ref img) = self.engine.current {
            let (target_rgba, status_msg) = if let Some([x, y, w, h]) = self.viewer.selection {
                let (img_w, img_h) = self.engine.dimensions();
                let clamped_w = w.min(img_w.saturating_sub(x));
                let clamped_h = h.min(img_h.saturating_sub(y));
                if clamped_w > 0 && clamped_h > 0 {
                    let cropped = img.crop_imm(x, y, clamped_w, clamped_h);
                    (
                        cropped.to_rgba8(),
                        format!("Copied selection ({} × {} px) to clipboard", clamped_w, clamped_h),
                    )
                } else {
                    (img.to_rgba8(), "Copied image to clipboard".to_string())
                }
            } else {
                (img.to_rgba8(), "Copied image to clipboard".to_string())
            };

            let (w, h) = target_rgba.dimensions();
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let img_data = arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Borrowed(target_rgba.as_raw()),
                };
                if clipboard.set_image(img_data).is_ok() {
                    self.status_message = status_msg;
                    return;
                }
            }
        }
        self.status_message = "Failed to copy image to clipboard".to_string();
    }

    pub fn save_as(&mut self) {
        if !self.engine.has_image() {
            return;
        }

        let dialog = rfd::FileDialog::new()
            .set_title("Save Image As")
            .add_filter("PNG Image (*.png)", &["png"])
            .add_filter("JPEG Image (*.jpg, *.jpeg)", &["jpg", "jpeg"])
            .add_filter("WebP Image (*.webp)", &["webp"])
            .add_filter("GIF Image (*.gif)", &["gif"])
            .add_filter("BMP Image (*.bmp)", &["bmp"])
            .add_filter("TIFF Image (*.tiff, *.tif)", &["tiff", "tif"]);

        if let Some(ref path) = self.engine.file_path {
            if let Some(parent) = path.parent() {
                let dialog = dialog.set_directory(parent);
                if let Some(selected) = dialog.save_file() {
                    self.handle_save_target(selected);
                }
                return;
            }
        }

        if let Some(selected) = dialog.save_file() {
            self.handle_save_target(selected);
        }
    }

    fn handle_save_target(&mut self, mut path: PathBuf) {
        if path.extension().is_none() {
            path.set_extension("png");
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "jpg" || ext == "jpeg" {
            self.quality_modal.open(path);
        } else {
            self.perform_save(&path, 90);
        }
    }

    pub fn save_current(&mut self) {
        if let Some(ref path) = self.engine.file_path.clone() {
            self.perform_save(path, 90);
        } else {
            self.save_as();
        }
    }

    fn perform_save(&mut self, path: &Path, quality: u8) {
        match self.engine.save(path, quality) {
            Ok(()) => {
                self.update_directory_list(path);
                self.status_message = format!("Saved {}", self.engine.file_name());
            }
            Err(e) => {
                self.status_message = format!("Save Error: {e}");
            }
        }
    }

    pub fn crop_to_selection(&mut self, ctx: &egui::Context) {
        if let Some([x, y, w, h]) = self.viewer.selection {
            if self.engine.crop(x, y, w, h) {
                self.update_texture(ctx);
                self.needs_fit = true;
                self.status_message = format!("Cropped to {w} × {h} px");
            }
        }
    }

    pub fn next_image(&mut self, ctx: &egui::Context) {
        if self.dir_files.is_empty() {
            return;
        }
        let next_idx = match self.dir_index {
            Some(i) => (i + 1) % self.dir_files.len(),
            None => 0,
        };
        let path = self.dir_files[next_idx].clone();
        self.load_file(&path, ctx);
    }

    pub fn prev_image(&mut self, ctx: &egui::Context) {
        if self.dir_files.is_empty() {
            return;
        }
        let prev_idx = match self.dir_index {
            Some(i) => (i + self.dir_files.len() - 1) % self.dir_files.len(),
            None => 0,
        };
        let path = self.dir_files[prev_idx].clone();
        self.load_file(&path, ctx);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.resize_modal.is_open || self.quality_modal.is_open {
            return;
        }

        let mut do_open = false;
        let mut do_paste = false;
        let mut do_copy = false;
        let mut do_save = false;
        let mut do_save_as = false;
        let mut do_resize = false;
        let mut do_undo = false;
        let mut do_redo = false;
        let mut do_rotate_cw = false;
        let mut do_rotate_ccw = false;
        let mut do_flip_h = false;
        let mut do_flip_v = false;
        let mut do_crop = false;
        let mut do_quit = false;
        let mut do_escape = false;
        let mut do_zoom_100 = false;
        let mut do_zoom_fit = false;
        let mut do_zoom_in = false;
        let mut do_zoom_out = false;
        let mut do_next = false;
        let mut do_prev = false;
        let mut do_info = false;
        let mut do_lossless_dialog = false;

        // 1. Collect key actions inside ctx.input without performing heavy operations
        ctx.input(|i| {
            if i.modifiers.ctrl && i.key_pressed(Key::O) {
                do_open = true;
            }

            // Paste detection:
            // egui-winit consumes Ctrl+V internally and emits Event::Paste(text), or
            // swallows it when the clipboard contains an image (calling smithay text load, which fails).
            // On key release, Event::Key with pressed=false is pushed.
            let is_paste = (i.modifiers.ctrl && i.key_pressed(Key::V))
                || (i.modifiers.shift && i.key_pressed(Key::Insert))
                || i.key_pressed(Key::Paste)
                || i.events.iter().any(|e| {
                    matches!(e, egui::Event::Paste(_))
                        || matches!(
                            e,
                            egui::Event::Key {
                                key: Key::V,
                                pressed: false,
                                modifiers,
                                ..
                            } if modifiers.ctrl || modifiers.command
                        )
                        || matches!(
                            e,
                            egui::Event::Key {
                                key: Key::Insert,
                                pressed: false,
                                modifiers,
                                ..
                            } if modifiers.shift
                        )
                });
            if is_paste {
                do_paste = true;
            }

            // Copy detection:
            // egui-winit consumes Ctrl+C internally and emits Event::Copy instead of standard Key::C pressed.
            let is_copy = (i.modifiers.ctrl && i.key_pressed(Key::C))
                || (i.modifiers.ctrl && i.key_pressed(Key::Insert))
                || i.key_pressed(Key::Copy)
                || i.events.iter().any(|e| {
                    matches!(e, egui::Event::Copy)
                        || matches!(
                            e,
                            egui::Event::Key {
                                key: Key::C,
                                pressed: false,
                                modifiers,
                                ..
                            } if modifiers.ctrl || modifiers.command
                        )
                });
            if is_copy {
                do_copy = true;
            }

            if i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::S) {
                do_save = true;
            }
            if i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::S) {
                do_save_as = true;
            }
            if i.modifiers.ctrl && i.key_pressed(Key::R) {
                do_resize = true;
            }
            if i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::Z) {
                do_undo = true;
            }
            if (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::Z))
                || (i.modifiers.ctrl && i.key_pressed(Key::Y))
            {
                do_redo = true;
            }
            if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::R) {
                do_rotate_cw = true;
            }
            if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::L) {
                do_rotate_ccw = true;
            }
            if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::H) {
                do_flip_h = true;
            }
            if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::V) {
                do_flip_v = true;
            }
            if i.key_pressed(Key::Enter) {
                do_crop = true;
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Q) {
                do_quit = true;
            }
            if i.key_pressed(Key::Escape) {
                do_escape = true;
            }
            if i.key_pressed(Key::Num1) || (i.modifiers.ctrl && i.key_pressed(Key::Num0)) {
                do_zoom_100 = true;
            }
            if !i.modifiers.ctrl && i.key_pressed(Key::F) {
                do_zoom_fit = true;
            }
            if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                do_zoom_in = true;
            }
            if i.key_pressed(Key::Minus) {
                do_zoom_out = true;
            }
            if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space) {
                do_next = true;
            }
            if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::Backspace) {
                do_prev = true;
            }
            if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::I) {
                do_info = true;
            }
            if i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::J) {
                do_lossless_dialog = true;
            }
        });

        // If a modal is open, let Escape close it and prevent background shortcut execution
        let modal_open = self.resize_modal.is_open
            || self.quality_modal.is_open
            || self.info_modal.is_open
            || self.lossless_modal.is_open;
        if modal_open {
            if do_escape {
                self.resize_modal.is_open = false;
                self.quality_modal.is_open = false;
                self.info_modal.is_open = false;
                self.lossless_modal.is_open = false;
            }
            return;
        }

        // 2. Safely execute actions OUTSIDE ctx.input lock
        if do_quit {
            std::process::exit(0);
        }
        if do_escape {
            if self.viewer.selection.is_some() {
                self.viewer.clear_selection();
            } else {
                std::process::exit(0);
            }
        }
        if do_open {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("Images", SUPPORTED_EXTS)
                .pick_file()
            {
                self.load_file(&file, ctx);
            }
        }
        if do_paste {
            self.paste_from_clipboard(ctx);
        }
        if do_copy {
            self.copy_to_clipboard();
        }
        if do_save {
            self.save_current();
        }
        if do_save_as {
            self.save_as();
        }
        if do_resize {
            if self.engine.has_image() {
                let (w, h) = self.engine.dimensions();
                self.resize_modal.open(w, h);
            }
        }
        if do_undo {
            if self.engine.undo() {
                self.update_texture(ctx);
                self.status_message = "Undo".to_string();
            }
        }
        if do_redo {
            if self.engine.redo() {
                self.update_texture(ctx);
                self.status_message = "Redo".to_string();
            }
        }
        if do_rotate_cw {
            if self.engine.rotate_cw() {
                self.update_texture(ctx);
                self.status_message = "Rotated 90° CW".to_string();
            }
        }
        if do_rotate_ccw {
            if self.engine.rotate_ccw() {
                self.update_texture(ctx);
                self.status_message = "Rotated 90° CCW".to_string();
            }
        }
        if do_flip_h {
            if self.engine.flip_h() {
                self.update_texture(ctx);
                self.status_message = "Flipped horizontally".to_string();
            }
        }
        if do_flip_v {
            if self.engine.flip_v() {
                self.update_texture(ctx);
                self.status_message = "Flipped vertically".to_string();
            }
        }
        if do_crop {
            self.crop_to_selection(ctx);
        }
        if do_zoom_100 {
            self.viewer.set_zoom_100();
        }
        if do_zoom_fit {
            self.needs_fit = true;
        }
        if do_zoom_in {
            self.viewer.zoom_in();
        }
        if do_zoom_out {
            self.viewer.zoom_out();
        }
        if do_next {
            self.next_image(ctx);
        }
        if do_prev {
            self.prev_image(ctx);
        }
        if do_info {
            let meta = self.engine.get_metadata();
            self.info_modal.open(meta);
        }
        if do_lossless_dialog {
            self.lossless_modal.open(
                self.engine.is_jpeg_file(),
                self.engine.is_modified,
                self.engine.file_path.clone(),
            );
        }
    }

    fn apply_direct_lossless_op(&mut self, op: JpegLosslessOp, ctx: &egui::Context) {
        if !self.engine.is_jpeg_file() {
            self.status_message = "JPG Lossless operations require a JPEG file.".to_string();
            return;
        }
        if self.engine.is_modified {
            self.lossless_modal.open(
                true,
                true,
                self.engine.file_path.clone(),
            );
            self.lossless_modal.op = op;
            return;
        }
        let config = LosslessJpegConfig {
            op,
            create_backup: false,
            trim_partial_mcu: false,
            reset_exif_orientation: true,
            destination_path: None,
        };
        let op_desc = op.description();
        match self.engine.apply_lossless_jpeg_transform(&config) {
            Ok(target_path) => {
                self.load_file(&target_path, ctx);
                self.status_message = format!("Losslessly transformed ({op_desc})");
            }
            Err(e) => {
                self.status_message = format!("Lossless transform error: {e}");
            }
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(ref path) = file.path {
                if path.is_file() {
                    self.load_file(path, ctx);
                    break;
                }
            }
        }
    }
}

impl eframe::App for ImgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            std::process::exit(0);
        }

        if self.focus_frames > 0 {
            let is_focused = ctx.input(|i| i.viewport().focused == Some(true));
            if is_focused {
                self.focus_frames = 0;
            } else {
                self.focus_frames -= 1;
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        self.handle_shortcuts(ctx);
        self.handle_drag_and_drop(ctx);

        if self.viewer.crop_requested {
            self.viewer.crop_requested = false;
            self.crop_to_selection(ctx);
        }

        // Apply modals
        self.resize_modal.show(ctx);
        if let Some((w, h, filter)) = self.resize_modal.should_apply.take() {
            if self.engine.resize(w, h, filter) {
                self.update_texture(ctx);
                self.needs_fit = true;
                self.status_message = format!("Resized to {w} × {h} px");
            }
        }

        self.quality_modal.show(ctx);
        if let Some((path, quality)) = self.quality_modal.should_apply.take() {
            self.perform_save(&path, quality);
        }

        self.info_modal.show(ctx);

        self.lossless_modal.show(ctx);
        if let Some(config) = self.lossless_modal.should_apply.take() {
            let op_desc = config.op.description();
            match self.engine.apply_lossless_jpeg_transform(&config) {
                Ok(target_path) => {
                    self.load_file(&target_path, ctx);
                    self.status_message = format!("Losslessly transformed ({op_desc})");
                }
                Err(e) => {
                    self.status_message = format!("Lossless transform error: {e}");
                }
            }
        }

        // Top Menu & Toolbar Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open... (Ctrl+O)").clicked() {
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("Images", SUPPORTED_EXTS)
                            .pick_file()
                        {
                            self.load_file(&file, ctx);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Paste (Ctrl+V)").clicked() {
                        self.paste_from_clipboard(ctx);
                        ui.close_menu();
                    }
                    let copy_label = if self.viewer.selection.is_some() {
                        "Copy Selection (Ctrl+C)"
                    } else {
                        "Copy (Ctrl+C)"
                    };
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new(copy_label)).clicked() {
                        self.copy_to_clipboard();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Save (Ctrl+S)")).clicked() {
                        self.save_current();
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Save As... (Ctrl+Shift+S)")).clicked() {
                        self.save_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(!self.dir_files.is_empty(), egui::Button::new("Previous Image (Left)")).clicked() {
                        self.prev_image(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(!self.dir_files.is_empty(), egui::Button::new("Next Image (Right)")).clicked() {
                        self.next_image(ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit (Esc / Ctrl+Q)").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(self.engine.can_undo(), egui::Button::new("Undo (Ctrl+Z)")).clicked() {
                        if self.engine.undo() {
                            self.update_texture(ctx);
                            self.status_message = "Undo".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.engine.can_redo(), egui::Button::new("Redo (Ctrl+Y)")).clicked() {
                        if self.engine.redo() {
                            self.update_texture(ctx);
                            self.status_message = "Redo".to_string();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let edit_copy_label = if self.viewer.selection.is_some() {
                        "Copy Selection (Ctrl+C)"
                    } else {
                        "Copy (Ctrl+C)"
                    };
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new(edit_copy_label)).clicked() {
                        self.copy_to_clipboard();
                        ui.close_menu();
                    }
                    if ui.button("Paste (Ctrl+V)").clicked() {
                        self.paste_from_clipboard(ctx);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(self.viewer.selection.is_some(), egui::Button::new("Crop to Selection (Enter)")).clicked() {
                        self.crop_to_selection(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.viewer.selection.is_some(), egui::Button::new("Clear Selection (Esc)")).clicked() {
                        self.viewer.clear_selection();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Image", |ui| {
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Image Information... (I)")).clicked() {
                        let meta = self.engine.get_metadata();
                        self.info_modal.open(meta);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Resize... (Ctrl+R)")).clicked() {
                        let (w, h) = self.engine.dimensions();
                        self.resize_modal.open(w, h);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Rotate 90° CW (R)")).clicked() {
                        if self.engine.rotate_cw() {
                            self.update_texture(ctx);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Rotate 90° CCW (L)")).clicked() {
                        if self.engine.rotate_ccw() {
                            self.update_texture(ctx);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Flip Horizontal (H)")).clicked() {
                        if self.engine.flip_h() {
                            self.update_texture(ctx);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.engine.has_image(), egui::Button::new("Flip Vertical (V)")).clicked() {
                        if self.engine.flip_v() {
                            self.update_texture(ctx);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("JPG Lossless Operations", |ui| {
                        let is_jpeg = self.engine.is_jpeg_file();
                        if ui.add_enabled(is_jpeg, egui::Button::new("⟳ Rotate 90° CW (Lossless)")).clicked() {
                            self.apply_direct_lossless_op(JpegLosslessOp::Rot90, ctx);
                            ui.close_menu();
                        }
                        if ui.add_enabled(is_jpeg, egui::Button::new("⟲ Rotate 90° CCW (Lossless)")).clicked() {
                            self.apply_direct_lossless_op(JpegLosslessOp::Rot270, ctx);
                            ui.close_menu();
                        }
                        if ui.add_enabled(is_jpeg, egui::Button::new("↷ Rotate 180° (Lossless)")).clicked() {
                            self.apply_direct_lossless_op(JpegLosslessOp::Rot180, ctx);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.add_enabled(is_jpeg, egui::Button::new("⇄ Flip Horizontal (Lossless)")).clicked() {
                            self.apply_direct_lossless_op(JpegLosslessOp::Hflip, ctx);
                            ui.close_menu();
                        }
                        if ui.add_enabled(is_jpeg, egui::Button::new("⇅ Flip Vertical (Lossless)")).clicked() {
                            self.apply_direct_lossless_op(JpegLosslessOp::Vflip, ctx);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Lossless Options Dialog... (Shift+J)").clicked() {
                            self.lossless_modal.open(
                                self.engine.is_jpeg_file(),
                                self.engine.is_modified,
                                self.engine.file_path.clone(),
                            );
                            ui.close_menu();
                        }
                    });
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Zoom In (+)").clicked() {
                        self.viewer.zoom_in();
                        ui.close_menu();
                    }
                    if ui.button("Zoom Out (-)").clicked() {
                        self.viewer.zoom_out();
                        ui.close_menu();
                    }
                    if ui.button("100% Actual Size (1)").clicked() {
                        self.viewer.set_zoom_100();
                        ui.close_menu();
                    }
                    if ui.button("Fit to Window (F)").clicked() {
                        self.needs_fit = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    ui.label(egui::RichText::new(format!("imge v{}", env!("CARGO_PKG_VERSION"))).strong());
                    ui.label("A fast, lightweight image manipulation and viewing desktop app inspired by IrfanView.");
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak());
                });
            });

            // Quick Toolbar
            ui.horizontal(|ui| {
                if ui.button("📂 Open").clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("Images", SUPPORTED_EXTS)
                        .pick_file()
                    {
                        self.load_file(&file, ctx);
                    }
                }
                if ui.button("📋 Paste").clicked() {
                    self.paste_from_clipboard(ctx);
                }
                if ui.add_enabled(self.engine.has_image(), egui::Button::new("💾 Save")).clicked() {
                    self.save_current();
                }

                ui.separator();
                if ui.add_enabled(self.engine.can_undo(), egui::Button::new("↩ Undo")).clicked() {
                    if self.engine.undo() {
                        self.update_texture(ctx);
                    }
                }
                if ui.add_enabled(self.engine.can_redo(), egui::Button::new("↪ Redo")).clicked() {
                    if self.engine.redo() {
                        self.update_texture(ctx);
                    }
                }

                ui.separator();
                if ui.add_enabled(self.engine.has_image(), egui::Button::new("📐 Resize")).clicked() {
                    let (w, h) = self.engine.dimensions();
                    self.resize_modal.open(w, h);
                }
                if ui.add_enabled(self.viewer.selection.is_some(), egui::Button::new("✂ Crop")).clicked() {
                    self.crop_to_selection(ctx);
                }
                if ui.add_enabled(self.engine.has_image(), egui::Button::new("⟳ Rotate")).clicked() {
                    if self.engine.rotate_cw() {
                        self.update_texture(ctx);
                    }
                }
                if ui.add_enabled(self.engine.has_image(), egui::Button::new("⇄ Flip")).clicked() {
                    if self.engine.flip_h() {
                        self.update_texture(ctx);
                    }
                }

                ui.separator();
                if ui.button("1:1").clicked() {
                    self.viewer.set_zoom_100();
                }
                if ui.button("Fit").clicked() {
                    self.needs_fit = true;
                }
                ui.separator();
                if ui.add_enabled(self.engine.has_image(), egui::Button::new("ℹ Info")).clicked() {
                    let meta = self.engine.get_metadata();
                    self.info_modal.open(meta);
                }
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(bytes) = self.engine.file_size_bytes() {
                        let size_str = if bytes < 1024 {
                            format!("{bytes} B")
                        } else if bytes < 1024 * 1024 {
                            format!("{:.1} KB", bytes as f64 / 1024.0)
                        } else {
                            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
                        };
                        ui.label(size_str);
                        ui.separator();
                    }

                    let zoom_pct = (self.viewer.zoom * 100.0).round() as i32;
                    ui.label(format!("{zoom_pct}%"));
                    ui.separator();

                    let (w, h) = self.engine.dimensions();
                    if w > 0 && h > 0 {
                        ui.label(format!("{w} × {h} px"));
                        ui.separator();
                    }

                    if let Some([_, _, sw, sh]) = self.viewer.selection {
                        ui.label(format!("Selection: {sw} × {sh} px"));
                        ui.separator();
                    }
                });
            });
        });

        // Central Image Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            let (w, h) = self.engine.dimensions();
            let tex_id = self.texture.as_ref().map(|t| t.id());

            if self.needs_fit && w > 0 && h > 0 {
                self.viewer.fit_to_viewport(ui.available_rect_before_wrap(), w, h);
                self.needs_fit = false;
            }

            self.viewer.ui(ui, tex_id, w, h);
        });

        // Update window title
        let mod_prefix = if self.engine.is_modified { "* " } else { "" };
        let (w, h) = self.engine.dimensions();
        let dir_info = match (self.dir_index, self.dir_files.len()) {
            (Some(idx), total) if total > 1 => format!(" [{}/{}]", idx + 1, total),
            _ => String::new(),
        };
        let title = if self.engine.has_image() {
            format!("{mod_prefix}{}{dir_info} ({w}×{h}) - imge v{}", self.engine.file_name(), env!("CARGO_PKG_VERSION"))
        } else {
            format!("imge v{}", env!("CARGO_PKG_VERSION"))
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        if self.first_frame {
            self.first_frame = false;
            if std::env::var("IMGE_PROFILE").is_ok() {
                eprintln!("[profile] First frame update() complete");
            }
        }
    }
}
