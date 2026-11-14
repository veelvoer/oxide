# IPC and scripting

Oxide listens on a Unix socket, `$XDG_RUNTIME_DIR/oxide.sock` (override with `OXIDE_SOCKET`). The protocol is
newline-delimited JSON: send one request, read one response (`subscribe` keeps the connection open and pushes a
`state` response after every change).

## oxide-ctl

| Command | Does |
|---|---|
| `oxide-ctl state` | windows, workspaces, screens and focus as JSON |
| `oxide-ctl subscribe` | prints a state line after every change |
| `oxide-ctl get-config` / `set <file.toml>` / `reload` | read the live config, replace it (not saved), re-read the file |
| `oxide-ctl dispatch '<action json>'` | run an action, e.g. `'{"type":"workspace","arg":3}'`, `'{"type":"toggle_floating"}'` |
| `oxide-ctl workspace <n>`, `exec <cmd>` | shortcuts for two common actions |
| `oxide-ctl rule <app> floating=true workspace=2 opacity=0.9 blur=true` | set window rules (`key=-` removes one) |
| `oxide-ctl terminal [cmd…]` | open an installed terminal (optionally running a command) |
| `oxide-ctl files` | open a file manager |
| `oxide-ctl launcher` / `lock` | toggle the launcher, lock the screen (with fallbacks when no shell runs) |
| `oxide-ctl ipc <target> <fn> [args]` | call a function of the active shell |
| `oxide-ctl theme list` / `theme apply <palette>` | list palettes, apply one |
| `oxide-ctl clip …` | clipboard history helpers used by the shell |
| `oxide-ctl restart-shell` | restart the shell |
| `oxide-ctl import-hyprland [file] [--write]` | convert a Hyprland config |
| `oxide-ctl input move X Y \| click X Y [button] \| dclick X Y \| scroll DX DY \| key NAME [mods…] \| type TEXT` | synthesize input (used by automation and the assistant) |
| `oxide-ctl unlock` | emergency: drop a screen lock whose locker is gone (also from another TTY) |
| `oxide-ctl default-config` | print the complete default config |
| `oxide-ctl doctor` | check the system for missing tools |
| `oxide-ctl quit` | quit Oxide |

## Requests

`{"cmd": "<name>", …}`, names in snake_case:

| cmd | fields | response |
|---|---|---|
| `get_config` | | `{"status":"config","config":{…}}` |
| `set_config` | `config` | `{"status":"ok"}` (live only; the settings app saves) |
| `reload` | | ok |
| `dispatch` | `action` | ok |
| `get_state` | | `{"status":"state","state":{…}}` |
| `subscribe` | | a `state` response now and on every change |
| `input` | `input` (`{"kind":"click","x":10,"y":20,"button":"left"}`, `move`, `button`, `dclick`, `scroll`, `key`, `type`) | ok |
| `restart_shell` | | ok |
| `set_rule` | `app_id`, `rule` (`floating`, `workspace`, `opacity`, `blur`), `clear` (field names) | ok; saved and applied |
| `capture_key`, `take_capture`, `cancel_capture` | | the shortcut recorder of the settings app |
| `record` | `dir`, `frames` | test aid: saves rendered frames (see CONTRIBUTING) |
| `force_unlock` | | emergency unlock |

Errors come back as `{"status":"error","message":"…"}`.

### State

```json
{ "active_workspace": 1, "layout": "dwindle",
  "focused": {"app_id": "kitty", "title": "~"},
  "workspaces": [{"id": 1, "windows": 2, "visible": true, "special": false}],
  "windows": [{"app_id": "kitty", "title": "~", "workspace": 1, "floating": false, "fullscreen": false,
               "focused": true, "geometry": [4, 40, 950, 1000], "rule": null}],
  "outputs": [{"name": "DP-1", "width": 1920, "height": 1080, "x": 0, "y": 0, "scale": 1.0, "refresh": 144.0,
               "workspace": 1, "focused": true}] }
```

## Actions

`{"type": "<name>", "arg": <value>}`: `exec` (string), `close`, `toggle_floating`, `toggle_fullscreen`, `focus_dir` /
`move_dir` / `focus_monitor` / `move_to_monitor` (`left|right|up|down`), `workspace` / `move_to_workspace` (number),
`workspace_rel` / `workspace_existing` / `move_to_workspace_rel` (±1), `toggle_special`, `move_to_special`,
`cycle_layout`, `grow_master`, `shrink_master`, `reload_config`, `send_key` (string), `quit`.

## Examples

```sh
# a bar widget: current workspace number
oxide-ctl state | jq .state.active_workspace

# react to changes
oxide-ctl subscribe | jq --unbuffered -r '.state.focused.title // ""'

# make Firefox always open on workspace 2
oxide-ctl rule firefox workspace=2
```
