use std::path::PathBuf;
use crate::image_engine::{JpegLosslessOp, LosslessJpegConfig, ResizeFilter};
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    Pixels,
    Percentage,
}

pub struct ResizeModal {
    pub is_open: bool,
    pub mode: ResizeMode,
    pub width: u32,
    pub height: u32,
    pub percentage: f64,
    pub lock_aspect: bool,
    pub orig_width: u32,
    pub orig_height: u32,
    pub filter: ResizeFilter,
    pub should_apply: Option<(u32, u32, ResizeFilter)>,
}

impl Default for ResizeModal {
    fn default() -> Self {
        Self {
            is_open: false,
            mode: ResizeMode::Pixels,
            width: 100,
            height: 100,
            percentage: 100.0,
            lock_aspect: true,
            orig_width: 100,
            orig_height: 100,
            filter: ResizeFilter::Lanczos3,
            should_apply: None,
        }
    }
}

impl ResizeModal {
    pub fn open(&mut self, orig_w: u32, orig_h: u32) {
        self.is_open = true;
        self.orig_width = orig_w;
        self.orig_height = orig_h;
        self.width = orig_w;
        self.height = orig_h;
        self.percentage = 100.0;
        self.lock_aspect = true;
        self.mode = ResizeMode::Pixels;
        self.should_apply = None;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        let mut close_requested = false;

        // Escape cancels, Enter confirms
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_requested = true;
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.should_apply = Some((self.width, self.height, self.filter));
            close_requested = true;
        }

        egui::Window::new("Resize Image")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.mode, ResizeMode::Pixels, "Pixels");
                    ui.selectable_value(&mut self.mode, ResizeMode::Percentage, "Percentage");
                });

                ui.separator();
                ui.label(format!("Original: {} × {} px", self.orig_width, self.orig_height));

                let ratio = if self.orig_height > 0 {
                    self.orig_width as f64 / self.orig_height as f64
                } else {
                    1.0
                };

                match self.mode {
                    ResizeMode::Pixels => {
                        let mut w = self.width;
                        let mut h = self.height;

                        ui.horizontal(|ui| {
                            ui.label("Width:");
                            if ui.add(egui::DragValue::new(&mut w).range(1..=100000).suffix(" px")).changed() {
                                self.width = w;
                                if self.lock_aspect && self.orig_width > 0 {
                                    self.height = (w as f64 / ratio).round().max(1.0) as u32;
                                    self.percentage = (w as f64 / self.orig_width as f64) * 100.0;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Height:");
                            if ui.add(egui::DragValue::new(&mut h).range(1..=100000).suffix(" px")).changed() {
                                self.height = h;
                                if self.lock_aspect && self.orig_height > 0 {
                                    self.width = (h as f64 * ratio).round().max(1.0) as u32;
                                    self.percentage = (h as f64 / self.orig_height as f64) * 100.0;
                                }
                            }
                        });
                    }
                    ResizeMode::Percentage => {
                        let mut p = self.percentage;
                        ui.horizontal(|ui| {
                            ui.label("Scale:");
                            if ui.add(egui::DragValue::new(&mut p).range(1.0..=1000.0).speed(1.0).suffix(" %")).changed() {
                                self.percentage = p;
                                let factor = p / 100.0;
                                self.width = (self.orig_width as f64 * factor).round().max(1.0) as u32;
                                self.height = (self.orig_height as f64 * factor).round().max(1.0) as u32;
                            }
                        });
                    }
                }

                ui.checkbox(&mut self.lock_aspect, "Maintain aspect ratio");

                ui.horizontal(|ui| {
                    ui.label("Presets:");
                    for pct in [25, 50, 75, 150, 200] {
                        if ui.button(format!("{pct}%")).clicked() {
                            self.percentage = pct as f64;
                            let factor = pct as f64 / 100.0;
                            self.width = (self.orig_width as f64 * factor).round().max(1.0) as u32;
                            self.height = (self.orig_height as f64 * factor).round().max(1.0) as u32;
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    egui::ComboBox::from_id_salt("resample_filter")
                        .selected_text(match self.filter {
                            ResizeFilter::Lanczos3 => "Lanczos3 (High Quality)",
                            ResizeFilter::CatmullRom => "Catmull-Rom (Smooth)",
                            ResizeFilter::Triangle => "Triangle (Bilinear)",
                            ResizeFilter::Nearest => "Nearest Neighbor (Pixel Art)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.filter, ResizeFilter::Lanczos3, "Lanczos3 (High Quality)");
                            ui.selectable_value(&mut self.filter, ResizeFilter::CatmullRom, "Catmull-Rom (Smooth)");
                            ui.selectable_value(&mut self.filter, ResizeFilter::Triangle, "Triangle (Bilinear)");
                            ui.selectable_value(&mut self.filter, ResizeFilter::Nearest, "Nearest Neighbor (Pixel Art)");
                        });
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        self.should_apply = Some((self.width, self.height, self.filter));
                        close_requested = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });

        if !is_open || close_requested {
            self.is_open = false;
        }
    }
}

pub struct QualityModal {
    pub is_open: bool,
    pub quality: u8,
    pub path: Option<std::path::PathBuf>,
    pub should_apply: Option<(std::path::PathBuf, u8)>,
}

impl Default for QualityModal {
    fn default() -> Self {
        Self {
            is_open: false,
            quality: 90,
            path: None,
            should_apply: None,
        }
    }
}

impl QualityModal {
    pub fn open(&mut self, path: std::path::PathBuf) {
        self.path = Some(path);
        self.quality = 90;
        self.is_open = true;
        self.should_apply = None;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        let mut close_requested = false;

        // Escape cancels, Enter saves
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_requested = true;
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(ref p) = self.path {
                self.should_apply = Some((p.clone(), self.quality));
            }
            close_requested = true;
        }

        egui::Window::new("Export Options")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Select JPEG compression quality:");
                ui.add(egui::Slider::new(&mut self.quality, 1..=100).suffix("%"));
                ui.label("Higher quality produces larger file sizes.");

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if let Some(ref p) = self.path {
                            self.should_apply = Some((p.clone(), self.quality));
                        }
                        close_requested = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });

        if !is_open || close_requested {
            self.is_open = false;
        }
    }
}

