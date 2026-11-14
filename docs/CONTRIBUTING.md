# Contributing

## Build and test

```sh
cargo build --release
cargo test --release
```

Style: `cargo fmt` is not enforced yet; match the surrounding code (short comments that say why, no dead code).

## Running it while you work

* Nested in your current desktop: `target/release/oxide --winit`. Test options (environment): `OXIDE_SIZE=3840x1080`
  window size, `OXIDE_OUTPUTS=2` splits the window into that many monitors, `OXIDE_NORMAL=1` renders like a real
  monitor, `OXIDE_TEST=1` stops theme writes from touching your real apps.
* Use a separate `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `OXIDE_SOCKET` so you do not touch your
  real session. The **test bed** does all of this: [tools/testbed](../tools/testbed/README.md).

## Test with real input, and look at every frame

Do not call UI work done from the code alone. The test bed runs Oxide nested in a headless host, drives it with
`oxide-ctl input …`, takes screenshots and **records every rendered frame** (`oxide-ctl record DIR N`), then
`frames.analyze` reports frame times, flicker (A-B-A frames) and stalls inside a motion. Check every state:
closed, open, empty, full, hover, dark and light, one and two monitors.

Rules the project follows (see also [../DESIGN.md](../DESIGN.md)):

* animate only transform, opacity and color; never make a window's size depend on an animated value;
* all motion is interruptible and driven by one smoothly moving value;
* colors only from the theme tokens, never hard-coded;
* every control has hover/pressed/focus states and is at least ~30 px;
* nothing overlaps, clips or truncates; text wraps or scrolls;
* optional features are off by default.

## Where things are

See [ARCHITECTURE.md](ARCHITECTURE.md). Adding a setting: field in `oxide-config`, a row in the settings page
(`crates/oxide-settings/src`), and the compositor reads it in `apply_config`. Adding a shell shortcut: an
`IpcHandler` function in `shell.qml`, a default bind in `oxide-config/src/lib.rs`, a friendly name in
`oxide-settings/src/shortcuts.rs`.

## Licenses

Code is GPL-3.0-or-later. Icons are [Lucide](https://lucide.dev) (ISC). Wallpapers keep their own licenses
(`contrib/wallpapers/CREDITS.md`). Please only add assets whose license allows redistribution and credit them.
