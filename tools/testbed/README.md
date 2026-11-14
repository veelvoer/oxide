# Test bed

A sandbox to run Oxide nested, drive it with real mouse and keyboard input, take screenshots and record every
rendered frame to look for flicker, stalls and dropped frames. See [../../docs/CONTRIBUTING.md](../../docs/CONTRIBUTING.md).

Needs: a release build (`cargo build --release`), `kwin_wayland` (the headless host), `grim`, `dbus-daemon`,
Python 3 with Pillow and numpy.

```sh
tools/testbed/setup.sh                 # once
cd /tmp/ox && . ./up.sh 1920x1080 1    # start the host + Oxide (size, number of monitors)
python3 - <<'PY'
import sys; sys.path.insert(0, '/tmp/ox')
from t import *; import frames
ctl('exec', 'kitty'); sleep(3)
d = record('open', 60)                 # record 60 frames
ctl('exec', 'kitty'); sleep(2)
frames.analyze(d)                      # frame times, flicker (A-B-A), stalls inside motion
shot('/tmp/ox/a.png', 'winit')         # screenshot of an output
PY
```

`up.sh WxH N [config dir]` starts N monitors side by side in one window. `lib.sh` exposes `Q shell toggleControl`
(call the test shell), `stop_ox`. Config for the test lives in `/tmp/ox/cfg/oxide/` (start empty = defaults).
