# Oxide Remote (phone app)

See your PC's screen with very little delay, read its numbers, and control it from your phone. Works with several
PCs, and wakes the ones that are off with Wake-on-LAN.

| | |
|---|---|
| **Screen** | the PC's screen live (JPEG stream, up to 30 fps; only the newest frame is ever on its way, so there is no lag build-up). Control it as a touchpad (drag moves the pointer, tap clicks, long press right-clicks, two fingers scroll) or tap directly where you want to click. Type with the phone keyboard; Esc, Tab, Enter, arrows, Super and Alt+Tab are one tap away. |
| **Stats** | CPU (total and per core, with a one-minute graph), memory and swap, network speed, temperatures (CPU, GPU, SSD), GPU load, battery, disks, the busiest programs, uptime. |
| **Control** | play/pause/next/previous and volume, switch workspaces, send text to the PC's clipboard or fetch it, open an app or a web address on the PC, show a notification, lock, sleep, restart, shut down. |
| **Several PCs** | each paired once; the list shows which are online. |
| **Wake-on-LAN** | a PC that is off shows **Wake up**: the phone sends the magic packet to the addresses it learned when you paired. |

## Set up the PC

1. Install Oxide (it includes `oxide-remoted`).
2. *Settings → Phone*: switch **Let my phone connect** on.
3. If the phone does not find the PC, open the firewall: TCP 47820 and UDP 47821
   (`sudo firewall-cmd --permanent --add-port=47820/tcp --add-port=47821/udp && sudo firewall-cmd --reload`, or
   `sudo ufw allow 47820/tcp && sudo ufw allow 47821/udp`).
4. For Wake-on-LAN: enable it in the BIOS/UEFI ("Wake on LAN", "Power on by PCI-E"), and for the network card:
   `nmcli connection modify "<your wired connection>" 802-3-ethernet.wake-on-lan magic` (or `sudo ethtool -s eth0 wol g`).
   Wake-on-LAN normally works over a cable; over Wi-Fi only on few laptops.

A PC without Oxide (another distro or desktop) can run `oxide-remoted` too: stats, actions and the screen work on
any wlroots-based compositor (Sway, Hyprland, …); mouse and keyboard control need Oxide.

## Install the app

Build the APK (needs the Android SDK and NDK, and `cargo install cargo-ndk`; `rustup target add aarch64-linux-android`):

```sh
cd phone
scripts/build-apk.sh          # → OxideRemote-<version>.apk
adb install OxideRemote-*.apk
```

## Pair

On the PC: *Settings → Phone → Pair a phone* shows a 6-digit code for two minutes. In the app: **Add PC**, pick the
PC from the list (or type its address), type the code. That's it; the PC appears on the home screen from then on.

## Security

* Only paired phones can connect. Pairing needs the code shown on the PC's screen, so someone on your network
  cannot pair without seeing your screen; three wrong codes end the pairing window.
* Pairing uses an X25519 key exchange with the code mixed in; afterwards every session derives fresh
  ChaCha20-Poly1305 keys, so everything (screen, keys you type, clipboard) is encrypted.
* *Settings → Phone → Paired phones → Forget* revokes a phone at once. The PC keeps the keys in
  `~/.config/oxide/remote.json` (readable only by you).
* oxide-remoted listens on your local network only as far as your router allows; do not forward its ports to the
  internet.

## Command line

`oxide-remoted pair | status | forget <id> | rename <name> | cancel`.
