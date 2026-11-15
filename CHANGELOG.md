# Changelog

## 0.2.0 (unreleased)

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
