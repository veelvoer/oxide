// The bar's layout and options, from ~/.config/oxide/bar.json (edited in Settings > Bar, applied live).
// Format:
//   { "position": "top"|"bottom", "style": "flat"|"island"|"floating", "height": 36, "margin": 6, "radius": 8, "opacity": 0.96,
//     "left": [ {"id": "launcher"}, {"id": "workspaces", "style": "numbers"} ],
//     "center": [ {"id": "clock", "seconds": false} ], "right": [ {"id": "tray"}, {"id": "status"} ] }
// Every module is a file named Mod<Id>.qml in this folder; add your own by dropping a new file in and
// putting its id into the lists.
pragma Singleton
import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property string position: "top"
    property string style: "flat"
    property int height: 36
    property int margin: 6
    property int radius: -1          // -1 follows the theme's roundness
    property int spacing: 6
    property string lanes: "plain"   // "plain" or "pill": give each group its own rounded background
    readonly property real cornerRadius: radius >= 0 ? radius : Theme.rBox
    property real opacity: 0.96
    property var left: [{ "id": "launcher" }, { "id": "workspaces" }, { "id": "window" }]
    property var center: [{ "id": "clock" }]
    property var right: [{ "id": "tray" }, { "id": "status" }, { "id": "bell" }, { "id": "assistant" }]

    // space the bar takes at the screen edge, popups sit just beyond it
    readonly property real inset: style === "flat" ? 0 : margin
    readonly property real barSpace: height + inset * 2

    function apply(c) {
        // keys that are absent go back to their defaults, so removing one in the editor takes effect
        root.radius = -1;
        const set = (k, v) => { if (v !== undefined && v !== null) root[k] = v; };
        set("position", c.position); set("style", c.style); set("height", c.height); set("margin", c.margin);
        set("radius", c.radius); set("spacing", c.spacing); set("lanes", c.lanes); set("opacity", c.opacity); set("left", c.left); set("center", c.center); set("right", c.right);
    }

    FileView {
        path: Theme.configDir + "/bar.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { try { root.apply(JSON.parse(text())); } catch (e) { console.log("bar.json:", e); } }
    }
}
