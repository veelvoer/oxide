# Changelog

## 0.2.0 (unreleased)

* Updates: half a minute after login Oxide looks for a new version and, when there is one, shows a card with an Update button (and Later). Automatic installing is now off by default (Settings > Config > Updates switches it on). Builds with local changes are not offered an update.
* Fixed (Unity and other X11 programs): the pointer no longer vanishes over their dropdowns (the X11 default cursor was empty); floating X11 windows can resize themselves, so windows that open small and grow (Unity's editor windows) show their content.
* Phone app: the quick actions are a fixed grid of four in three groups (Windows, Media, PC): nothing scrolls sideways or is cut off, Unlock is the accent tile.
* Fixed (phone): "Oxide Remote is not responding": the watch service closed its Bluetooth socket on the main thread. Updates install again (0.7.0 had a different signing key).
* Unlock the PC from the phone: an "Unlock" tile in the app asks for the phone's fingerprint, face, PIN or pattern, then unlocks the
  lock screen. Off by default: Settings > Phone > "Let my phone unlock this PC".
* Oxide's own wallpaper engine: the compositor draws the wallpaper itself (photo, picture, color, theme, and a new slideshow of
  a folder with an interval and shuffle), cross-fading between pictures, scaled once to each screen. The shell no longer opens
  a wallpaper window of its own (other shells and compositors still get the old one).
* Oxide's own QML module: `import Oxide` gives any Quickshell config `OxideState` (workspaces, windows, focus, outputs, profile,
  dispatch) and `Sys`, like Quickshell.Hyprland does for Hyprland. Installed to ~/.local/share/oxide/qml; the shell finds it
  through QML_IMPORT_PATH. See docs/QML.md.
* Settings: every row has the same height, icon column and control height; sliders, switches, fields and choices line up on
  one edge, and corners use one radius.
