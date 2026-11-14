# Getting started

A five minute tour. Keys use `Super` (the Windows key).

## The desktop

* **Open a terminal** with `Super+Q`. Open another: the screen is split automatically (dwindle layout: each new
  window halves the one that has focus). `Super+Space` switches to the **master** layout (one big window, the
  rest stacked beside it).
* **Move focus** with `Super+Arrows` (or `Super+I/J/K/L`), **move the window** with `Super+Ctrl+I/J/K/L`, **resize**
  the main area with `Super+-` and `Super+=`. With the mouse: hold `Super` and drag with the left button to
  move a window (it floats), the right button to resize.
* **Close** a window with `Super+C`, **fullscreen** with `Super+F`, **float/tile** with `Super+V`.
* **Workspaces**: `Super+1…9` switch, `Super+Shift+1…9` send the window there. There is no limit and empty
  workspaces cost nothing. `Alt+Tab` shows a scratchpad; `Super+Shift+S` sends a window to it.

## Per-window options (`Super+O`)

Focus a window and press `Super+O` (or click the window title in the bar). A panel hangs from the window:

* **This window**: Float · Fullscreen · Close · Send to workspace 1–6. These act now.
* **Always, for this app**: *Open floating*, *Open on workspace*, *See-through*, *Blur behind*. Switch one on
  and every window of that app follows it from then on. *Undo* reverts your last change, *Forget these settings*
  clears them all. See [WINDOW-OPTIONS.md](WINDOW-OPTIONS.md).

## The shell

* **Bar** (top): launcher button, workspaces, window title, clock (click for the calendar), tray, quick settings
  (Wi-Fi, Bluetooth, sound, battery), notifications, assistant. Change what it shows and where in
  *Settings → Bar* by dragging modules around.
* **Launcher** (`Super+R`, `Alt+Space` or `Super+D`): type to search apps, do maths (`2+3*4`), run a command
  (start with `>`), or search the web. Arrows and Enter pick; Esc closes.
* **Quick settings** (`Super+Z`): Wi-Fi and Bluetooth (click the round icon to switch, the tile to see the
  list), Do not disturb, volume, brightness, notifications, Lock / Log out / Restart / Shut down.
* **Clipboard history** (`Super+Shift+V`): everything you copied, searchable; Enter pastes it back, Delete
  removes one.
* **Lock** (`Super+N`): your login password unlocks it.
* **Assistant** (`Super+A`): an AI that can operate the computer, with you in control. See [ASSISTANT.md](ASSISTANT.md).

## Settings (`Super+,`)

Everything you see can be changed here, and changes apply instantly. The sidebar has System (Wi-Fi, Bluetooth,
Sound, Displays, Power), Look (Theme, Windows, Animations), Shell (Bar, Launcher, Assistant), Desktop (Layout,
Input, Shortcuts, Terminal) and Config. Turn on **Advanced** at the bottom of the sidebar for the rest: Shells,
Apps (startup commands, window rules, environment variables) and **Own configs**.

## Make it yours in five steps

1. *Theme*: pick a palette, or **From wallpaper**; choose dark or light; set the roundness.
2. *Theme → Background*: pick one of the bundled photos, or your own picture.
3. *Bar*: drag modules, choose Flat / Island / Floating, top or bottom.
4. *Shortcuts*: click a shortcut to change it (press the keys you want), or add a new one.
5. *Animations*: choose a feel (Springy, Smooth, Snappy, Bouncy), or switch animations off.

## Coming from Hyprland

`oxide-ctl import-hyprland --write` (or *Settings → Config → Import*) converts your `hyprland.conf`: key and mouse binds,
window rules, gaps, border size, rounding, opacity, layout, input, environment variables and monitors
(animations are imported as on/off only). Lines Oxide does not support are listed so you can see what was skipped.
