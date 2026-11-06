use std::path::PathBuf;
use std::time::Instant;
use eframe::egui;
use imge::app::ImgeApp;
use raw_window_handle::HasWindowHandle;

fn decode_percent_encoded(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                decoded.push(byte);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| input.to_string())
}

fn parse_image_path(arg: &str) -> Option<PathBuf> {
    // If it starts with file://, strip the scheme
    let raw_path = if let Some(stripped) = arg.strip_prefix("file://") {
        stripped.strip_prefix("localhost").unwrap_or(stripped)
    } else {
        arg
    };

    let decoded = decode_percent_encoded(raw_path);
    let path = PathBuf::from(&decoded);
    if path.exists() {
        Some(path)
    } else {
        let raw = PathBuf::from(raw_path);
        if raw.exists() {
            Some(raw)
        } else {
            Some(path)
        }
    }
}

fn load_app_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../assets/icons/128x128.png");
    let image = image::load_from_memory(icon_bytes).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}

fn main() {
    let t_start = Instant::now();

    // Consume Freedesktop / XDG startup tokens immediately at launch
    let startup_id = imge::startup_notify::take_startup_id();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-v" || arg == "-V") {
        println!("imge v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("imge - Fast, lightweight IrfanView-inspired image viewer and editor");
        println!("\nUsage: imge [OPTIONS] [FILE]");
        println!("\nArguments:");
        println!("  [FILE]  Path or file:// URI to an image to open");
        println!("\nOptions:");
        println!("  -h, --help     Print help");
        println!("  -v, --version  Print version");
        return;
    }

    let initial_path = args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .and_then(|arg| parse_image_path(arg));

    let do_profile = std::env::var("IMGE_PROFILE").is_ok();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1024.0, 720.0])
        .with_min_inner_size([400.0, 300.0])
        .with_title(format!("imge v{}", env!("CARGO_PKG_VERSION")))
        .with_app_id("imge")
        .with_active(true)
        .with_visible(true)
        .with_drag_and_drop(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if do_profile {
        eprintln!("[profile] Options prepared: {:.2} ms", t_start.elapsed().as_secs_f64() * 1000.0);
    }

    let _ = eframe::run_native(
        "imge",
        native_options,
        Box::new(move |cc| {
            if do_profile {
                eprintln!("[profile] eframe cc created (GL context ready): {:.2} ms", t_start.elapsed().as_secs_f64() * 1000.0);
            }
            // Signal startup notification completion and focus window
            let raw_handle = cc.window_handle().ok().map(|h| h.as_raw());
            imge::startup_notify::complete_startup(raw_handle, startup_id.as_deref());

            let app = ImgeApp::new(initial_path, cc);
            if do_profile {
                eprintln!("[profile] ImgeApp::new finished: {:.2} ms", t_start.elapsed().as_secs_f64() * 1000.0);
            }
            Ok(Box::new(app))
        }),
    );

    // Cleanly terminate process so lingering background worker threads
    // do not block process termination or trigger compositor unresponsive window warnings.
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_percent_encoded() {
        assert_eq!(decode_percent_encoded("/path/to/my%20image.png"), "/path/to/my image.png");
        assert_eq!(decode_percent_encoded("/path/hello%2Fworld"), "/path/hello/world");
        assert_eq!(decode_percent_encoded("no_percent"), "no_percent");
    }

    #[test]
    fn test_parse_image_path() {
        let p = parse_image_path("file:///tmp/nonexistent.png").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/nonexistent.png"));

        let p2 = parse_image_path("file://localhost/tmp/photo%201.jpg").unwrap();
        assert_eq!(p2, PathBuf::from("/tmp/photo 1.jpg"));

        let p3 = parse_image_path("/tmp/regular.png").unwrap();
        assert_eq!(p3, PathBuf::from("/tmp/regular.png"));
    }
}
