# Oxide Remote (phone app)

See your PC's screen with very little delay, read its numbers, and control it from your phone. Works with several
PCs, and wakes the ones that are off with Wake-on-LAN.

| | |
|---|---|
| **Screen** | the PC's screen live (JPEG stream, up to 30 fps; only the newest frame is ever on its way, so there is no lag build-up). Control it as a touchpad (drag moves the pointer, tap clicks, long press right-clicks, two fingers scroll) or tap directly where you want to click. Type with the phone keyboard; Esc, Tab, Enter, arrows, Super and Alt+Tab are one tap away. |
| **Stats** | CPU (total and per core, with a one-minute graph), memory and swap, network speed, temperatures (CPU, GPU, SSD), GPU load, battery, disks, the busiest programs, uptime. |
| **Control** | play/pause/next/previous and volume, switch workspaces, send text to the PC's clipboard or fetch it, open an app or a web address on the PC, show a notification, lock, sleep, restart, shut down. |
| **Full screen** | the expand button shows the PC's screen on the whole phone, in landscape, without the phone's bars. Pinch to zoom, two fingers move the zoomed picture. A touch on the little handle at the top shows the toolbar: exit, keyboard, touchpad or tap, Super, Alt+Tab, Esc. |
| **Several PCs** | each paired once; the list shows which are online, and whether at home or over the internet. |
| **Away from home** | with *Reach this PC from anywhere* on, the phone finds the PC over the internet too (see below). |
| **Wake-on-LAN** | a PC that is off shows **Wake up**: the phone sends the magic packet itself (at home), and asks every other paired PC that is on to send it too, so it also works away from home when one of your PCs is running. |
| **Looks like your phone** | the app follows the phone's dark or light mode and its Material You colors (Android 12+). In the app's settings it can use your PC's Oxide colors or Oxide's own instead. |
| **Updates** | the app checks GitHub for a newer version; **Update** downloads it and Android asks you to confirm. |

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
scripts/build-apk.sh          # → OxideRemote-<version>.apk (also needs a JDK for the small Java part)
adb install OxideRemote-*.apk
```

`scripts/release.sh` builds it and publishes it as a GitHub release, where the app's **Update** button finds it.
An update installs only over an app signed with the same key, so keep the key you build with.

## Your phone on the PC

**Super+P** (or *Settings → Phone → Show my phone*) opens your phone's screen in a window, with very little delay
(scrcpy: the phone's own video encoder, typically 35–70 ms on good Wi-Fi). You can use it with mouse and keyboard
while the phone stays locked in your pocket: its own screen stays dark, a locked phone shows its lock screen and you
type the PIN on the PC. Closing the window turns the phone's screen off again. Super+P again closes it.

Setup, once: install `scrcpy` and `android-tools`, turn on USB debugging on the phone (Developer options), connect it
with USB and press *Settings → Phone → Set up phone screen*. From then on it works over Wi-Fi. A phone restart turns
Wi-Fi debugging off; with **Show this phone on my PCs** on in the app (its settings), the app switches Android's
wireless debugging back on whenever it starts, and the PC finds the phone by itself.

`[remote] phone_max_size` (1600), `phone_bitrate` (12, Mbit/s), `phone_audio` (false: the phone's sound stays on the phone).

## Other networks (school, work, a friend's)

The phone remembers each PC for good. When a PC is not at its last address (it is on another network, or got a new
address), the app looks for it by its id on the network the phone is on, tries the local addresses it was last
seen with, and reads the PC's encrypted note on ntfy for where it is now. The home screen shows the PC as
online and the saved address is updated. *Settings → Phone → Find this PC on any network* (on by default) controls the
note; it needs no port forwarding. Networks that isolate devices from each other (some schools) can still block it:
then use *Reach this PC from anywhere* or Tailscale.

## Away from home

*Settings → Phone → Reach this PC from anywhere* (off by default):

1. The PC asks your router to forward one TCP port to it (UPnP; most home routers have it on, otherwise switch it
   on in the router or forward TCP 47820 to the PC yourself). The settings page shows the public address it got.
2. Connect once at home: the phone learns where the PC can be reached.
3. When the address changes (most home connections get a new one now and then), the PC leaves an encrypted note
   for each paired phone on a [ntfy](https://ntfy.sh) server: the topic name comes from the pairing key, the content
   is sealed with it, so only that phone can find and read it. `[remote] rendezvous` points to your own ntfy server
   if you prefer.

Also used when present: the PC's global IPv6 addresses and VPN addresses (Tailscale, ZeroTier). If your provider
gives no public IPv4 address (CGNAT), IPv6 or a VPN is the way.

From the internet the PC only accepts paired phones (the key is checked before anything else happens), never
pairing, at most a few handshakes at a time.

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
* Without *Reach this PC from anywhere*, oxide-remoted is only reachable on your local network. With it, only
  the TCP port is forwarded, and only paired phones get past the first message.

## Command line

`oxide-remoted pair | status | forget <id> | rename <name> | cancel`.
