# Configuration reference

The file is `~/.config/oxide/oxide.toml` (`$XDG_CONFIG_HOME/oxide/oxide.toml`). Every key is optional: what you leave
out keeps its default. `oxide-ctl default-config` prints the complete default; [oxide.default.toml](oxide.default.toml)
is a copy. Edit by hand and run `oxide-ctl reload` (or `Super+Shift+R`), or use *Settings* (changes apply instantly
and are saved). The settings app and the file are two views of the same data.

## `[general]`

| Key | Default | |
|---|---|---|
| `gaps_in` / `gaps_out` | 7 / 3 | pixels between windows / around the screen edge |
| `border_size` | 2 | window border thickness |
| `active_border` | 3-color gradient | colors (`#rrggbb[aa]`) of the focused border; one color = solid |
| `border_angle` | 45 | gradient direction in degrees |
| `inactive_border` | `#4c566aaa` | border of other windows |
| `layout` | `dwindle` | `dwindle` or `master` (per workspace with `Super+Space`) |
| `focus_follows_mouse` | true | |
| `cursor_theme` / `cursor_size` | `default` / 24 | |
| `float_size` | 0.6 | default size of a floating window, as a fraction of the screen |
| `xwayland` | true | start XWayland for X11 apps |
| `scroll_workspaces` | true | `Super+scroll` switches workspaces |

## `[decoration]`

`rounding` (12), `active_opacity` (1.0), `inactive_opacity` (0.94), `dim_inactive` (0.0);
`[decoration.blur]` `enabled`, `size`, `passes`; `[decoration.shadow]` `enabled`, `range`, `color`.

## `[input]`

| Key | Default | |
|---|---|---|
| `kb_layout`, `kb_variant`, `kb_options` | detected from your system (`XKB_DEFAULT_LAYOUT`, `/etc/default/keyboard`, X11 config, console keymap), else `us` | xkb names, e.g. `kb_layout = "us,de"`, `kb_options = "grp:alt_shift_toggle"` |
| `repeat_rate` / `repeat_delay` | 25 / 600 | |
| `natural_scroll` | true | |
| `tap_to_click`, `disable_while_typing` | true | touchpads |
| `sensitivity` | 0.0 | −1.0 … 1.0 |
| `accel_profile` | `flat` | `flat` or `adaptive` |
| `scroll_factor` | 1.0 | |
| `numlock_on_start` | false | |

## `[layout]`

`master_factor` (share of the screen for the master area), `split_ratio` (dwindle split for new windows).

## `[[monitor]]`

By default every screen is used as it comes (preferred mode, placed to the right of the previous one). Add an
entry to change one:

```toml
[[monitor]]
name = "DP-1"        # as listed by `oxide-ctl state`
width = 2560
height = 1440
refresh = 144.0      # optional
x = 0                # optional position; omit to place automatically
y = 0
scale = 1.25
```

Settings kept for a screen that is unplugged apply again when it returns.

## `[theme]`

`preset` (`oxide`, `nord`, `gruvbox`, `catppuccin`, `tokyo`, `everforest`, `rose-pine`, or `auto` = from wallpaper),
`mode` (`dark`/`light`), `accent` (`#rrggbb` to override), `roundness` (0–20), `kitty` (terminal follows theme),
`apps` (recolor GTK/Qt). See [THEMING.md](THEMING.md).

## `[shell]`, `[own]`, `[terminal]`

```toml
[shell]
enabled = true            # run a shell at all
active = "oxide"          # a folder of ~/.config/oxide/shells (used when own.shell = true)

[own]                     # use your own files instead of Oxide's standard setup
shell = false             # your shell instead of the bundled one
kitty = false             # your ~/.config/kitty/kitty.conf
fish = false              # your ~/.config/fish/config.fish
gtk = false               # your gtk.css / settings.ini

[terminal]                # the standard kitty (ignored while own.kitty = true)
font = "monospace"; font_size = 12.0; opacity = 0.95; padding = 10
cursor = "beam"           # beam | block | underline
scrollback = 10000; greeting = false
```

## `[animations]`

See [ANIMATIONS.md](ANIMATIONS.md).

## `[[bind]]`, `[[mouse_bind]]`

See [SHORTCUTS.md](SHORTCUTS.md). When a new version adds a shell shortcut (such as `Super+O`) or an `Alt+I/J/K/L` key remap, it is added to an
existing file automatically, unless the key is already used by one of yours. Other defaults only apply to new files.

## `[[rule]]`

Window rules; see [WINDOW-OPTIONS.md](WINDOW-OPTIONS.md).

## `exec_once` and `env`

```toml
exec_once = ["nm-applet --indicator", "swayidle -w"]   # run every time Oxide starts
env = [["GTK_THEME", "Adwaita:dark"], ["QT_QPA_PLATFORMTHEME", "qt6ct"]]   # for every app Oxide starts
```

## Other files in `~/.config/oxide/`

| File | |
|---|---|
| `wallpaper.json` | wallpaper of the default shell |
| `bar.json` | bar layout of the default shell |
| `launcher.json`, `launcher-history.json` | launcher options and most-used apps |
| `theme.json`, `kitty-theme.conf`, `auto-theme.json` | generated from `[theme]`; do not edit |
| `ai.toml`, `ai-chat.json`, `ai-memory.json` | the assistant |
| `ui.toml` | settings app state (Advanced on/off) |
| `shells/`, `active-shell`, `shell-backups/` | shells |