#[derive(Default)]
pub struct ImageInfoModal {
    pub is_open: bool,
    pub metadata: Option<crate::image_engine::ImageMetadata>,
}

impl ImageInfoModal {
    pub fn open(&mut self, meta: Option<crate::image_engine::ImageMetadata>) {
        self.metadata = meta;
        self.is_open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        let mut close_requested = false;

        if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) {
            close_requested = true;
        }

        egui::Window::new("Image Information")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .default_height(380.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(ref meta) = self.metadata {
                    ui.strong("General Properties");
                    egui::Grid::new("general_info_grid")
                        .striped(true)
                        .num_columns(2)
                        .spacing([20.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("File Name:");
                            ui.label(&meta.file_name);
                            ui.end_row();

                            if let Some(ref path) = meta.file_path {
                                ui.label("File Path:");
                                ui.label(path);
                                ui.end_row();
                            }

                            if let Some(bytes) = meta.file_size_bytes {
                                ui.label("File Size on Disk:");
                                let size_str = if bytes < 1024 {
                                    format!("{bytes} B")
                                } else if bytes < 1024 * 1024 {
                                    format!("{:.1} KB ({bytes} bytes)", bytes as f64 / 1024.0)
                                } else {
                                    format!("{:.2} MB ({bytes} bytes)", bytes as f64 / (1024.0 * 1024.0))
                                };
                                ui.label(size_str);
                                ui.end_row();
                            }

                            ui.label("Dimensions:");
                            ui.label(format!("{} × {} pixels", meta.width, meta.height));
                            ui.end_row();

                            ui.label("Aspect Ratio:");
                            ui.label(&meta.aspect_ratio_str);
                            ui.end_row();

                            ui.label("Color Type:");
                            ui.label(&meta.color_type);
                            ui.end_row();

                            ui.label("Memory Footprint:");
                            let mem_mb = meta.memory_footprint_bytes as f64 / (1024.0 * 1024.0);
                            ui.label(format!("{mem_mb:.2} MB uncompressed"));
                            ui.end_row();
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.strong(format!("EXIF Metadata ({})", meta.exif_tags.len()));

                    if meta.exif_tags.is_empty() {
                        ui.label("No EXIF metadata found in this image.");
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                egui::Grid::new("exif_info_grid")
                                    .striped(true)
                                    .num_columns(2)
                                    .spacing([15.0, 4.0])
                                    .show(ui, |ui| {
                                        for (tag, val) in &meta.exif_tags {
                                            ui.label(tag);
                                            ui.label(val);
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                } else {
                    ui.label("No image loaded.");
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        close_requested = true;
                    }
                });
            });

        if !is_open || close_requested {
            self.is_open = false;
        }
    }
}

pub struct LosslessJpegModal {
    pub is_open: bool,
    pub is_jpeg: bool,
    pub is_modified: bool,
    pub file_path: Option<PathBuf>,
    pub op: JpegLosslessOp,
    pub create_backup: bool,
    pub trim_partial_mcu: bool,
    pub reset_exif_orientation: bool,
    pub save_to_new_file: bool,
    pub custom_output_path: Option<PathBuf>,
    pub should_apply: Option<LosslessJpegConfig>,
}

impl Default for LosslessJpegModal {
    fn default() -> Self {
        Self {
            is_open: false,
            is_jpeg: false,
            is_modified: false,
            file_path: None,
            op: JpegLosslessOp::Rot90,
            create_backup: false,
            trim_partial_mcu: false,
            reset_exif_orientation: true,
            save_to_new_file: false,
            custom_output_path: None,
            should_apply: None,
        }
    }
}

impl LosslessJpegModal {
    pub fn open(&mut self, is_jpeg: bool, is_modified: bool, file_path: Option<PathBuf>) {
        self.is_open = true;
        self.is_jpeg = is_jpeg;
        self.is_modified = is_modified;
        self.file_path = file_path;
        self.should_apply = None;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        let mut close_requested = false;

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_requested = true;
        }

        egui::Window::new("JPG Lossless Operations (Shift+J)")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if !self.is_jpeg {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 180, 80),
                        "JPG Lossless operations can only be performed on JPEG (.jpg / .jpeg) files.",
                    );
                    ui.add_space(6.0);
                    ui.label("Save current image as JPEG to enable lossless DCT transformations.");
                    ui.add_space(12.0);
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                    return;
                }

