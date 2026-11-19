# Oxide Remote for Wear OS

The watch app (Galaxy Watch 4 and newer, Wear OS 3+): your PCs on your wrist. Source in `wear/`; it shares the
protocol, pairing, PC list and colors with the phone app (`phone/src`), so it talks to the same `oxide-remoted`.

## Screens

* **Home**: your PCs with their state; tap to open, the round power button wakes one (Wake-on-LAN, also through
  another PC that is on, when the watch is not on its network). **Add** pairs a new PC from the watch itself: it
  lists PCs that answer on the Wi-Fi, you type the 6 digits the PC shows (Settings > Phone > Pair a phone).
* **A PC** has three pages side by side, with dots below:
  1. **Touchpad** or **gyro mouse** (see below), with Left / Right click buttons.
  2. **Controls**: lock, sleep, mute, media, volume, overview. The rotary bezel changes the volume.
  3. **Stats**: processor, memory, temperature, battery.
  Swipe left for the next page, right to go back. On the touchpad, swipes that start at the left or right edge
  change the page (its middle moves the pointer); with the gyro mouse a swipe anywhere does.
* **Setup**: theme, cursor, updates.

## Settings

* **Theme**: *Watch* (Oxide's colors on black), *Color* (pick an accent), *PC* (the colors of your PC's Oxide
  theme; open a PC once).
* **Cursor**: *Touchpad* (drag, tap = left click, hold = right click, a little acceleration) or *Gyro mouse*
  (move your hand, pinch to click). Speed, flip left/right and up/down.
* **Updates**: looks at GitHub (releases tagged `watch-vX.Y.Z`), downloads and installs from the app, like the
  phone app. Can check by itself.

## The gyro mouse

The pointer follows the wrist's rotation (gyroscope). **A pinch is detected from the accelerometer**: Wear OS has no
public pinch API, so the app listens for the short sharp tap a pinch makes while the wrist is steady. One pinch =
left click, three quick pinches = right click. This is a heuristic: try it in Setup > Cursor (it counts the pinches it
sees) and adjust *Pinch sensitivity*; the middle of the pad pauses the pointer so your arm can rest. The sensors only run
while the gyro pad (or its test) is on screen.

## Network

The watch needs to reach the PC: on the same Wi-Fi (discovery and pairing) or over the internet route the PC
publishes (the same as the phone). Wi-Fi must be on; a watch that only has Bluetooth to its phone can use the internet
route but cannot find PCs on the local network.

## Build and install

```sh
cd wear && bash scripts/build-apk.sh        # OxideWatch-<version>.apk (arm64 and 32-bit arm)
adb connect <watch-ip>:<port>               # Settings > Developer options > Wireless debugging on the watch
adb install -r OxideWatch-0.1.0.apk
bash scripts/release.sh                     # publish on GitHub (tag watch-vX.Y.Z), where the app's Update finds it
```
