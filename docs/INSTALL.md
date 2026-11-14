# Install

Oxide needs a Linux system with Wayland-capable graphics (any GPU with a Mesa/NVIDIA GBM driver), `seatd` or
`systemd-logind`, and [Quickshell](https://quickshell.org) for the bar and panels. `oxide-ctl doctor` checks all
of this once Oxide is installed.

## 1. Dependencies

### Runtime

| What | For | Package names |
|---|---|---|
| **Quickshell** (0.3 or newer) | bar, launcher, quick settings, notifications, lock screen | Arch: `quickshell` · Fedora: `quickshell` (COPR `errornointernet/quickshell`) · others: build from source, see quickshell.org |
| a **terminal** | `Super+Q` | kitty (best integrated), foot, alacritty, wezterm, ghostty, gnome-terminal, konsole, xterm |
| `xwayland` | X11 apps | `xwayland` / `xorg-x11-server-Xwayland` / `xorg-xwayland` |
| `grim`, `slurp` | screenshots (`Print`) | same names everywhere |
| `wl-clipboard` | clipboard history | `wl-clipboard` |
| PipeWire + WirePlumber (`wpctl`) or PulseAudio (`pactl`) | volume keys, Sound page | `pipewire wireplumber` |
| `brightnessctl` | brightness keys, Displays page | `brightnessctl` |
| `playerctl` | media keys | `playerctl` |
| NetworkManager (`nmcli`) | Wi-Fi page and tile | `network-manager` / `NetworkManager` / `networkmanager` |
| BlueZ (`bluetoothctl`) | Bluetooth page and tile | `bluez` |
| a polkit agent | apps asking for admin rights | `polkit-gnome`, `lxqt-policykit`, `polkit-kde-agent`, … |
| `xdg-desktop-portal` + a backend | screen sharing, file pickers | `xdg-desktop-portal xdg-desktop-portal-gtk` |

Everything except Quickshell and a terminal is optional: a missing tool only switches off the feature it serves.

### Build

Rust (stable, 1.75+) and the development files of: wayland, xkbcommon, libinput, libseat, gbm, udev, drm,
libdisplay-info, EGL/GLES, fontconfig.

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libwayland-dev libxkbcommon-dev libinput-dev libseat-dev \
  libgbm-dev libudev-dev libdrm-dev libdisplay-info-dev libegl-dev libgles-dev libfontconfig-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config wayland-devel libxkbcommon-devel libinput-devel libseat-devel \
  mesa-libgbm-devel systemd-devel libdrm-devel libdisplay-info-devel mesa-libEGL-devel mesa-libGLES-devel fontconfig-devel

# Arch
sudo pacman -S base-devel wayland libxkbcommon libinput seatd mesa libdrm libdisplay-info fontconfig

# openSUSE
sudo zypper install gcc pkg-config wayland-devel libxkbcommon-devel libinput-devel libseat-devel \
  libgbm-devel libudev-devel libdrm-devel libdisplay-info-devel Mesa-libEGL-devel Mesa-libGLESv2-devel fontconfig-devel
```

The names above are the usual ones; if your distro calls one differently, `pkg-config --list-all` finds it.
(NixOS: use a `nix-shell` with the same libraries; there is no ready-made flake yet.)

## 2. Build and install

```sh
git clone https://github.com/veelvoer/oxide
cd oxide
```

**For your user only** (no root, installs to `~/.local`):

```sh
contrib/install.sh
```

Make sure `~/.local/bin` is in your `PATH`.

**System wide** (needed for login-manager entries to work for everyone):

```sh
make
sudo make install                 # to /usr/local; add PREFIX=/usr for a distro-style layout
```

`make install DESTDIR=...` stages the files for packagers. `make uninstall` removes them.

What gets installed:

| | user install | system install |
|---|---|---|
| programs `oxide oxide-ctl oxide-settings oxide-ai oxide-session` | `~/.local/bin` | `$PREFIX/bin` |
| the default shell | `~/.local/share/oxide/shells/oxide` | `$PREFIX/share/oxide/shells/oxide` |
| wallpapers | `~/.local/share/oxide/wallpapers` | `$PREFIX/share/oxide/wallpapers` |
| login entry | (see below) | `$PREFIX/share/wayland-sessions/oxide.desktop` |

Your own files always live in `~/.config/oxide/`: the config (`oxide.toml`), your shells (`shells/`), the
wallpaper choice (`wallpaper.json`). Updating Oxide never overwrites them.

## 3. Start it

* **From a login manager** (SDDM, GDM, greetd, …): choose **Oxide** in the session list. For a user install, copy the
  entry once: `sudo install -Dm644 contrib/oxide.desktop /usr/share/wayland-sessions/oxide.desktop` (it runs
  `oxide-session`, so that must be on the system `PATH`, or edit `Exec=` to the full path).
* **From a TTY**: log in on a text console and run `oxide-session`.
* **Nested inside another desktop** (to try it safely): `oxide --winit`.

Oxide needs permission to open your GPU and input devices. With `systemd-logind` (nearly every desktop distro)
this just works. Without it, run `seatd` and add your user to the `seat` group.

Logs: `~/.cache/oxide/oxide.log` (compositor) and `~/.cache/oxide/shell.log` (shell).

## 4. Check it

```sh
oxide-ctl doctor
```

lists every optional tool, whether it is installed and the exact package command for your distro.

## Updating

`git pull && contrib/install.sh` (or `make && sudo make install`). The default shell in `~/.config/oxide/shells/oxide`
is refreshed automatically when it changes; your previous copy is kept in `~/.config/oxide/shell-backups`.
