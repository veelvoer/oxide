# Overview, scratchpads, HUD and smart tiling

## Overview (`Super+Tab`)

`Super+Tab` zooms out to a live map of every workspace on the screen. Each workspace is a tile with its windows
drawn live inside it.

* Click a window to go to it, click an empty tile to go to that workspace.
* Drag a window onto another tile to move it there. The last tile is always an empty workspace, so dragging a
  window onto it gives the window a workspace of its own.
* `Escape`, `Super+Tab` or a click outside the tiles zooms back in.

## Macro scratchpads

A scratchpad is a group of apps that you summon and dismiss together with one key, laid out side by side over
whatever you are doing. Apps that are not running yet are started; windows that are already open are taken into
the group.

```toml
[[scratchpad]]
name = "music"
width = 0.84      # share of the screen
height = 0.72
apps = [
  { app_id = "spotify", exec = "spotify" },
  { app_id = "btop",    exec = "kitty --class btop btop" },
]

[[bind]]
mods = "SUPER"
key = "m"
[bind.action]
type = "toggle_scratchpad"
arg = "music"
```

Press `Super+M` to bring the group in front of the current workspace (the rest is dimmed), press it again to send it
away. A click on the dimmed area dismisses it too. From a script: `oxide-ctl dispatch '{"type":"toggle_scratchpad","arg":"music"}'`.

## Zero-bar mode and the HUD

In **Settings → Bar** switch off **Always show the bar**. The bar disappears and windows get the whole screen.
Holding `Super` fades in a HUD at the screen edges: time, workspaces, Wi-Fi, volume, battery, the focused window and
the most useful shortcuts. It is gone the moment you let go, and it does not appear when Super is part of a
shortcut you are pressing. This is stored as `"hidden": true` (and `"hud": true`) in `~/.config/oxide/bar.json`.

## Windows that make room for what matters

With `[layout] context_aware = true` (the default, **Settings → Layout → Make room for what matters**) Oxide watches
what tiled windows are doing:

| Signal | How it is detected |
|---|---|
| building | a compiler or build tool (cargo, rustc, make, ninja, gcc, clang, go, npm, gradle, …) runs below the window |
| error | the last command failed: the fish hook in the default config reports it, any shell can run `oxide-ctl signal error --pid $PPID` |
| call | the window's app records from the microphone or camera (PipeWire) |
| audio | the window's app plays sound |

A window with an error or a call grows to `focus_weight` (0.62) of its row or column without touching your manual
sizes; once the signal is gone it shrinks back. Scripts can set a signal themselves:
`oxide-ctl signal busy|error|ok|call|clear --pid <pid>`.

## Borders that show what is going on

With `[decoration] telemetry = true` (**Settings → Windows → Borders show what the app is doing**) the focused
border is drawn by a shader that reacts to these signals: a slow pulse while something builds, a red glow after an
error, a green flash when a build succeeds, a soft wave during a call, and a warmer border when the CPU is busy.
Only windows with something to show are animated, so it costs nothing while idle. (Oxide renders with OpenGL ES,
so the shader is GLSL rather than WGSL.)

## Config errors never break the session

Oxide watches `oxide.toml`. When a saved file has a mistake, the previous working configuration stays active and a
small card shows the message and the line. **Open in editor** opens `$EDITOR` (in your terminal) at that line;
**Dismiss** hides the card. Fix the file and save: the card goes away and the change applies.
