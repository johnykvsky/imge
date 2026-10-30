use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use eframe::egui;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, RgbaImage};

const MAX_HISTORY: usize = 15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResizeFilter {
    Lanczos3,
    CatmullRom,
    Triangle,
    Nearest,
}

impl ResizeFilter {
    pub fn to_image_filter(self) -> FilterType {
        match self {
            ResizeFilter::Lanczos3 => FilterType::Lanczos3,
            ResizeFilter::CatmullRom => FilterType::CatmullRom,
            ResizeFilter::Triangle => FilterType::Triangle,
            ResizeFilter::Nearest => FilterType::Nearest,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JpegLosslessOp {
    Rot90,
    Rot180,
    Rot270,
    Hflip,
    Vflip,
    Transpose,
    Transverse,
}

impl JpegLosslessOp {
    pub fn description(&self) -> &'static str {
        match self {
            JpegLosslessOp::Rot90 => "Rotate 90° CW",
            JpegLosslessOp::Rot180 => "Rotate 180°",
            JpegLosslessOp::Rot270 => "Rotate 90° CCW",
            JpegLosslessOp::Hflip => "Flip Horizontal",
            JpegLosslessOp::Vflip => "Flip Vertical",
            JpegLosslessOp::Transpose => "Transpose",
            JpegLosslessOp::Transverse => "Transverse",
        }
    }

    pub fn to_turbojpeg_op(self) -> turbojpeg::TransformOp {
        match self {
            JpegLosslessOp::Rot90 => turbojpeg::TransformOp::Rot90,
            JpegLosslessOp::Rot180 => turbojpeg::TransformOp::Rot180,
            JpegLosslessOp::Rot270 => turbojpeg::TransformOp::Rot270,
            JpegLosslessOp::Hflip => turbojpeg::TransformOp::Hflip,
            JpegLosslessOp::Vflip => turbojpeg::TransformOp::Vflip,
            JpegLosslessOp::Transpose => turbojpeg::TransformOp::Transpose,
            JpegLosslessOp::Transverse => turbojpeg::TransformOp::Transverse,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LosslessJpegConfig {
    pub op: JpegLosslessOp,
    pub create_backup: bool,
    pub trim_partial_mcu: bool,
    pub reset_exif_orientation: bool,
    pub destination_path: Option<PathBuf>,
}

#[derive(Default)]
pub struct ImageEngine {
    pub current: Option<DynamicImage>,
    pub file_path: Option<PathBuf>,
    pub is_modified: bool,
    undo_stack: Vec<DynamicImage>,
    redo_stack: Vec<DynamicImage>,
}

impl ImageEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_image(&self) -> bool {
        self.current.is_some()
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.current.as_ref().map(|img| img.dimensions()).unwrap_or((0, 0))
    }

    pub fn file_name(&self) -> String {
        if let Some(ref path) = self.file_path {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else if self.has_image() {
            "Untitled (Clipboard)".to_string()
        } else {
            "No Image".to_string()
        }
    }

    pub fn file_size_bytes(&self) -> Option<u64> {
        self.file_path.as_ref().and_then(|p| std::fs::metadata(p).ok().map(|m| m.len()))
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let img = image::open(path).map_err(|e| format!("Failed to open image: {e}"))?;
        self.current = Some(img);
        self.file_path = Some(path.to_path_buf());
        self.is_modified = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    pub fn load_dynamic_image(&mut self, img: DynamicImage, path: Option<PathBuf>) {
        self.is_modified = path.is_none();
        self.current = Some(img);
        self.file_path = path;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn push_undo(&mut self) {
        if let Some(ref current) = self.current {
            self.undo_stack.push(current.clone());
            if self.undo_stack.len() > MAX_HISTORY {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
            self.is_modified = true;
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            if let Some(current) = self.current.take() {
                self.redo_stack.push(current);
            }
            self.current = Some(prev);
            self.is_modified = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            if let Some(current) = self.current.take() {
                self.undo_stack.push(current);
            }
            self.current = Some(next);
            self.is_modified = true;
            true
        } else {
            false
        }
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32, filter: ResizeFilter) -> bool {
        if new_w == 0 || new_h == 0 {
            return false;
        }
        let (w, h) = self.dimensions();
        if w == new_w && h == new_h {
            return false;
        }

        self.push_undo();
        if let Some(ref current) = self.current {
            let resized = current.resize_exact(new_w, new_h, filter.to_image_filter());
            self.current = Some(resized);
            true
        } else {
            false
        }
    }

    pub fn crop(&mut self, x: u32, y: u32, w: u32, h: u32) -> bool {
        if w == 0 || h == 0 {
            return false;
        }
        let (img_w, img_h) = self.dimensions();
        if x >= img_w || y >= img_h {
            return false;
        }

        let clamped_w = w.min(img_w - x);
        let clamped_h = h.min(img_h - y);
        if clamped_w == 0 || clamped_h == 0 {
            return false;
        }

        self.push_undo();
        if let Some(ref current) = self.current {
            let cropped = current.crop_imm(x, y, clamped_w, clamped_h);
            self.current = Some(cropped);
            true
        } else {
            false
        }
    }

    pub fn rotate_cw(&mut self) -> bool {
        self.push_undo();
        if let Some(ref current) = self.current {
            self.current = Some(current.rotate90());
            true
        } else {
            false
        }
    }

    pub fn rotate_ccw(&mut self) -> bool {
        self.push_undo();
        if let Some(ref current) = self.current {
            self.current = Some(current.rotate270());
            true
        } else {
            false
        }
    }

    pub fn flip_h(&mut self) -> bool {
        self.push_undo();
        if let Some(ref current) = self.current {
            self.current = Some(current.fliph());
            true
        } else {
            false
        }
    }

    pub fn flip_v(&mut self) -> bool {
        self.push_undo();
        if let Some(ref current) = self.current {
            self.current = Some(current.flipv());
            true
        } else {
            false
        }
    }

    pub fn save(&mut self, path: &Path, quality: u8) -> Result<(), String> {
        let current = self.current.as_ref().ok_or_else(|| "No image to save".to_string())?;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();

        let format = ImageFormat::from_extension(&ext).unwrap_or(ImageFormat::Png);

        let file = File::create(path).map_err(|e| format!("Failed to create file: {e}"))?;
        let mut writer = BufWriter::new(file);

        match format {
            ImageFormat::Jpeg => {
                let rgb = current.to_rgb8();
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
                encoder
                    .encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                    .map_err(|e| format!("Failed to encode JPEG: {e}"))?;
            }
            _ => {
                current
                    .write_to(&mut writer, format)
                    .map_err(|e| format!("Failed to save image: {e}"))?;
            }
        }

        self.file_path = Some(path.to_path_buf());
        self.is_modified = false;
        Ok(())
    }

    pub fn is_jpeg_file(&self) -> bool {
        if let Some(ref path) = self.file_path {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let lower = ext.to_lowercase();
                return lower == "jpg" || lower == "jpeg";
            }
        }
        false
    }

    pub fn apply_lossless_jpeg_transform(
        &mut self,
        config: &LosslessJpegConfig,
    ) -> Result<PathBuf, String> {
        let src_path = self
            .file_path
            .as_ref()
            .ok_or_else(|| "No file path associated with the current image.".to_string())?;

        if !src_path.exists() {
            return Err("Source file does not exist on disk.".to_string());
        }

        if !self.is_jpeg_file() {
            return Err(
                "JPG Lossless operations can only be performed on JPEG (.jpg / .jpeg) files."
                    .to_string(),
            );
        }

        let src_bytes = std::fs::read(src_path)
            .map_err(|e| format!("Failed to read source JPEG file: {e}"))?;

        let mut transform = turbojpeg::Transform::op(config.op.to_turbojpeg_op());
        transform.trim = config.trim_partial_mcu;
        transform.copy_none = false; // preserve EXIF, ICC profiles, and metadata

        let transformed = turbojpeg::transform(&transform, &src_bytes)
            .map_err(|e| format!("TurboJPEG lossless transform failed: {e}"))?;

        let mut out_bytes = transformed.to_vec();

        if config.reset_exif_orientation {
            reset_exif_orientation(&mut out_bytes);
        }

        let target_path = config
            .destination_path
            .clone()
            .unwrap_or_else(|| src_path.clone());

        // Create .bak backup if requested and saving in place
        if config.create_backup && target_path == *src_path {
            let bak_path = src_path.with_extension("jpg.bak");
            let _ = std::fs::copy(src_path, &bak_path);
        }

        std::fs::write(&target_path, &out_bytes)
            .map_err(|e| format!("Failed to write transformed JPEG: {e}"))?;

        Ok(target_path)
    }

    pub fn to_egui_image(&self) -> Option<egui::ColorImage> {
        self.current.as_ref().map(|img| {
            let rgba: RgbaImage = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw())
        })
    }

    pub fn get_metadata(&self) -> Option<ImageMetadata> {
        let current = self.current.as_ref()?;
        let (w, h) = current.dimensions();
        let color_type = match current.color() {
            image::ColorType::Rgb8 => "RGB (24 bpp)".to_string(),
            image::ColorType::Rgba8 => "RGBA with Alpha (32 bpp)".to_string(),
            image::ColorType::L8 => "Grayscale (8 bpp)".to_string(),
            image::ColorType::La8 => "Grayscale with Alpha (16 bpp)".to_string(),
            image::ColorType::Rgb16 => "RGB 16-bit (48 bpp)".to_string(),
            image::ColorType::Rgba16 => "RGBA 16-bit (64 bpp)".to_string(),
            other => format!("{other:?}"),
        };

        fn gcd(mut a: u32, mut b: u32) -> u32 {
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a
        }

        let aspect_ratio_str = if h > 0 {
            let g = gcd(w, h);
            if g > 0 && w / g <= 32 && h / g <= 32 {
                format!("{}:{} ({:.2}:1)", w / g, h / g, w as f64 / h as f64)
            } else {
                format!("{:.2}:1", w as f64 / h as f64)
            }
        } else {
            "N/A".to_string()
        };

        let memory_footprint_bytes = (w as u64) * (h as u64) * 4;

        let mut exif_tags = Vec::new();
        if let Some(ref path) = self.file_path {
            if let Ok(file) = std::fs::File::open(path) {
                let mut bufreader = std::io::BufReader::new(file);
                let exifreader = exif::Reader::new();
                if let Ok(exif_data) = exifreader.read_from_container(&mut bufreader) {
                    for f in exif_data.fields() {
                        exif_tags.push((f.tag.to_string(), f.display_value().to_string()));
                    }
                }
            }
        }

        Some(ImageMetadata {
            file_name: self.file_name(),
            file_path: self.file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            file_size_bytes: self.file_size_bytes(),
            width: w,
            height: h,
            aspect_ratio_str,
            color_type,
            memory_footprint_bytes,
            exif_tags,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImageMetadata {
    pub file_name: String,
    pub file_path: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub aspect_ratio_str: String,
    pub color_type: String,
    pub memory_footprint_bytes: u64,
    pub exif_tags: Vec<(String, String)>,
}

/// In-place reset of EXIF Orientation tag to 1 (Normal / top-left) in raw JPEG byte buffer.
/// Returns true if an orientation tag was found and patched, false otherwise.
pub fn reset_exif_orientation(data: &mut [u8]) -> bool {
    let len = data.len();
    if len < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return false;
    }

    let mut pos = 2;
    while pos + 4 <= len {
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        if marker == 0xDA || marker == 0xD9 {
            // SOS (Start of Scan) or EOI (End of Image)
            break;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 || pos + 2 + seg_len > len {
            break;
        }

        if marker == 0xE1 {
            // APP1 marker
            let app1_data = &mut data[pos + 4..pos + 2 + seg_len];
            if app1_data.len() >= 14 && &app1_data[..6] == b"Exif\0\0" {
                let tiff = &mut app1_data[6..];
                return patch_tiff_orientation(tiff);
            }
        }

        pos += 2 + seg_len;
    }
    false
}

fn patch_tiff_orientation(tiff: &mut [u8]) -> bool {
    if tiff.len() < 8 {
        return false;
    }
    let is_le = if &tiff[0..2] == b"II" && tiff[2..4] == [0x2A, 0x00] {
        true
    } else if &tiff[0..2] == b"MM" && tiff[2..4] == [0x00, 0x2A] {
        false
    } else {
        return false;
    };

    let ifd0_offset = if is_le {
        u32::from_le_bytes([tiff[4], tiff[5], tiff[6], tiff[7]]) as usize
    } else {
        u32::from_be_bytes([tiff[4], tiff[5], tiff[6], tiff[7]]) as usize
    };

    if ifd0_offset + 2 > tiff.len() {
        return false;
    }

    let num_entries = if is_le {
        u16::from_le_bytes([tiff[ifd0_offset], tiff[ifd0_offset + 1]]) as usize
    } else {
        u16::from_be_bytes([tiff[ifd0_offset], tiff[ifd0_offset + 1]]) as usize
    };

    let mut entry_pos = ifd0_offset + 2;
    for _ in 0..num_entries {
        if entry_pos + 12 > tiff.len() {
            break;
        }
        let tag = if is_le {
            u16::from_le_bytes([tiff[entry_pos], tiff[entry_pos + 1]])
        } else {
            u16::from_be_bytes([tiff[entry_pos], tiff[entry_pos + 1]])
        };

        if tag == 0x0112 {
            // Orientation tag (type SHORT = 3, count = 1)
            if is_le {
                tiff[entry_pos + 8] = 1;
                tiff[entry_pos + 9] = 0;
            } else {
                tiff[entry_pos + 8] = 0;
                tiff[entry_pos + 9] = 1;
            }
            return true;
        }
        entry_pos += 12;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn create_test_image(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for x in 0..w {
            for y in 0..h {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_initial_state() {
        let mut engine = ImageEngine::new();
        assert!(!engine.has_image());
        assert_eq!(engine.dimensions(), (0, 0));

        engine.load_dynamic_image(create_test_image(200, 100), None);
        assert!(engine.has_image());
        assert_eq!(engine.dimensions(), (200, 100));
        assert!(engine.is_modified);
        assert!(!engine.can_undo());
        assert!(!engine.can_redo());
    }

    #[test]
    fn test_resize_and_undo_redo() {
        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(200, 100), None);

        assert!(engine.resize(400, 300, ResizeFilter::Lanczos3));
        assert_eq!(engine.dimensions(), (400, 300));
        assert!(engine.can_undo());

        // Undo
        assert!(engine.undo());
        assert_eq!(engine.dimensions(), (200, 100));
        assert!(engine.can_redo());

        // Redo
        assert!(engine.redo());
        assert_eq!(engine.dimensions(), (400, 300));
    }

    #[test]
    fn test_crop() {
        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(200, 100), None);

        assert!(engine.crop(50, 20, 100, 60));
        assert_eq!(engine.dimensions(), (100, 60));
    }

    #[test]
    fn test_rotate_and_flip() {
        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(200, 100), None);

        assert!(engine.rotate_cw());
        assert_eq!(engine.dimensions(), (100, 200));

        assert!(engine.rotate_ccw());
        assert_eq!(engine.dimensions(), (200, 100));

        assert!(engine.flip_h());
        assert!(engine.flip_v());
        assert_eq!(engine.dimensions(), (200, 100));
    }

    #[test]
    fn test_save_and_reload() {
        let temp_dir = std::env::temp_dir();
        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(80, 60), None);

        for ext in ["png", "jpg", "webp", "bmp"] {
            let path = temp_dir.join(format!("imge_test_out.{ext}"));
            let res = engine.save(&path, 90);
            assert!(res.is_ok(), "Failed saving {ext}: {:?}", res);
            assert!(path.exists());

            let mut reloaded = ImageEngine::new();
            assert!(reloaded.load_file(&path).is_ok());
            assert_eq!(reloaded.dimensions(), (80, 60));
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn test_metadata() {
        let mut engine = ImageEngine::new();
        assert!(engine.get_metadata().is_none());

        engine.load_dynamic_image(create_test_image(1920, 1080), None);
        let meta = engine.get_metadata().expect("Metadata should exist");
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert_eq!(meta.aspect_ratio_str, "16:9 (1.78:1)");
        assert_eq!(meta.color_type, "RGBA with Alpha (32 bpp)");
        assert_eq!(meta.file_name, "Untitled (Clipboard)");
        assert_eq!(meta.file_path, None);
        assert!(meta.memory_footprint_bytes > 0);
    }

    #[test]
    fn test_turbojpeg_lossless_transform() {
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("test_lossless_input.jpg");

        // Create an 85x43 test JPEG (non-multiple of 16)
        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(85, 43), None);
        engine.save(&test_path, 90).unwrap();

        let src_bytes = std::fs::read(&test_path).unwrap();
        let mut transform = turbojpeg::Transform::op(turbojpeg::TransformOp::Rot90);
        transform.trim = false;
        let transformed = turbojpeg::transform(&transform, &src_bytes).unwrap();

        let out_img = image::load_from_memory_with_format(&transformed, ImageFormat::Jpeg).unwrap();
        println!("Transformed non-multiple dimensions: {:?}", out_img.dimensions());

        let _ = std::fs::remove_file(test_path);
    }

    #[test]
    fn test_engine_lossless_jpeg_transform() {
        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("test_engine_lossless.jpg");
        let bak_path = temp_dir.join("test_engine_lossless.jpg.bak");

        let mut engine = ImageEngine::new();
        engine.load_dynamic_image(create_test_image(160, 96), None);
        engine.save(&test_path, 90).unwrap();
        assert!(engine.is_jpeg_file());

        // Test Rot90 with backup creation
        let config = LosslessJpegConfig {
            op: JpegLosslessOp::Rot90,
            create_backup: true,
            trim_partial_mcu: false,
            reset_exif_orientation: true,
            destination_path: None,
        };

        let result = engine.apply_lossless_jpeg_transform(&config);
        assert!(result.is_ok());
        assert!(bak_path.exists());

        // Reload to check dimensions
        assert!(engine.load_file(&test_path).is_ok());
        assert_eq!(engine.dimensions(), (96, 160));

        // Test Rot180
        let config_180 = LosslessJpegConfig {
            op: JpegLosslessOp::Rot180,
            create_backup: false,
            trim_partial_mcu: false,
            reset_exif_orientation: true,
            destination_path: None,
        };
        assert!(engine.apply_lossless_jpeg_transform(&config_180).is_ok());
        assert!(engine.load_file(&test_path).is_ok());
        assert_eq!(engine.dimensions(), (96, 160));

        let _ = std::fs::remove_file(&test_path);
        let _ = std::fs::remove_file(&bak_path);
    }
}
