# Oxide Remote for Wear OS

The watch app (Galaxy Watch 4 and newer, Wear OS 3+): your PCs on your wrist. Source in `wear/`. **It needs no Wi-Fi**:
the watch talks to your phone over Bluetooth, and the phone (its Oxide Remote app) keeps the encrypted sessions with the
PCs and passes everything on. A watch on Wi-Fi drains its battery; Bluetooth to the phone barely does.

## Set up (once)

1. Install the phone app, then on the phone: **Settings > My watch > Let my watch connect: On** (Android asks for the
   Bluetooth and notification permission; a quiet notification keeps the connection alive, also after a restart).
2. The watch and phone must be paired in Android's own Bluetooth settings (they are, when the watch was set up with the phone).
3. Open the watch app: it finds the phone. Press **Enter code** on the watch, press **Pair a watch** on the phone, type
   the 6 digits (valid 2 minutes, three wrong tries cancel it). From then on the watch is known and connects by itself.
   "Forget this phone" (watch) or the bin icon (phone) undoes it.

The PCs are the ones paired with the phone: nothing to pair on the watch.

## Screens

* **Home**: your PCs (from the phone) with their state; tap to open, the round power button wakes one (the phone sends
  the Wake-on-LAN packet, also through another PC that is on).
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

**Point at the screen with your index finger and thumb, then move your hand: the pointer follows; pinch to click.** It
also works while the watch's screen is off, for as long as a PC is open in the app (a quiet notification shows it; it
stops when you leave the PC or close the app). Lower your arm to pause the pointer, raise it to carry on.

Two modes (Setup > Cursor > Gyro): **Move** turns the wrist into mouse movement (rest your arm to stop), **Point** puts the
pointer where your arm points (about 35° sideways reaches the screen's edge at speed 1; tap the middle to center). Set
*Worn on right wrist* if you wear it on the right.

The pointer follows the wrist's rotation (gyroscope). **A pinch is detected from the accelerometer**: Wear OS has no
public pinch API, so the app listens for the short sharp tap a pinch makes while the wrist is steady. One pinch =
left click, three quick pinches = right click. This is a heuristic: try it in Setup > Cursor (it counts the pinches it
sees) and adjust *Pinch sensitivity*; the middle of the pad pauses the pointer so your arm can rest. The sensors only run
while the gyro pad (or its test) is on screen.

## How it connects

Watch <-> phone: Bluetooth (RFCOMM service of the phone app, encrypted by Android; the watch also proves itself with a
token it got when it was paired). Phone <-> PC: as always (home network or the internet route). Away from home that
works through the phone's mobile data. If the watch says "Looking for your phone", check that Bluetooth is on and that
"Let my watch connect" is on in the phone app.

## Build and install

```sh
cd wear && bash scripts/build-apk.sh        # OxideWatch-<version>.apk (arm64 and 32-bit arm)
adb connect <watch-ip>:<port>               # Settings > Developer options > Wireless debugging on the watch
adb install -r OxideWatch-0.2.0.apk
bash scripts/release.sh                     # publish on GitHub (tag watch-vX.Y.Z), where the app's Update finds it
```
