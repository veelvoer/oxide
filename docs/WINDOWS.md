# Oxide Remote for Windows

The Oxide Remote phone app (Android) works with any Windows PC through **Oxide Remote host**, a small Rust program
with the same low-latency stream, touchpad, keyboard, stats and controls. You do not need Oxide, Linux or anything
else on the PC.

## Install

1. Download `oxide-remote-host.exe` from the [releases page](https://github.com/veelvoer/oxide/releases) (or build it,
   see below) and run it. No installer, no admin rights needed.
2. Windows asks whether to let it use the network: allow **private** networks. If the phone cannot find the PC
   later, press *Allow in Windows Firewall* in the app.
3. Press **Show a code** in the window. On the phone (Oxide Remote app): *Add PC*, pick the PC, type the code.
4. Optional: tick *Start with Windows (minimized)*. Closing the window only minimizes it; *Quit* ends it.

## What works

| | |
|---|---|
| **Screen** | the screen with very little delay (DXGI desktop duplication: only changed pictures are captured, a one-slot mailbox always sends the newest one, at most two frames in flight). Falls back to GDI where duplication is not available. The pointer is drawn into the picture. |
| **Control** | touchpad, tap-to-click, keyboard, text, scroll, right click, drag (SendInput). |
| **Remote tab** | the touchpad without a picture; *Overview* is Win+Tab, *Close* Alt+F4, *Float* Win+Up, *Fullscreen* F11, workspace arrows switch virtual desktops (Ctrl+Win+Left/Right). |
| **Stats** | processor (per core), memory, disks, network, the busiest programs, battery, uptime. |
| **Control tab** | media keys, volume (steps), mute, lock, sleep, restart, shut down, open an address or a program, clipboard in both directions, screenshot (Win+Shift+S). |
| **Away from home** | the same as on Linux: encrypted notes on ntfy with the PC's address on whatever network it is on, and optionally router forwarding (UPnP). Only paired phones get in. |
| **Several PCs** | pair as many PCs as you like (Linux with Oxide, Windows, both). |

Not available on Windows: jumping to virtual desktop number N (only one by one), the exact volume slider, the media
title, GPU load, Wake-on-LAN *for this PC* works if the BIOS and network card allow it (the app shows its address).

## Your phone's screen on the PC

The window has a *Your phone's screen on this PC* section: **Get the tools** downloads scrcpy (with adb) from its official
release into the app's own folder, **Show / hide my phone** opens the phone in a window (its own screen stays dark, so it
can stay in your pocket), **Unlock the phone** types the PIN you enter (Android hides its PIN pad from mirrors; the PIN is
never stored). For Wi-Fi without a cable: connect the phone with USB once and press *Set up*, switch on *Show this phone on
my PCs* in the Oxide Remote app, then *Pair over Wi-Fi* and scan the QR code (Developer options › Wireless debugging).

## Updates

The program checks GitHub for a newer `windows-v…` release and shows an **Update** button; one click downloads it,
swaps it in and restarts. The Android app does the same from its settings (Updates).

## Latency

Frames are JPEG, encoded on the CPU (about 4–6 ms for 1280 px on a recent PC), sent over an encrypted TCP stream
with `TCP_NODELAY`. On a good Wi-Fi expect roughly 30–60 ms from screen to phone. The app's settings pick the picture
quality; the phone asks for a smaller, faster picture away from home.

## Build it yourself

From Linux (needs Rust, and zig for the cross-compile): 

```sh
pip install --user ziglang && cargo install cargo-zigbuild
rustup target add x86_64-pc-windows-gnu
cargo zigbuild --release --target x86_64-pc-windows-gnu -p oxide-remote-host
# → target/x86_64-pc-windows-gnu/release/oxide-remote-host.exe
```

On Windows: `cargo build --release -p oxide-remote-host`.

The same program builds on Linux (with a moving test picture instead of the screen), which is how its server is tested.
