use std::path::{Path, PathBuf};
use std::time::Instant;
use imge::image_engine::{ImageEngine, JpegLosslessOp, LosslessJpegConfig, ResizeFilter};

fn get_process_memory() -> (u64, u64) {
    let mut vmrss = 0;
    let mut priv_dirty = 0;
    if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    vmrss = val.parse().unwrap_or(0);
                }
            }
        }
    }
    if let Ok(content) = std::fs::read_to_string("/proc/self/smaps_rollup") {
        for line in content.lines() {
            if line.starts_with("Private_Dirty:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    priv_dirty = val.parse().unwrap_or(0);
                }
            }
        }
    }
    (vmrss, priv_dirty)
}

fn bench_image_load(path: &Path, label: &str) {
    let runs = 5;
    let mut times = Vec::new();
    for _ in 0..runs {
        let mut engine = ImageEngine::new();
        let start = Instant::now();
        engine.load_file(path).unwrap();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let avg = times.iter().sum::<f64>() / runs as f64;
    let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    println!("  {:<20} | Avg: {:6.2} ms | Min: {:6.2} ms | Max: {:6.2} ms", label, avg, min, max);
}

fn bench_to_egui_image(path: &Path, label: &str) {
    let mut engine = ImageEngine::new();
    engine.load_file(path).unwrap();
    let runs = 5;
    let mut times = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let _egui_img = engine.to_egui_image();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let avg = times.iter().sum::<f64>() / runs as f64;
    println!("  {:<20} | to_egui_image: {:6.2} ms", label, avg);
}

fn main() {
    println!("=== IMGE ENGINE PERFORMANCE BENCHMARKS ===");
    let (init_rss, init_priv) = get_process_memory();
    println!("Baseline Process Memory: VmRSS = {:.2} MB, Private Dirty = {:.2} MB\n", 
        init_rss as f64 / 1024.0, init_priv as f64 / 1024.0);

    let test_images = [
        ("assets/icons/128x128.png", "128x128 PNG (4KB)"),
        ("bench/assets/sample_1080p.jpg", "1080p Photo JPG (400KB)"),
        ("bench/assets/sample_1080p.png", "1080p Photo PNG (3.7MB)"),
        ("bench/assets/sample_4k.jpg", "4K Photo JPG (1.5MB)"),
        ("bench/assets/sample_4k.png", "4K Photo PNG (15MB)"),
    ];

    println!("--- 1. Image Decode & Load Times (image::open) ---");
    for (path, label) in &test_images {
        bench_image_load(Path::new(path), label);
    }

    println!("\n--- 2. DynamicImage -> egui::ColorImage conversion ---");
    for (path, label) in &[
        ("bench/assets/sample_1080p.jpg", "1080p RGBA conv"),
        ("bench/assets/sample_4k.jpg", "4K RGBA conv"),
    ] {
        bench_to_egui_image(Path::new(path), label);
    }

    println!("\n--- 3. Core Manipulation Transforms (4K Image: 3840x2160) ---");
    let mut engine_4k = ImageEngine::new();
    engine_4k.load_file(Path::new("bench/assets/sample_4k.jpg")).unwrap();
    
    // Rotate CW
    let t0 = Instant::now();
    engine_4k.rotate_cw();
    println!("  Rotate 90 CW (4K)       : {:6.2} ms", t0.elapsed().as_secs_f64() * 1000.0);

    // Flip H
    let t0 = Instant::now();
    engine_4k.flip_h();
    println!("  Flip Horizontal (4K)    : {:6.2} ms", t0.elapsed().as_secs_f64() * 1000.0);

    // Crop (center 1920x1080)
    let t0 = Instant::now();
    engine_4k.crop(500, 500, 1920, 1080);
    println!("  Crop 1920x1080 out of 4K: {:6.2} ms", t0.elapsed().as_secs_f64() * 1000.0);

    println!("\n--- 4. Resize Resampling Filters (4K -> 1920x1080) ---");
    for (filter, name) in [
        (ResizeFilter::Nearest, "Nearest Neighbor"),
        (ResizeFilter::Triangle, "Triangle (Bilinear)"),
        (ResizeFilter::CatmullRom, "Catmull-Rom (Bicubic)"),
        (ResizeFilter::Lanczos3, "Lanczos3"),
    ] {
        let mut eng = ImageEngine::new();
        eng.load_file(Path::new("bench/assets/sample_4k.jpg")).unwrap();
        let t0 = Instant::now();
        eng.resize(1920, 1080, filter);
        println!("  {:<24} : {:6.2} ms", name, t0.elapsed().as_secs_f64() * 1000.0);
    }

    println!("\n--- 5. Lossless TurboJPEG vs Standard Recompression ---");
    let src_jpg = Path::new("bench/assets/sample_4k.jpg");
    let mut eng = ImageEngine::new();
    eng.load_file(src_jpg).unwrap();

    // Lossless DCT rotate
    let config = LosslessJpegConfig {
        op: JpegLosslessOp::Rot90,
        create_backup: false,
        trim_partial_mcu: false,
        reset_exif_orientation: false,
        destination_path: Some(PathBuf::from("/tmp/imge_bench_4k_lossless.jpg")),
    };
    let t0 = Instant::now();
    eng.apply_lossless_jpeg_transform(&config).unwrap();
    let lossless_time = t0.elapsed().as_secs_f64() * 1000.0;

    // Lossy decode + rotate + re-encode
    let t0 = Instant::now();
    let img = image::open(src_jpg).unwrap();
    let rot = img.rotate90();
    let mut out_file = std::fs::File::create("/tmp/imge_bench_4k_lossy.jpg").unwrap();
    rot.write_to(&mut out_file, image::ImageFormat::Jpeg).unwrap();
    let lossy_time = t0.elapsed().as_secs_f64() * 1000.0;

    println!("  Lossless TurboJPEG Transform : {:6.2} ms", lossless_time);
    println!("  Standard Decode+Rotate+Encode: {:6.2} ms ({:.1}x slower)", lossy_time, lossy_time / lossless_time);

    println!("\n--- 6. Undo/Redo Stack Memory Growth (Max 3 edits on 4K image) ---");
    let mut eng_undo = ImageEngine::new();
    eng_undo.load_file(Path::new("bench/assets/sample_4k.jpg")).unwrap();
    let (rss_base, priv_base) = get_process_memory();
    println!("  Loaded 4K Base    : VmRSS = {:.2} MB, Private Dirty = {:.2} MB", 
        rss_base as f64 / 1024.0, priv_base as f64 / 1024.0);

    for i in 1..=5 {
        eng_undo.flip_h();
        let (rss_now, priv_now) = get_process_memory();
        println!("  After {:2} edits  : VmRSS = {:.2} MB (+{:.2} MB), Private Dirty = {:.2} MB (+{:.2} MB)",
            i,
            rss_now as f64 / 1024.0, (rss_now - rss_base) as f64 / 1024.0,
            priv_now as f64 / 1024.0, (priv_now - priv_base) as f64 / 1024.0,
        );
    }
}
