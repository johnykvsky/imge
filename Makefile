# ==============================================================================
# imge - Fast Native Linux Image Viewer & Manipulation Tool
# Inspired by IrfanView
# ==============================================================================

export PATH := $(HOME)/.cargo/bin:$(PATH)

CARGO ?= cargo
PREFIX ?= $(HOME)/.local
BIN_DIR ?= $(PREFIX)/bin
APP_DIR ?= $(PREFIX)/share/applications
ICON_DIR ?= $(PREFIX)/share/icons/hicolor/128x128/apps
TARGET_DIR = target
RELEASE_BIN = $(TARGET_DIR)/release/imge
DEBUG_BIN = $(TARGET_DIR)/debug/imge

# Terminal color formatting
CYAN  := \033[0;36m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RESET := \033[0m

# Supported MIME types for file associations
MIME_TYPES := image/png image/jpeg image/webp image/bmp image/x-bmp image/gif image/tiff image/x-icon image/vnd.microsoft.icon
MIME_LIST  := image/png;image/jpeg;image/webp;image/bmp;image/x-bmp;image/gif;image/tiff;image/x-icon;image/vnd.microsoft.icon;

.PHONY: all help build release dev run check test bench clean distclean install install-desktop associate-mime uninstall

all: build

help:
	@echo -e "$(CYAN)imge - Fast Native Linux Image Manipulation Application$(RESET)"
	@echo ""
	@echo "Available targets:"
	@echo -e "  $(GREEN)make build$(RESET)           Build optimized release binary ($(RELEASE_BIN))"
	@echo -e "  $(GREEN)make run [FILE=img.png]$(RESET) Build and run imge (optional: FILE=/path/to/image)"
	@echo -e "  $(GREEN)make dev$(RESET)             Run debug build without arguments"
	@echo -e "  $(GREEN)make check$(RESET)           Fast compilation verification via cargo check"
	@echo -e "  $(GREEN)make test$(RESET)            Run test suite"
	@echo -e "  $(GREEN)make bench$(RESET)           Run automated benchmarks (startup, memory, engine)"
	@echo -e "  $(GREEN)make install$(RESET)         Install binary to $(BIN_DIR)/imge"
	@echo -e "  $(GREEN)make install-desktop$(RESET) Install desktop launcher, icon, and register file associations"
	@echo -e "  $(GREEN)make associate-mime$(RESET)  Set imge as default viewer for all supported image types"
	@echo -e "  $(GREEN)make uninstall$(RESET)       Remove binary, desktop entry, and icon"
	@echo -e "  $(GREEN)make clean$(RESET)           Clean cargo build artifacts"
	@echo -e "  $(GREEN)make distclean$(RESET)       Deep clean including target directory and temporary caches"

# Build optimized release binary
build: release

release:
	@echo -e "$(CYAN)Building optimized release binary...$(RESET)"
	$(CARGO) build --release
	@echo -e "$(GREEN)Release build ready: $(RELEASE_BIN)$(RESET)"

# Build and run with optional file parameter: make run FILE=/path/to/image.png
run: build
	./$(RELEASE_BIN) $(or $(FILE),$(ARGS))

# Dev mode (cargo run)
dev:
	$(CARGO) run -- $(or $(FILE),$(ARGS))

# Fast type check
check:
	$(CARGO) check

# Test suite
test:
	$(CARGO) test

# Automated benchmarks
bench: release
	@$(CARGO) build --release --bin bench_suite
	@python3 bench/run_benchmarks.py

# Clean cargo cache
clean:
	$(CARGO) clean

# Deep clean
distclean: clean
	rm -rf $(TARGET_DIR)
	rm -f *.rs.bk *~
	rm -rf .cache

# Install release binary only
install: release
	@mkdir -p $(BIN_DIR)
	@install -m 755 $(RELEASE_BIN) $(BIN_DIR)/imge
	@echo -e "$(GREEN)Installed imge binary to $(BIN_DIR)/imge$(RESET)"

# Setup desktop shortcut, application icon, and file associations
install-desktop: install
	@mkdir -p $(APP_DIR)
	@for size in 16x16 24x24 32x32 48x48 64x64 128x128 256x256; do \
		if [ -f assets/icons/$$size.png ]; then \
			mkdir -p $(PREFIX)/share/icons/hicolor/$$size/apps; \
			install -m 644 assets/icons/$$size.png $(PREFIX)/share/icons/hicolor/$$size/apps/imge.png; \
		fi \
	done
	@gtk-update-icon-cache -f -t $(PREFIX)/share/icons/hicolor 2>/dev/null || true
	@echo -e "$(GREEN)Installed application icons to $(PREFIX)/share/icons/hicolor/...$(RESET)"
	@printf "[Desktop Entry]\n\
Type=Application\n\
Name=imge\n\
GenericName=Image Viewer & Editor\n\
Comment=Fast, lightweight IrfanView-inspired image viewer and editor\n\
Exec=$(BIN_DIR)/imge %%F\n\
Icon=imge\n\
Terminal=false\n\
Categories=Graphics;Viewer;2DGraphics;\n\
MimeType=$(MIME_LIST)\n\
StartupNotify=true\n\
StartupWMClass=imge\n\
Keywords=image;viewer;crop;resize;convert;photo;\n" > $(APP_DIR)/imge.desktop
	@update-desktop-database $(APP_DIR) 2>/dev/null || true
	@echo -e "$(GREEN)Created desktop launcher: $(APP_DIR)/imge.desktop$(RESET)"
	@$(MAKE) associate-mime

# Associate all supported image MIME types with imge
associate-mime:
	@echo -e "$(CYAN)Associating supported image formats with imge...$(RESET)"
	@for mime in $(MIME_TYPES); do \
		xdg-mime default imge.desktop $$mime 2>/dev/null || true; \
		gio mime $$mime imge.desktop 2>/dev/null || true; \
	done
	@echo -e "$(GREEN)imge is now associated as default viewer for:$(RESET)"
	@echo -e "  $(YELLOW)PNG, JPEG, WebP, BMP, GIF, TIFF, and ICO$(RESET)"

# Uninstall all files
uninstall:
	@rm -f $(BIN_DIR)/imge
	@rm -f $(APP_DIR)/imge.desktop
	@for size in 16x16 24x24 32x32 48x48 64x64 128x128 256x256; do \
		rm -f $(PREFIX)/share/icons/hicolor/$$size/apps/imge.png; \
	done
	@gtk-update-icon-cache -f -t $(PREFIX)/share/icons/hicolor 2>/dev/null || true
	@update-desktop-database $(APP_DIR) 2>/dev/null || true
	@echo -e "$(GREEN)Removed imge binary, desktop entry, and icons.$(RESET)"
