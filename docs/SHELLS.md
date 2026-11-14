# Own shells

The "shell" is everything you see besides windows: bar, launcher, panels, notifications, lock screen, wallpaper.
Oxide ships one ([Quickshell](https://quickshell.org)-based, in `contrib/shells/oxide`) and runs whichever
Quickshell config you choose.

## Use your own

1. *Settings → Advanced → Shells*: drop a folder or `.zip` on the window, paste a git/download link, or press
   **Import** to take your existing `~/.config/quickshell`. Any folder with a `shell.qml` works. Add a `shell.json`
   (`{"name": "...", "author": "...", "description": "..."}`) and a `preview.png` for a nicer list entry.
2. Press **Use**. Oxide restarts the shell. (This sets `own.shell = true` and `shell.active = "<folder>"`.)
3. Because your shell draws its own wallpaper, the palette switches to **From wallpaper**, so the settings app,
   your terminal and GTK apps follow your wallpaper automatically. Choose a fixed palette in *Settings → Theme*
   any time.

The pages that configure the *bundled* bar and launcher (*Bar*, *Launcher*, and the wallpaper chooser in *Theme*)
are hidden while your own shell runs; they would not do anything.

Shells live in `~/.config/oxide/shells/<name>`. `~/.config/oxide/active-shell` is a symlink to the one in use, so
`quickshell -p ~/.config/oxide/active-shell ipc call …` always reaches it. Logs: `~/.cache/oxide/shell.log`.
The shell is restarted if it crashes (up to five quick crashes in a row) and when it grows beyond 600 MB
(`OXIDE_SHELL_LIMIT_MB` changes the limit).

Editing the default shell: copy it (it is in `~/.config/oxide/shells/oxide`; it is refreshed from the installed
copy when a new version arrives, and your old one is kept in `shell-backups`) or choose it in *Own configs →
Shell files* to edit single files in the built-in editor.

## What a shell can use from Oxide

### Live state

Connect to `$XDG_RUNTIME_DIR/oxide.sock` (override with `OXIDE_SOCKET`) and send `{"cmd":"subscribe"}`. You get
a `state` object now and after every change: `active_workspace`, `workspaces[]`, `layout`, `focused`
(`app_id`, `title`), `windows[]` (app_id, title, workspace, floating, fullscreen, focused, geometry, and the
`rule` stored for the app) and `outputs[]` (name, size, position, scale, refresh, the `workspace` shown there and
whether it is `focused`). `contrib/shells/oxide/OxideState.qml` is a ready-made Quickshell singleton.

While a popup of your shell holds the keyboard, the window it was opened over still counts as `focused`.

### Actions

`oxide-ctl dispatch '{"type":"workspace","arg":3}'` (see [IPC.md](IPC.md) for all actions), or the shortcuts
`oxide-ctl terminal`, `oxide-ctl files`, `oxide-ctl lock`, `oxide-ctl launcher`.

### Calling your shell from key binds

`oxide-ctl ipc <target> <function> [args]` calls an `IpcHandler` of the active shell:

```qml
IpcHandler { target: "shell"; function toggleLauncher(): void { launcher.visible = !launcher.visible } }
```
```toml
[[bind]]
mods = "SUPER"
key = "d"
[bind.action]
type = "exec"
arg = "oxide-ctl ipc shell toggleLauncher"
```

The default binds call `shell toggleControl | toggleCalendar | toggleLauncher | toggleClipboard | toggleWindow |
toggleNotifications | lock` and `ai toggle`; a shell that provides these handlers works with the default keys.

### Theme

`~/.config/oxide/theme.json` (watch it with a Quickshell `FileView { watchChanges: true }`) holds the colors
(see [THEMING.md](THEMING.md)). `contrib/shells/oxide/Theme.qml` is a singleton that loads it and exposes
`Theme.bg`, `Theme.surface`, `Theme.raised`, `Theme.line`, `Theme.text`, `Theme.muted`, `Theme.accent`,
`Theme.ink`, `Theme.good`, `Theme.warn`, `Theme.bad`, `Theme.roundness`, `Theme.light`; copy it into your shell and
your colors follow the theme, including the palette that follows your wallpaper.

### Other pieces you can copy

* `AIClient.qml` + `AIPanel.qml`: the assistant ([ASSISTANT.md](ASSISTANT.md)).
* `Pop.qml`: the popup base (overlay layer, click-outside to close, grows out of its corner, follows the active
  screen via `OxideState.activeScreen`).
* `Wallpaper.qml`, `LockScreen.qml`, `Notifications.qml`, `Clipboard.qml`: complete, self-contained components.
* Notifications: the default shell is the notification server (Quickshell's `NotificationServer`); your shell can
  be too.

## The default shell's files

`shell.qml` (entry), `Bar.qml` and `Mod*.qml` (bar modules, configured by `~/.config/oxide/bar.json`),
`Launcher.qml`, `ControlCenter.qml`, `Calendar.qml`, `NotificationCenter.qml`, `Notifications.qml`,
`Clipboard.qml`, `WindowOptions.qml`, `LockScreen.qml`, `Wallpaper.qml`, `AIPanel.qml`, plus the small shared
parts `Theme.qml`, `OxideState.qml`, `Pop.qml`, `Chip.qml`, `Toggle.qml`, `PillSlider.qml`, `Icon.qml`, `Txt.qml`.

### bar.json

```json
{ "position": "top", "style": "flat", "height": 36, "margin": 6, "radius": 8, "opacity": 0.96,
  "left":   [ {"id": "launcher"}, {"id": "workspaces", "style": "numbers"}, {"id": "window"} ],
  "center": [ {"id": "clock", "seconds": false} ],
  "right":  [ {"id": "tray"}, {"id": "status"}, {"id": "bell"}, {"id": "assistant"} ] }
```

`style` is `flat`, `island` or `floating`. Every module is a file named `Mod<Id>.qml`; add your own by dropping a
file in the shell folder and putting its id in a lane. Edit it visually in *Settings → Bar*.