                if self.is_modified {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 200, 80),
                        "⚠️ Current image has unsaved edits in memory.\nLossless transform operates directly on the disk file bitstream.",
                    );
                    ui.add_space(6.0);
                }

                ui.strong("Transformation:");
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Rot90, "⟳ Rotate 90° CW");
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Rot270, "⟲ Rotate 90° CCW");
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Rot180, "↷ Rotate 180°");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Hflip, "⇄ Flip Horizontal");
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Vflip, "⇅ Flip Vertical");
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Transpose, "⤢ Transpose");
                    ui.selectable_value(&mut self.op, JpegLosslessOp::Transverse, "⤡ Transverse");
                });

                ui.add_space(10.0);
                ui.separator();
                ui.strong("Options:");
                ui.checkbox(
                    &mut self.reset_exif_orientation,
                    "Reset EXIF orientation tag to 1 (Normal)",
                );
                ui.checkbox(
                    &mut self.create_backup,
                    "Create backup copy of original file (.bak)",
                );
                ui.checkbox(
                    &mut self.trim_partial_mcu,
                    "Trim non-standard edge MCU blocks (if any)",
                );

                ui.add_space(10.0);
                ui.separator();
                ui.strong("Destination:");
                ui.radio_value(&mut self.save_to_new_file, false, "Overwrite original file");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.save_to_new_file, true, "Save transformed copy as:");
                    if self.save_to_new_file {
                        let path_display = self
                            .custom_output_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Select path...");
                        if ui.button(format!("📁 {path_display}")).clicked() {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter("JPEG Images", &["jpg", "jpeg"])
                                .save_file()
                            {
                                self.custom_output_path = Some(dest);
                            }
                        }
                    }
                });

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let can_apply = !self.save_to_new_file || self.custom_output_path.is_some();
                    if ui
                        .add_enabled(can_apply, egui::Button::new("✔ Apply Lossless Transform"))
                        .clicked()
                        || (can_apply && ctx.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        self.should_apply = Some(LosslessJpegConfig {
                            op: self.op,
                            create_backup: self.create_backup,
                            trim_partial_mcu: self.trim_partial_mcu,
                            reset_exif_orientation: self.reset_exif_orientation,
                            destination_path: if self.save_to_new_file {
                                self.custom_output_path.clone()
                            } else {
                                None
                            },
                        });
                        close_requested = true;
                    }

                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });

        if !is_open || close_requested {
            self.is_open = false;
        }
    }
}
