# Troubleshooting

Start with `oxide-ctl doctor` and the logs: `~/.cache/oxide/oxide.log` (compositor, `RUST_LOG=debug` for more) and
`~/.cache/oxide/shell.log` (shell).

| Problem | Try |
|---|---|
| Black screen / back to the login prompt right away | read `oxide.log`. Is `seatd` running or are you in a logind session? Pick another GPU with `OXIDE_DRM_DEVICE=/dev/dri/card1` |
| No bar, panels or wallpaper | Quickshell missing or too old (`quickshell --version`, needs 0.3+). `shell.log` shows QML errors. *Settings → Shell → Restart*, or `oxide-ctl restart-shell` |
| Shortcuts do nothing | the keyboard layout may differ: *Settings → Input → Layout*. Check the bind in *Shortcuts*. For `Super+<digit>` on AZERTY etc. nothing needs changing (position based) |
| Volume / brightness keys do nothing | install `wireplumber` (or `pactl`) and `brightnessctl`; check `oxide-ctl doctor`; your user needs permission for backlight (`video` group or the `brightnessctl` udev rule) |
| Wi-Fi / Bluetooth pages empty | NetworkManager (`nmcli`) / BlueZ (`bluetoothctl`) must be running |
| Screenshots fail | install `grim` and `slurp` |
| X11 app does not start | install `xwayland`; `xwayland = true` in `[general]` |
| Screen sharing or file pickers missing | install `xdg-desktop-portal` and a backend (gtk, kde or wlr) |
| Wrong colors after switching shells | *Settings → Theme*, pick a palette (an own shell uses *From wallpaper* by default) |
| The theme does not follow my wallpaper | *Theme → Palette → From wallpaper*. With your own shell the background layer must be visible (not covered by a fullscreen window) for a moment after the wallpaper changes |
| A setting reverted | with *Apply instantly* off, press **Save**. `oxide-ctl reload` re-reads the file, discarding unsaved changes |
| Locked out of the lock screen | switch to a text console (`Ctrl+Alt+F3`), log in, run `oxide-ctl unlock` |
| High memory use | the shell restarts itself above 600 MB; `OXIDE_SHELL_LIMIT_MB=400` lowers that. Look at `ps -o rss,cmd -C oxide,quickshell` |
| Config broke something | move `~/.config/oxide/oxide.toml` away; Oxide starts with defaults. *Settings → Config → Reset everything* does the same |

## Reporting a bug

Include: `oxide-ctl doctor`, the last lines of `oxide.log` and `shell.log`, your distro and GPU, and what you
did. If a visual glitch: a screenshot, and if you can the frames recorded with the test bed
(`tools/testbed`).
