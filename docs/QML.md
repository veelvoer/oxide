# `import Oxide`

Oxide ships its own Quickshell module, the counterpart of `Quickshell.Hyprland`. Any Quickshell config running under Oxide can use it:

```qml
import Quickshell
import Oxide

ShellRoot {
    Text { text: "Workspace " + OxideState.active + " of " + OxideState.workspaces.length }
}
```

* `OxideState`: live state pushed by the compositor (`active`, `workspaces`, `windows`, `focused`, `outputs`, `layout`, `profile`,
  `onAc`, `activeScreen`) and `dispatch({type: "workspace", arg: 3})` to make Oxide do something.
* `Sys`: system numbers (cpu, memory, battery, network).

The module lives in `~/.local/share/oxide/qml/Oxide` (`/usr/share/oxide/qml` for packages). The compositor puts these folders on
`QML_IMPORT_PATH` for the shell it starts; to try it in a shell you start by hand, export the same variable.

## Wallpaper

The compositor draws the wallpaper itself; the bundled shell does not. `~/.config/oxide/wallpaper.json`:

```json
{ "mode": "photo" | "theme" | "image" | "color" | "slideshow",
  "photo": "dolomites", "path": "/home/me/x.jpg", "color": "#1b1917", "fit": "fill" | "fit" | "center",
  "folder": "/home/me/Pictures/walls", "interval": 600, "shuffle": false }
```

Changing the file cross-fades to the new picture. Settings > Theme > Background writes it.
