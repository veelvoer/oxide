#!/bin/sh
# Build in release mode and install:
#   binaries      -> ~/.local/bin
#   shells        -> ~/.local/share/oxide/shells (bundled) and ~/.config/oxide/shells (yours, never overwritten)
set -e
cd "$(dirname "$0")/.."
cargo build --release
T="${CARGO_TARGET_DIR:-target}/release"
mkdir -p "$HOME/.local/share/oxide/wallpapers" "$HOME/.local/bin" "$HOME/.local/share/oxide/shells" "$HOME/.config/oxide/shells"
install -m755 "$T/oxide" "$T/oxide-ctl" "$T/oxide-settings" "$T/oxide-ai" contrib/oxide-session "$HOME/.local/bin/"
rm -rf "$HOME/.local/share/oxide/shells/oxide"
cp -r contrib/shells/oxide "$HOME/.local/share/oxide/shells/oxide"
cp -r contrib/wallpapers/. "$HOME/.local/share/oxide/wallpapers/"
# first install: give the user an editable copy; later runs leave their copy alone
[ -d "$HOME/.config/oxide/shells/oxide" ] || cp -r contrib/shells/oxide "$HOME/.config/oxide/shells/oxide"
echo "Installed to ~/.local/bin (make sure it is in PATH)."
echo "The default shell is copied to ~/.config/oxide/shells/oxide and refreshed automatically when it changes"
echo "(the old copy is kept in ~/.config/oxide/shell-backups). To refresh it by hand:"
echo "    rm -rf ~/.config/oxide/shells/oxide && cp -r ~/.local/share/oxide/shells/oxide ~/.config/oxide/shells/oxide"
echo "To get a login-manager entry:  sudo install -Dm644 contrib/oxide.desktop /usr/share/wayland-sessions/oxide.desktop"
echo "Or from a TTY just run:        oxide-session"
