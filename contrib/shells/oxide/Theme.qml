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
    readonly property real rControl: Math.round(roundness * 0.5)
    readonly property real rBox: roundness
    readonly property real rPop: Math.round(roundness * 1.25)

    // motion: one curve, short durations
    readonly property var curve: [0.2, 0.8, 0.2, 1, 1, 1]
    readonly property int fast: 110
    readonly property int normal: 200
    readonly property int slow: 320

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
