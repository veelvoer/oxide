# Shortcuts

`Super` is the Windows key. Everything here is a default: change, remove or add shortcuts in
*Settings → Shortcuts* (click a shortcut, press the keys you want) or as `[[bind]]` entries in `oxide.toml`.
Shortcuts that use `Super`, and the media keys, keep working while a popup (launcher, quick settings, …) is open.

On layouts where the number row needs Shift (AZERTY, for example) `Super+1…9` still works: the digit shortcuts
follow the key's **position** on the keyboard.

## Programs

| Keys | Action |
|---|---|
| `Super+Q` | Terminal (the first of `$TERMINAL`, kitty, foot, alacritty, wezterm, ghostty, … that is installed) |
| `Super+E` | Files (a terminal file manager such as yazi or ranger when installed, else your desktop's) |
| `Super+R`, `Alt+Space`, `Super+D` | App launcher |
| `Super+,` | Settings |
| `Super+A` | Assistant (press again to close; while the chat is open but unfocused it takes the cursor back) |
| `Super+Z` | Quick settings |
| `Super+O` | Options for the focused window |
| `Super+Shift+V` | Clipboard history |
| `Super+N`, `Super+Shift+X` | Lock the screen |
| `Print` | Screenshot of an area (to the clipboard; needs `grim` and `slurp`) |

## Windows

| Keys | Action |
|---|---|
| `Super+C` | Close |
| `Super+V` | Float / tile |
| `Super+F` | Fullscreen |
| `Super+Arrows` or `Super+I/J/K/L` | Focus up / left / down / right |
| `Super+Ctrl+I/J/K/L` | Move the window |
| `Super+Space` | Switch dwindle ↔ master layout (for this workspace) |
| `Super+-`, `Super+=` | Shrink / grow the main area |
| `Super+Left drag` | Move (the window floats) |
| `Super+Right drag` | Resize |

## Workspaces and screens

| Keys | Action |
|---|---|
| `Super+1…9` | Go to workspace |
| `Super+Shift+1…9` | Send the window to a workspace |
| `Super+Shift+L` / `Super+Shift+J` | Next / previous workspace |
| `Super+Shift+Alt+L` / `Super+Shift+Alt+J` | Send the window to the next / previous workspace |
| `Super+scroll` | Next / previous workspace |
| `Alt+Tab` | Show / hide the scratchpad workspace |
| `Super+Shift+S` | Send the window to the scratchpad |
| `Super+Shift+Left/Right` | Focus the screen on the left / right |
| `Super+Ctrl+Left/Right` | Send the window to the screen on the left / right |

The screen under the mouse pointer is the active one: new windows open there.

## Keyboard helpers

`Alt+I/J/K/L` act as the arrow keys in every app (add `Shift` to select, `Ctrl` to jump by word with J/L).

## Media and hardware keys

Volume up/down/mute (PipeWire `wpctl`, falling back to PulseAudio `pactl`), microphone mute, brightness
up/down (`brightnessctl`), play/pause/next/previous (`playerctl`).

## Session

| Keys | Action |
|---|---|
| `Super+Shift+R` | Reload the config |
| `Super+M` | Quit Oxide |
| `Ctrl+Alt+F1…F12` | Switch virtual terminal |

## Writing your own

In `oxide.toml`:

```toml
[[bind]]
mods = "SUPER+SHIFT"        # SUPER, CTRL, ALT, SHIFT joined with +
key  = "b"                  # an xkb key name: b, Return, F5, XF86AudioMute …
[bind.action]
type = "exec"               # run a command
arg  = "firefox"
```

Actions: `exec` (command), `close`, `toggle_floating`, `toggle_fullscreen`, `focus_dir` / `move_dir` (`left`, `right`,
`up`, `down`), `workspace` (number), `move_to_workspace`, `workspace_rel`, `workspace_existing`,
`move_to_workspace_rel`, `toggle_special`, `move_to_special`, `focus_monitor`, `move_to_monitor`, `cycle_layout`,
`grow_master`, `shrink_master`, `reload_config`, `send_key` (press another key for the app, e.g. `alt+Left`), `quit`.
Mouse binds are `[[mouse_bind]]` with `button` (`left`, `right`, `middle`) and `action` (`move_window`,
`resize_window`).
