# Linux desktop integration.
#
# Wayland compositors (Mutter/KWin/wlroots) ignore the per-window icon and
# instead resolve the taskbar/overview icon from the window's app_id
# (dev.wzed.editor) matching the .desktop file, whose Icon= points into the
# icon theme. So showing the icon on Wayland means installing the .desktop file
# plus a themed icon — not embedding a bitmap. X11 still uses the embedded
# WindowOptions.icon via _NET_WM_ICON.

PREFIX ?= $(HOME)/.local
DATADIR ?= $(PREFIX)/share
APP_ID := dev.wzed.editor
ICON_SIZES := 16 22 32 48 64 128 256

.PHONY: install uninstall build

install: build
	install -Dm755 target/release/wzed "$(DESTDIR)$(PREFIX)/bin/wzed"
	install -Dm644 dist/$(APP_ID).desktop "$(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop"
	@for size in $(ICON_SIZES); do \
		install -Dm644 dist/icons/$$size.png \
			"$(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png"; \
	done
	@echo "Installed to $(DESTDIR)$(PREFIX). Log out/in (or restart the shell) for the icon to appear."

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/wzed"
	rm -f "$(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop"
	@for size in $(ICON_SIZES); do \
		rm -f "$(DESTDIR)$(DATADIR)/icons/hicolor/$${size}x$${size}/apps/$(APP_ID).png"; \
	done

build:
	cargo build --release
