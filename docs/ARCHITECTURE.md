# Architecture

```
oxide (compositor)  ←— JSON over a Unix socket —→  oxide-ctl, oxide-settings, the shell (Quickshell), oxide-ai
```

| Crate | Role |
|---|---|
| `oxide-compositor` | the `oxide` binary: smithay-based Wayland compositor. DRM/KMS + libinput + libseat backend for real sessions, winit backend for running nested. Layout, animations, rendering, input, XWayland, session lock, screencopy, the shell supervisor, the wallpaper-following theme. |
| `oxide-config` | the config schema (serde/TOML), defaults, palettes, theme/kitty/GTK writers, wallpaper → palette (`autotheme`), Hyprland importer, animation math shared with the settings preview |
| `oxide-ipc` | request/response types and the socket client |
| `oxide-layout` | pure tiling geometry (dwindle, master), unit tested |
| `oxide-settings` | the egui settings app; a separate process that only runs while open |
| `oxide-ctl` | the command-line client and helpers (`terminal`, `files`, `ipc`, `doctor`, …) |
| `oxide-ai` | the assistant: a small process speaking JSON lines, used by the shell |
| `contrib/shells/oxide` | the default shell (QML) |

## Memory budget

Target: well under 1 GB for the whole session, typically ~300 MB.

| Process | Typical (private memory / with shared libraries) |
|---|---|
| `oxide` | ~21 MB / ~73 MB |
| Quickshell with the default shell | ~83 MB / ~200 MB |
| helpers (`oxide-ai`, clipboard watcher) | ~3 MB each |

How it stays small: the compositor is single-threaded and draws only on damage (idle CPU is 0 %); wallpapers are decoded
at screen size and only the visible picture stays in memory (the old one is released after the cross-fade);
the clipboard history keeps images on disk; popups are unmapped while hidden. A watchdog restarts a shell whose
private memory exceeds 600 MB (`OXIDE_SHELL_LIMIT_MB`).

## Design decisions

* **Compositor and shell are separate processes.** A crash or hang in the shell never takes your windows with
  it, and you can replace the shell. Oxide restarts it and keeps a log.
* **One source of truth.** The config file; the settings app edits it live over IPC, the compositor applies it
  without restarting, and scripts use the same IPC.
* **The active monitor follows the pointer.** New windows, popups and toasts appear where you are looking.
* **Rules, not magic.** Per-window behaviour is explicit `[[rule]]` entries (the `Super+O` panel writes them).
* **Animations are evaluated per frame.** Each frame computes every running animation at the frame's own
  timestamp, so motion stays even regardless of refresh rate.
* **Nothing hard-coded to one machine.** Terminal, file manager, volume/brightness tools, keyboard layout and
  monitors are detected or have fallbacks; `oxide-ctl doctor` reports what is missing.
* **The theme is data.** `theme.json` is the contract between the compositor, the settings app, the default
  shell, your own shell, kitty and GTK.

## Protocols

xdg-shell, xdg-decoration (server side), layer-shell, session-lock, screencopy (wlr), data-device / primary
selection / wlr-data-control, dmabuf, viewporter, fractional-scale, cursor-shape, xdg-activation, idle-notify and
idle-inhibit, pointer-constraints, relative-pointer, tablet, and XWayland.

## Vendored smithay

`vendor/smithay` is smithay 0.7 with one patch: it tolerates a client committing after destroying a layer surface
(Quickshell does that when a popup closes). See the comment in the root `Cargo.toml`.
