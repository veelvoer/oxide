# Oxide: build and install.
#   make                      build (release)
#   sudo make install         install system wide (PREFIX=/usr/local by default)
#   sudo make install PREFIX=/usr
#   make install DESTDIR=pkg  stage into a folder (for packagers)
#   make uninstall
# A user-only install (no root) is contrib/install.sh.
PREFIX  ?= /usr/local
DESTDIR ?=
CARGO   ?= cargo
TARGET  ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)/release
BINS    := oxide oxide-ctl oxide-settings oxide-ai oxide-remoted

all: build

build:
	$(CARGO) build --release --locked

# install only copies: under sudo, root usually has no Rust toolchain, so build first with plain `make`
install:
	@for b in $(BINS); do [ -x $(TARGET)/$$b ] || { echo "$(TARGET)/$$b is missing: run 'make' first (without sudo)"; exit 1; }; done
	install -d $(DESTDIR)$(PREFIX)/bin
	for b in $(BINS); do install -m755 $(TARGET)/$$b $(DESTDIR)$(PREFIX)/bin/$$b; done
	install -m755 contrib/oxide-session $(DESTDIR)$(PREFIX)/bin/oxide-session
	install -d $(DESTDIR)$(PREFIX)/share/oxide/shells $(DESTDIR)$(PREFIX)/share/oxide/wallpapers
	cp -r contrib/shells/oxide $(DESTDIR)$(PREFIX)/share/oxide/shells/
	install -d $(DESTDIR)$(PREFIX)/share/oxide/qml
	cp -r contrib/qml/Oxide $(DESTDIR)$(PREFIX)/share/oxide/qml/
	cp -r contrib/wallpapers/. $(DESTDIR)$(PREFIX)/share/oxide/wallpapers/
	install -Dm644 contrib/oxide.desktop $(DESTDIR)$(PREFIX)/share/wayland-sessions/oxide.desktop
	install -Dm644 contrib/oxide-settings.desktop $(DESTDIR)$(PREFIX)/share/applications/oxide-settings.desktop
	install -Dm644 contrib/branding/oxide-logo.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/oxide-settings.svg
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/oxide/LICENSE

uninstall:
	for b in $(BINS) oxide-session; do rm -f $(DESTDIR)$(PREFIX)/bin/$$b; done
	rm -rf $(DESTDIR)$(PREFIX)/share/oxide $(DESTDIR)$(PREFIX)/share/licenses/oxide
	rm -f $(DESTDIR)$(PREFIX)/share/wayland-sessions/oxide.desktop $(DESTDIR)$(PREFIX)/share/applications/oxide-settings.desktop
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/oxide-settings.svg

test:
	$(CARGO) test --release

.PHONY: all build install uninstall test
