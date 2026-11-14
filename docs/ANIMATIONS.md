# Animations

Everything that moves in Oxide is driven by one small set of settings. In *Settings → Animations* pick a **feel**;
in Advanced mode tune every animation on its own.

## Feels

| Feel | Character |
|---|---|
| **Springy** (default) | a spring that overshoots a little and settles |
| **Smooth** | long ease-out curves, no overshoot |
| **Snappy** | short and direct |
| **Bouncy** | a loose spring with a visible bounce |

*Animate things* turns every animation off at once (windows then appear and move instantly).

## What animates

| Config key | When |
|---|---|
| `windows` | a window moves or resizes (layout change, float/unfloat, fullscreen, drag release) |
| `windows_in` / `fade_in` | a window opens (scale up from `style`, fade in) |
| `windows_out` / `fade_out` | a window closes |
| `workspaces` | switching workspaces (`style = "slide"` slides, anything else crossfades) |
| `border` | the focus border changes color |

The shell has its own motion that follows the same feel: popups grow out of the bar button they belong to,
panels morph to their new size when their content changes, the wallpaper cross-fades, the theme fades when you
change palette, toggles glide. All of it is interruptible: starting a new motion from the middle of another
continues smoothly instead of jumping.

## Settings per animation

```toml
[animations]
enabled = true

[animations.windows_in]
enabled = true
speed = 3.6          # bezier curves: duration in tenths of a second (3.6 = 360 ms); springs ignore it
curve = "spring"     # the name of a curve below
style = "gnomed"     # "popin 80%" or "gnomed" (scale from 90%), "slide", or "" for none
```

### Curves

```toml
[[animations.curves]]
name = "ease"; kind = "bezier"      # CSS cubic-bezier(x1, y1, x2, y2)
x1 = 0.25; y1 = 1.0; x2 = 0.5; y2 = 1.0

[[animations.curves]]
name = "bounce"; kind = "spring"    # a damped spring released from 0 to 1
mass = 0.6; stiffness = 170.0; dampening = 9.0
```

* A **bezier** takes `speed` × 100 ms. `ease`, `snap`, `soft` and `linear` are built in.
* A **spring** needs no duration: it ends when it has settled. Higher `stiffness` is faster, lower `dampening`
  bounces more, `mass` makes it heavier. `spring` and `bounce` are built in.

The settings app draws each curve and previews it with a moving box, so you can feel a change before you keep it.

## Frame pacing

Animations advance to the exact moment of each frame, so they stay even at 60, 120 or 144 Hz, and the compositor
draws only while something moves.
