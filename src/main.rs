use std::path::PathBuf;
use std::time::Instant;
use eframe::egui;
use imge::app::ImgeApp;

fn ensure_window_focus() {
    // Cinnamon/Muffin and X11 compositors enforce Focus Stealing Prevention (FSP)
    // when launched from file managers like Double Commander.
    // An external activation signal via xdotool (if present on X11) bypasses FSP
    // and brings the window to the active foreground.
    std::thread::spawn(|| {
        let pid = std::process::id();
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Ok(out) = std::process::Command::new("xdotool")
                .args(["search", "--pid", &pid.to_string(), "--onlyvisible"])
                .output()
            {
                if let Ok(stdout) = String::from_utf8(out.stdout) {
                    if let Some(win_id) = stdout.lines().next() {
                        let win_id = win_id.trim();
                        if !win_id.is_empty() {
                            let _ = std::process::Command::new("xdotool")
                                .args(["windowactivate", win_id])
                                .status();
                            break;
                        }
                    }
                }
            }
        }
    });
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
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-v" || arg == "-V") {
        println!("imge v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let initial_path = args.get(1).map(PathBuf::from);
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

    ensure_window_focus();

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
