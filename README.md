# imge

A fast, lightweight, and easy-to-use native image viewer and manipulation application for Linux (Manjaro, Arch, Ubuntu, Fedora, etc.), built in **Rust** and inspired by IrfanView.

Designed for quick edits without unnecessary complexity: open, paste from clipboard, crop, resize, rotate, flip, and export to any popular image format. No layers, no heavy drawing tools, minimal memory footprint.

![imge](https://img.shields.io/badge/Language-Rust-orange)
![GUI](https://img.shields.io/badge/GUI-egui%20%2F%20eframe-blue)
![Engine](https://img.shields.io/badge/Engine-image--rs-green)

---

## Features

- ⚡ **Instant Startup & Native Performance**: Standalone native ELF binary with sub-50ms launch time and tiny RAM footprint (~30 MB).
- 📋 **Paste from Clipboard (`Ctrl+V`)**: Instant paste from screenshots (`PrintScreen`), browsers, or other apps directly into an editable canvas, ready to save.
- 📐 **Smart Resize (`Ctrl+R`)**:
  - Pixel dimensions or percentage scale.
  - Linked aspect ratio lock/unlock.
  - High-quality resampling filters (Lanczos3, Catmull-Rom, Triangle/Bilinear, Nearest Neighbor).
- ✂️ **Rubber-Band Cropping**: Drag a selection box over the image and hit `Enter` or double-click to crop.
- 🔄 **Quick Transforms**:
  - Rotate 90° clockwise (`R`) or counter-clockwise (`L`).
  - Flip horizontal (`H`) or vertical (`V`).
- ⚡ **Lossless JPEG Transforms (`Shift+J`)**:
  - Rotate (90° CW, 180°, 90° CCW) and flip JPEG images directly in the DCT frequency domain without recompressing or losing quality.
  - Automatic EXIF orientation reset, optional `.bak` backup, and overwrite or save-as support.
- 💾 **Multi-Format Export**:
  - Save as PNG, JPEG, WebP, BMP, TIFF.
  - Interactive quality slider for lossy JPEG compression.
- ⏪ **Undo / Redo (`Ctrl+Z` / `Ctrl+Y`)**: Non-destructive history for all operations.
- ℹ️ **Image Information & EXIF (`I`)**: View image properties (dimensions, aspect ratio, color depth, memory footprint) and detailed camera EXIF tags.
- 📁 **Folder Navigation**: Browse previous/next images in the same folder using arrow keys or `Space`/`Backspace`.
- 🖱️ **Drag and Drop**: Drag any image file from Dolphin, Nautilus, or your desktop directly into the window.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + O` | Open image file |
| `Ctrl + V` | **Paste image from clipboard** |
| `Ctrl + C` | **Copy to clipboard** (selection if active, otherwise full image) |
| `Ctrl + S` | Save image |
| `Ctrl + Shift + S` | Save As (choose format / location) |
| `Ctrl + R` | **Resize image** |
| `Enter` / Double-Click | **Crop** to selected rectangle |
| `Escape` | **Close application** (or clear crop selection if active) |
| `Ctrl + Q` | **Quit application** |
| `I` | **Image Information & EXIF metadata** |
| `Shift + J` | **JPG Lossless Operations dialog** |
| `R` | Rotate 90° Clockwise |
| `L` | Rotate 90° Counter-Clockwise |
| `H` | Flip Horizontal |
| `V` | Flip Vertical |
| `Ctrl + Z` | Undo |
| `Ctrl + Shift + Z` / `Ctrl + Y` | Redo |
| `1` / `Ctrl + 0` | 100% Actual Size |
| `F` | Fit image to window |
| `Left-Click Drag` | Draw rubber-band crop selection |
| `Right-Click Drag` | **Pan / move image** in window |
| `Right-Click` | Clear crop selection |
| `Middle-Click Drag` / `Ctrl + Drag` | Pan / move image in window |
| `+` / `-` or `Mouse Wheel` | Zoom In / Zoom Out (centered on cursor) |
| `Right` / `Space` | Next image in directory |
| `Left` / `Backspace` | Previous image in directory |

---

## Building from Source

### Prerequisites
- **Rust & Cargo** (1.80+):
  ```bash
  # Available via rustup:
  rustup default stable
  ```
- **System Dependencies**:
  - Arch / Manjaro: `sudo pacman -S libjpeg-turbo pkgconf` (usually pre-installed)
  - Ubuntu / Debian: `sudo apt install libturbojpeg0-dev pkg-config`
  - Fedora: `sudo dnf install turbojpeg-devel pkgconf-pkg-config`

### Quick Commands (Makefile)
A [`Makefile`](Makefile) is provided for building, desktop integration, and system installation:
```bash
make build             # Compile optimized release binary (target/release/imge)
make run [FILE=x.png]  # Build and run imge (optional file argument)
make test              # Run unit test suite
make install           # Install binary to ~/.local/bin/imge
make install-desktop   # Install binary, desktop shortcut, icon & associate files
make associate-mime    # Set imge as default viewer for all supported image formats
make uninstall         # Remove binary, desktop shortcut, and icon
make clean             # Clean cargo build artifacts
make distclean         # Deep clean target/ and local temporary caches
make help              # Show all available targets
```

### Manual Cargo Commands
To build the optimized release binary directly with Cargo:
```bash
cargo build --release
```
The compiled binary will be located at:
```bash
target/release/imge
```

You can optionally copy it to your system path:
```bash
cp target/release/imge ~/.local/bin/
```

---

## Usage

Run directly:
```bash
./target/release/imge
```

Open a specific image file:
```bash
./target/release/imge /path/to/my_image.png
```

Run test suite:
```bash
cargo test
```

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
