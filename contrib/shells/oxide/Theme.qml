// Colors, fonts, shape and motion. Colors come from ~/.config/oxide/theme.json, which Oxide writes when you
// change the theme in Settings; the shell follows live. The values below are the fallback (Oxide preset).
// Design rules are in DESIGN.md at the root of the Oxide repository.
pragma Singleton
import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property color bg: "#1b1917"
    property color surface: "#232120"
    property color raised: "#2c2926"
    property color line: "#36322e"
    property color text: "#e9e4de"
    property color muted: "#9a928a"
    property color accent: "#d4572e"
    property color accentSoft: "#e08a63"
    property color ink: "#1d0f09"          // text on top of the accent
    property color good: "#5fa391"
    property color warn: "#d9a441"
    property color bad: "#c8504a"
    property color blue: "#6fa8dc"

    // Material roles (for the Oxide presets they are derived from the colors above)
    property bool expressive: false
    property color container: Qt.tint(surface, alpha(accent, 0.28))
    property color onContainer: text
    property color secondaryContainer: Qt.tint(surface, alpha(accentSoft, 0.14))
    property color onSecondaryContainer: text
    property color tertiary: blue
    property color surfaceLow: Qt.tint(bg, alpha(surface, 0.5))
    property color surfaceHigh: raised
    property color surfaceHighest: Qt.tint(raised, alpha(text, 0.06))
    property color outline: Qt.tint(line, alpha(muted, 0.5))

    // fonts (IBM Plex, shipped in ./fonts)
    readonly property string sans: "IBM Plex Sans"
    readonly property string mono: "IBM Plex Mono"
    FontLoader { source: Qt.resolvedUrl("fonts/IBMPlexSans-Regular.ttf") }
    FontLoader { source: Qt.resolvedUrl("fonts/IBMPlexSans-Medium.ttf") }
    FontLoader { source: Qt.resolvedUrl("fonts/IBMPlexSans-SemiBold.ttf") }
    FontLoader { source: Qt.resolvedUrl("fonts/IBMPlexMono-Regular.ttf") }
    FontLoader { source: Qt.resolvedUrl("fonts/IBMPlexMono-Medium.ttf") }

    // shape: a scale, not one radius for everything
    // all derived from one roundness value (Settings > Theme), 0 is square
    property real roundness: 8
    property bool light: false
    property string themeName: "oxide"
    // Expressive: controls are pills, cards and panels big and soft
    readonly property real rControl: expressive ? 999 : Math.round(roundness * 0.5)
    readonly property real rBox: expressive ? Math.max(20, roundness) : roundness
    readonly property real rPop: expressive ? 28 : Math.round(roundness * 1.25)

    // motion: one curve, short durations
    readonly property var curve: [0.2, 0.8, 0.2, 1, 1, 1]
    // every duration goes through this factor (0 turns motion off)
    property real motionScale: 1
    readonly property int fast: 110 * motionScale
    readonly property int normal: (expressive ? 240 : 200) * motionScale
    readonly property int slow: (expressive ? 380 : 320) * motionScale
    // movement (position, size, scale): Expressive springs past the target a little and settles;
    // colors and fades never overshoot
    readonly property int spatialType: expressive ? Easing.OutBack : Easing.OutCubic
    readonly property real overshoot: expressive ? 1.25 : 0.6
    readonly property int spatial: (expressive ? 420 : 260) * motionScale

    function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a); }
    // "off" state of a colour that fades in: the same colour at zero alpha. Animating from plain "transparent" passes
    // through dark (black at zero alpha), which shows as a dark flash on every hover.
    function clear(c) { return Qt.rgba(c.r, c.g, c.b, 0); }

    readonly property string dataDir: (Quickshell.env("XDG_DATA_HOME") || (Quickshell.env("HOME") + "/.local/share")) + "/oxide"
    readonly property string configDir: (Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config")) + "/oxide"

    function apply(t) {
        const set = (k, v) => { if (v) root[k] = v; };
        set("bg", t.bg); set("surface", t.surface); set("raised", t.raised); set("line", t.line);
        set("text", t.text); set("muted", t.muted); set("accent", t.accent); set("accentSoft", t.accent_soft);
        set("ink", t.ink); set("good", t.good); set("warn", t.warn); set("bad", t.bad); set("blue", t.blue);
        if (t.radius !== undefined) root.roundness = t.radius;
        root.expressive = t.style === "expressive";
        set("container", t.container); set("onContainer", t.on_container);
        set("secondaryContainer", t.secondary_container); set("onSecondaryContainer", t.on_secondary_container);
        set("tertiary", t.tertiary); set("surfaceLow", t.surface_low); set("surfaceHigh", t.surface_high);
        set("surfaceHighest", t.surface_highest); set("outline", t.outline);
        root.light = t.light === true;
        if (t.name) root.themeName = t.name;
    }

    FileView {
        path: root.configDir + "/theme.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { try { root.apply(JSON.parse(text())); } catch (e) { console.log("theme.json:", e); } }
    }
}
