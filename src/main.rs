mod app;
mod dialogs;
mod image_engine;
mod viewer;

use std::path::PathBuf;
use app::ImgeApp;
use eframe::egui;

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

fn main() {
    let initial_path = std::env::args().nth(1).map(PathBuf::from);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("imge")
            .with_app_id("imge")
            .with_active(true)
            .with_visible(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    ensure_window_focus();

    let _ = eframe::run_native(
        "imge",
        native_options,
        Box::new(|cc| Ok(Box::new(ImgeApp::new(initial_path, cc)))),
    );

    // Cleanly terminate process so lingering background worker threads
    // do not block process termination or trigger compositor unresponsive window warnings.
    std::process::exit(0);
}
