# AGENTS.md

Welcome to **imge**, a fast, lightweight image manipulation and viewing desktop application inspired by IrfanView, built for Linux (Manjaro / Arch / Wayland / X11) using **Rust**, **eframe/egui**, **image-rs**, and **arboard**.

## 1. Project Philosophy & Scope
- **Minimal, Fast, and Focused**: No layer stacks, no complex vector/brush drawing tools.
- **Single-Image Workflow**: Open, paste from clipboard, crop, resize, rotate/flip, and export across common formats (PNG, JPEG, WebP, BMP, TIFF).
- **Keyboard-Driven**: IrfanView-inspired single-key and standard modifier shortcuts (`Ctrl+V` to paste, `Ctrl+R` to resize, `Ctrl+S` to save, `R`/`L` to rotate, `H`/`V` to flip, `Ctrl+Z` to undo, `Esc` to close).
- **Native Binary**: Direct native ELF binary with instant startup (<50ms) and minimal RAM footprint.

## 2. Architecture & File Structure
```
imge/
├── .gitignore
├── Makefile               # Build, run, test, install, desktop integration, and clean automation
├── AGENTS.md              # Agent workflow, architecture, and instructions
├── README.md              # User documentation and shortcut guide
├── Cargo.toml             # Rust package manifest & dependencies
├── assets/
│   └── icons/
│       └── 128x128.png    # Application icon for desktop integration
└── src/
    ├── main.rs            # Entry point & eframe native window launcher
    ├── app.rs             # Main eframe::App, menu bar, toolbar, shortcuts, drag & drop
    ├── image_engine.rs    # Core engine: DynamicImage, undo/redo history, image transforms & I/O
    ├── viewer.rs          # Canvas widget: zoom/pan & crop rectangle selection
    └── dialogs.rs         # Modals: Resize, JPEG Quality, EXIF Info & Lossless JPEG
```

## 3. Tech Stack & Dependencies
- **Rust**: Edition 2024 (1.80+)
- **eframe / egui**: Immediate-mode hardware-accelerated GUI (using `glow` OpenGL backend for lightweight builds on Wayland & X11).
- **image (image-rs)**: Image decoding, processing (resampling, transforms), and encoding.
- **turbojpeg**: DCT-domain lossless JPEG transformations (rotation, flips) without recompression generational loss.
- **kamadak-exif**: Camera EXIF metadata extraction and decoding.
- **arboard**: Cross-platform system clipboard integration (raw bitmap read/write for instant `Ctrl+V` and `Ctrl+C`).
- **rfd**: Native desktop file open and save dialogs.

## 4. Key Components & Responsibilities
- `src/image_engine.rs`:
  - Holds active `DynamicImage` buffer.
  - Keeps an in-memory undo/redo history stack (capped at 15 states for memory efficiency).
  - Handles disk I/O, format conversions, and metadata tracking (dimensions, format, path).
- `src/viewer.rs`:
  - Canvas rendering using `egui::Painter`.
  - Implements rubber-band drag selection for cropping in image coordinates.
  - Handles mouse wheel zoom towards cursor and middle-click or drag panning.
- `src/app.rs`:
  - Orchestrates menus, toolbar, keyboard shortcuts, and status bar.
  - Integrates `arboard` for instant `Ctrl+V` clipboard pasting.
  - Handles directory navigation (Previous/Next image in same directory) and drag-and-drop.
- `src/dialogs.rs`:
  - `ResizeModal`: Pixel and percentage resize modes with aspect ratio preservation.
  - `QualityModal`: JPEG compression quality slider.
  - `ImageInfoModal`: Image properties (dimensions, aspect ratio, color depth, memory size) and camera EXIF table (`I`).
  - `LosslessJpegModal`: Lossless JPEG DCT operations, EXIF reset, and backup management (`Shift+J`).

## 5. Development & Testing Instructions
- Build release binary:
  ```bash
  make build        # Or: cargo build --release
  # Output binary: target/release/imge
  ```
- Run directly:
  ```bash
  make run          # Or: cargo run -- /path/to/image.png
  make run FILE="/path/to/image.png"
  make run ARGS="/path/to/image.png"
  ```
- Run unit tests:
  ```bash
  make test         # Or: cargo test
  ```
- Clean build cache and artifacts:
  ```bash
  make clean        # Or: cargo clean
  ```

## 6. Coding Guidelines for Agents
- Do not add bloat or multi-layer editing features; keep the UX simple and snappy.
- Preserve keyboard shortcuts and ensure actions are intuitive.
- Maintain memory safety when handling large images.
- Ensure clipboard pasting handles image data cleanly.
