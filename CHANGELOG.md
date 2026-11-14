# Changelog

## 0.2.0 (unreleased)

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
