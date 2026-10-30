use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

pub struct ImageViewer {
    pub zoom: f32,
    pub pan: Vec2,
    pub selection: Option<[u32; 4]>, // x, y, w, h in image coordinates
    is_selecting: bool,
    select_start: Option<Pos2>,
    pub crop_requested: bool,
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            selection: None,
            is_selecting: false,
            select_start: None,
            crop_requested: false,
        }
    }
}

impl ImageViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fit_to_viewport(&mut self, viewport: Rect, img_w: u32, img_h: u32) {
        if img_w == 0 || img_h == 0 {
            return;
        }
        let scale_x = viewport.width() / img_w as f32;
        let scale_y = viewport.height() / img_h as f32;
        self.zoom = scale_x.min(scale_y).clamp(0.01, 1.0);
        self.pan = Vec2::ZERO;
    }

    pub fn set_zoom_100(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(32.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.02);
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.is_selecting = false;
        self.select_start = None;
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        texture_id: Option<egui::TextureId>,
        img_w: u32,
        img_h: u32,
    ) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // Background canvas fill
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(32, 33, 36));

        if texture_id.is_none() || img_w == 0 || img_h == 0 {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No image loaded\nDrop an image file here or press Ctrl+V to paste",
                egui::FontId::proportional(16.0),
                Color32::from_rgb(140, 145, 155),
            );
            return;
        }

        let tex_id = texture_id.unwrap();

        // Handle Mouse Wheel Zoom anchored around cursor
        if let Some(hover_pos) = response.hover_pos() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta.abs() > 0.1 {
                // Smooth, proportional exponential zoom curve
                let zoom_factor = (scroll_delta * 0.002_f32).clamp(-0.35, 0.35).exp();
                let old_zoom = self.zoom;
                let new_zoom = (self.zoom * zoom_factor).clamp(0.02, 32.0);

                let center = rect.center() + self.pan;
                let offset_from_center = hover_pos - center;
                self.pan -= offset_from_center * (new_zoom / old_zoom - 1.0);
                self.zoom = new_zoom;
            }
        }

        // Image geometry on screen
        let display_size = Vec2::new(img_w as f32 * self.zoom, img_h as f32 * self.zoom);
        let img_center = rect.center() + self.pan;
        let img_rect = Rect::from_center_size(img_center, display_size);

        // Middle drag or Ctrl+Drag = Pan
        let is_pan_drag = response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.ctrl));

        if is_pan_drag {
            self.pan += response.drag_delta();
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                self.is_selecting = true;
                self.select_start = Some(pos);
            }
        } else if self.is_selecting && response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(start), Some(curr)) = (self.select_start, response.interact_pointer_pos()) {
                let screen_sel = Rect::from_two_pos(start, curr);
                // Convert screen rect to image coordinates
                let rel_min = screen_sel.min - img_rect.min;
                let rel_max = screen_sel.max - img_rect.min;

                let x0 = (rel_min.x / self.zoom).clamp(0.0, img_w as f32);
                let y0 = (rel_min.y / self.zoom).clamp(0.0, img_h as f32);
                let x1 = (rel_max.x / self.zoom).clamp(0.0, img_w as f32);
                let y1 = (rel_max.y / self.zoom).clamp(0.0, img_h as f32);

                let x = x0.min(x1).round() as u32;
                let y = y0.min(y1).round() as u32;
                let w = (x0.max(x1) - x0.min(x1)).round() as u32;
                let h = (y0.max(y1) - y0.min(y1)).round() as u32;

                if w > 2 && h > 2 {
                    self.selection = Some([x, y, w, h]);
                } else {
                    self.selection = None;
                }
            }
        }

        if response.drag_stopped() {
            self.is_selecting = false;
            self.select_start = None;
        }

        // Right click clears selection
        if response.clicked_by(egui::PointerButton::Secondary) {
            self.clear_selection();
        }

        // Double click inside selection triggers crop
        if response.double_clicked() && self.selection.is_some() {
            self.crop_requested = true;
        }

        // Paint Image
        let painter = ui.painter().with_clip_rect(rect);
        painter.image(
            tex_id,
            img_rect,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // Paint Selection Overlay if active
        if let Some([sx, sy, sw, sh]) = self.selection {
            let sel_min = img_rect.min + Vec2::new(sx as f32 * self.zoom, sy as f32 * self.zoom);
            let sel_max = sel_min + Vec2::new(sw as f32 * self.zoom, sh as f32 * self.zoom);
            let sel_rect = Rect::from_min_max(sel_min, sel_max);

            painter.rect_filled(sel_rect, 0.0, Color32::from_rgba_unmultiplied(0, 150, 255, 45));
            painter.rect_stroke(
                sel_rect,
                0.0,
                Stroke::new(1.5_f32, Color32::from_rgb(0, 180, 255)),
                egui::StrokeKind::Outside,
            );
        }
    }
}
