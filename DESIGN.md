# Oxide design contract

One place for the decisions behind the settings app and the default shell, so they stay consistent.
(Updated to the current design: Lucide line icons, uppercase small section labels, inset segmented controls,
cards with hairline row separators, a palette that can follow the wallpaper.)

## Material
Oxide is iron and rust. Surfaces are warm iron grays; the accent is rust; the second color is copper patina
(verdigris), used for "on / ok" so the rust accent stays rare.

| role | value |
|---|---|
| bg | `#1b1917` |
| surface | `#232120` |
| raised | `#2c2926` |
| line | `#36322e` |
| text | `#e9e4de` |
| muted | `#9a928a` |
| accent (rust) | `#d4572e` |
| ink on accent | `#1d0f09` |
| ok (verdigris) | `#5fa391` |
| warn (brass) | `#d9a441` |
| bad | `#c8504a` |

Accent is for: the primary button, the selected item, the focused window. Not for decoration.

## Type
IBM Plex Sans for everything, IBM Plex Mono for keys, values, clock digits and paths. Sizes: 13 (small), 14.5 (body),
22 (page title). Weight changes (Regular / Medium) carry hierarchy, not size jumps. Sentence case for text; section labels above groups are small, uppercase and letter-spaced.

## Shape
Radius is a scale, not a constant: controls 4, rows and groups 8, popups 10. Pills only for tiny tags.
No shadows except under floating popups. Hairline (1px) dividers instead of a card per item.

## Icons
Lucide line icons (ISC), single color (muted, text when active). No tinted tiles, no rainbow.

## Motion
Short (120-260 ms), one easing curve. Page change is a quick crossfade. Toggles slide. Nothing bounces on hover,
nothing fades up on a timer.

## Copy
Say what the thing does. No hype words. No hero empty states with a big icon and example chips.