* Phone: icons are centered by eye (each glyph's ink, not its box); the settings page asks Android for permissions once a second
  instead of every frame (the app could say it was not responding).
* Fixed (watch): the app "did not open": opened again from the launcher it showed the PC it was left on, and after 6 s the dark
  pointer screen. Leaving the app with a lit screen now ends the pointer, and opening it shows the first screen. Java could not
  reach the app's native methods (NativeActivity opens the library on its own): the library is loaded the usual way too.
* Watch: the PC state dot sits in front of its words, not on the icon.

* Fixed: Steam (and every other X11 program started from the shell or the launcher) said "Unable to open a connection to X":
  the shell and the autostart programs were started before XWayland was ready, so they had no DISPLAY. They now wait for
  XWayland (3 s at most), and D-Bus-activated programs get DISPLAY too.

* Watch: the bezel and the back button really work now (winit dropped the bezel's scroll events and swallowed the back key:
  a patched winit in `vendor/winit-wear` forwards them), with ticks under the finger and a scroll arc on the rim like Hearth.
* Watch: with the gyro mouse the screen stays on but black (keep-screen-on, brightness 0) so battery saver cannot freeze it.

* The workspace highlight has two settings again: *delay* (ms before it starts, 0 = at once) and *speed* (ms it takes, 0 = jumps).
* Watch: a proper Wear OS app: the touch bezel scrolls lists (volume on the controls page, the mouse wheel on the touchpad
  page), the back button goes back and closes the app from the first screen, the system's swipe-to-dismiss is off so the app
  keeps its place (a swipe from the left edge goes back inside the app).
* Watch: the gyro pointer is smoother and less sensitive (filtered angles, slow = precise, fast = far, no tremor), and goes on
  with the screen off in battery saver (wake-up sensors; Setup > Cursor > Screen off asks Android to allow it).

* New: **presets** (Settings > Presets, `oxide-ctl preset`): every setting under a name, with an automatic undo; and **GitHub
  sign-in** (device flow or the GitHub CLI) to keep presets and setup in a private gist. The assistant's API key never
  goes into a preset or to GitHub. See docs/PRESETS.md.
* New: the bar's workspace styles of the old quickshell (numbers, dots, buttons, bars, scroll, stretch). *Highlight delay* is
  in milliseconds: 0 follows at once.
* New: Oxide's default fish prompt is the pill prompt (folder, git branch, time) in the theme's colors, also in terminals
  that are already open (Settings > Terminal > Oxide's prompt).
* Fixed: after unlocking, the shell was killed by a protocol error (the lock surface was unmapped with a NULL buffer) and the
  bar came back only after the restart.
* Fixed: settings that were lost when another change (control center profile, assistant, preset) was saved at the same time:
  Settings now merges with the file and takes over outside changes; writes no longer share one temporary file.
* Fixed: kitty, fish and GTK did not follow a theme changed by anything but Settings (the assistant, a preset, the file).
* Watch: pointer filtered (One Euro) and chased by the phone, pinch clicks go back to where the pointer was 90 ms earlier, an
  adaptive pinch detector with a live strength meter, taps on the touchpad and the back swipe work on a touch screen.
* Phone and watch apps log panics to `crash.log` and keep running when one frame fails.

* Watch: the gyro mouse points like a finger and thumb at the screen and follows your hand, a pinch clicks, and it keeps
  working with the screen off while the app is open (foreground service; ends when the PC is left or the app closed).
* Watch: icons are aligned for the eye (each icon centered by its ink and mass, measured; `scripts/gen_icon_metrics.py`).

* New: natural scrolling is set apart for the **touchpad and the mouse** (`mouse_natural_scroll`).
* New: **swipe between workspaces** with a set number of fingers on the touchpad (2, 3 or 4; Settings > Input).
* New: the bar's workspace highlight has a **delay and a move time** (Settings > Bar > Workspaces).
* Watch: made for the round screen (lists and rings that follow the circle, scrolling with momentum and the bezel), a
  smoother touchpad (pointer moves are joined on the Bluetooth link and glided by the phone), and the gyro mouse has
  two modes: *Move* (like a mouse) and *Point* (the pointer goes where the arm points).

* New: the **watch works without Wi-Fi**: it talks to the phone over Bluetooth and the phone passes everything to the PCs
  (phone app: Settings > My watch). The watch is paired with the phone by a 6-digit code and shows the phone's PCs.

* New: **Oxide Remote for Wear OS** (`wear/`, docs/WEAR.md): PC list with wake-up, touchpad or gyro mouse (pinch to
  click), controls and stats as swipeable pages, theme (watch, color, PC), updates.
* Fixed: X11 menus and dropdowns (Unity and others) keep the arrow cursor, and X11 popup windows appear at their own
  place and size instead of as a large empty floating window.
* Changed: phone app updates install through a broadcast receiver (as in Hearth), show a progress bar, and ask for
  no extra confirmation when Android allows it. PCs are found again by their MAC address on any network (also where
  broadcasts are blocked: the subnet is probed address by address).
* New: **performance profiles** Eco / Normal / Performance in the control center (and Settings > Power, the
  `set_profile` action, `[power] profile`). Eco turns off blur, shadows and telemetry, halves animation times and sets
  the system's power mode; it can switch on by itself under a battery level. Your own settings are never overwritten.
* New: **the lid and idle**: closing the lid locks and/or sleeps (separately on battery and on mains, not with an external
  screen, with a delay), idle lock and sleep timers, `lock` action. All in Settings > Power and `[power]`.
* New: the assistant knows Oxide (built-in handbook: actions, config keys, recipes), so it needs fewer steps.
* New: the phone app's **Assistant tab**: talk to the PC's assistant from the phone (answers word by word, confirmations
  on the phone).
* Faster workspace indicator animation in the bar.
* New: **Oxide Remote for Windows** (`oxide-remote-host`): the phone app's stream, control, stats and actions with a
  Windows PC, DXGI capture and a newest-frame-only pipeline.
* New: the phone app's **Remote tab**: a touchpad without a picture (acceleration, long-press drag, two-finger scroll,
  three-finger swipes between workspaces), workspace strip, click buttons and quick actions.
* Fixed: dragging and resizing windows has no delay (no animation while dragging), and tiled windows stay tiled:
  Super+drag swaps, Super+right-drag moves the split (like Hyprland).
* New: unlocking the phone from the PC (Super+Shift+P): Android hides its PIN pad from screen mirrors, so the PIN is asked
  on the PC and typed on the phone (never stored).
* Expressive everywhere: Settings (cards, colored titles, filter chips, Material switch), clipboard, calendar, HUD,
  notifications, overview tiles (rounded), bar modules.
* New: **your phone on the PC** (Super+P): its screen with very little delay, usable while it is locked in your pocket.
* New: Hearth's expressive motion: elastic presses with light and ring, staggered entrances, morphing chips.
* Faster panels: key binds reach the shell directly (no process start), the launcher searches a prepared index and
  loads icons in the background.
* New: **Material Expressive** style for the whole desktop and the phone app (Settings → Theme → Style).
* New: phone app finds a PC again on any network it moves to; encrypted notes work without port forwarding.
* Fixed: sliders in Settings (clean thick track, solid handle, nothing past the handle), the border color bar has rounded ends.
* New: **automatic updates** from the git repository (`oxide-ctl update`), with an "Oxide was updated" card
  after the next login; Settings → Config → Updates.
* New: the phone app reaches the PC **away from home** (UPnP, IPv6, VPN addresses, encrypted notes on ntfy),
  goes **full screen** with pinch zoom, follows the phone's **Material You** colors, updates itself, and wakes a PC
  through another one that is on.
* Fixed: Electron apps that place their own views (like Lodica) kept their default size inside a tiled window.
* New: **overview** (`Super+Tab`): a live map of all workspaces, drag windows between them.
* New: **macro scratchpads**: one key summons or dismisses a group of apps laid out side by side.
* New: **zero-bar mode** with a HUD while `Super` is held.
* New: **context-aware tiling** (windows with errors or calls grow) and **telemetry borders** (build pulse, error
  glow, call wave, CPU warmth).
* New: **config errors never break the session**: the last good config stays, a card shows the line and opens `$EDITOR`.
* New: the assistant is a **docked panel** that takes its own space instead of floating over windows.
* New: **Oxide Remote**: `oxide-remoted` plus an Android app in Rust (live screen, stats, touchpad, keyboard,
  media, notifications, Wake-on-LAN, several PCs). Settings → Phone.
* New: **window options** (`Super+O`): act on the window and remember rules per app, with undo.
* New: the theme can **follow the wallpaper** (default shell: the chosen picture; own shells: whatever the
  background layer shows).
* New: eight bundled **real photographs** as wallpapers (replacing the drawn scenes), cross-fading when changed.
* New: settings redesign (uppercase section labels, inset segmented controls, Lucide icons, palette check marks,
  photo grid), cross-fading theme changes, quick settings with tile switches and percentages.
* New: own shells hide the Bar and Launcher pages; switching to one selects the wallpaper palette.
* New: per-monitor popups and workspace highlight, the monitor under the pointer is the active one.
* New: `oxide-ctl terminal | files | ipc | doctor | rule | default-config`; portable default shortcuts.
* New: `make install` / `DESTDIR`, system-wide data directories, shell memory watchdog.
* Fixed: `Super+1…9` on layouts where the digit row needs Shift; Super shortcuts over open popups; animation
  frames repeating (animations now advance per frame); fullscreen now animates; many flicker/hover fixes.
* Changed: default keyboard layout is detected (was a fixed one); no default monitor entry.
