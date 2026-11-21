# Presets and GitHub

**Settings > Presets** saves your whole setup under a name and brings it back later.

* A preset holds every settings file Oxide has: `oxide.toml` (windows, animations, input, keys, theme, power...), the bar,
  the launcher, the wallpaper, the look of Settings and the assistant's settings. **The assistant's API key is never in a
  preset** (and never sent anywhere): it is blanked in the copy, and when a preset is used the key of this computer stays.
  Phone pairing keys and the assistant's chat history are not part of it either.
* *Use* makes the setup what the preset holds. What was there before is kept as **Before the last preset**: press *Undo*
  on it to go back.
* *Replace with now* updates a preset with the setup of the moment. Presets are files in `~/.config/oxide/presets/`.
* From a terminal: `oxide-ctl preset list | save <name> | use <name> | delete <name>`.

## GitHub

Sign in to keep your presets and your current setup on your own GitHub account, to use on another computer. They live in a
**private gist** called "Oxide configuration" (only `gist` permission is asked). *Save to GitHub* uploads, *Get from GitHub*
adds your presets here (and, if you choose, the setup itself).

Two ways to sign in:

* **GitHub CLI**: when `gh` is installed and signed in, *Use the GitHub CLI* takes its sign-in.
* **Device flow** (for everybody): once, register an OAuth app on GitHub (Settings > Developer settings > OAuth Apps > New
  OAuth App, tick **Enable Device Flow**), paste its *client id* in Settings > Presets and press *Sign in*: GitHub shows a
  short code to type at github.com/login/device. The token is kept in `~/.config/oxide/github.json` (only readable by you);
  *Sign out* forgets it.

Needs `curl`. Settings changed by something else while Settings is open (the control center's performance profile, the
assistant, a preset) are merged with yours, not overwritten.
