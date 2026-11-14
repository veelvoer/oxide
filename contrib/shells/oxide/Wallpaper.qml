// Desktop background. Options live in ~/.config/oxide/wallpaper.json (Settings > Theme > Background):
//   { "mode": "photo" | "theme" | "image" | "color",
//     "photo": "dolomites",                      // one of the bundled photos (see ~/.local/share/oxide/wallpapers)
//     "path": "/home/me/Pictures/x.jpg",         // mode "image"
//     "color": "#1b1917", "fit": "fill"|"fit"|"center" }
// "theme" draws a quiet background from the palette with the Oxide tiles in a corner.
// Changing the background cross-fades, and only the picture being shown is kept in memory.
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick

Scope {
    id: root
    property var opts: ({})
    // the older "scene" modes became photos
    readonly property string mode: { const m = opts.mode || "photo"; return (m === "scene1" || m === "scene2") ? "photo" : m; }
    readonly property string photo: opts.photo || "dolomites"
    // where bundled photos may live: the user's data dir, then the system one
    readonly property var photoDirs: [Theme.dataDir + "/wallpapers", "/usr/local/share/oxide/wallpapers", "/usr/share/oxide/wallpapers"]
    // the file to show now ("" = none)
    readonly property string file: mode === "image" ? (opts.path || "") : mode === "photo" ? photo + ".jpg" : ""

    FileView {
        path: Theme.configDir + "/wallpaper.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { try { root.opts = JSON.parse(text()); } catch (e) { root.opts = ({}); } }
    }

    Variants {
        model: Quickshell.screens
        PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            color: Theme.bg
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Background
            WlrLayershell.namespace: "oxide-wallpaper"

            // flat color
            Rectangle {
                anchors.fill: parent
                property bool on: root.mode === "color"
                opacity: on ? 1 : 0
                visible: opacity > 0.001
                Behavior on opacity { NumberAnimation { duration: 450; easing.type: Easing.InOutQuad } }
                color: root.opts.color || Theme.bg
            }

            // pictures: two slots, the new one fades in over the old one, then the old one is released
            Item {
                id: pics
                anchors.fill: parent
                property int front: 0
                property string shown: ""
                function show(f) {
                    if (f === shown) return;
                    shown = f;
                    const back = 1 - front;
                    slots.itemAt(back).want = f;
                }
                Component.onCompleted: show(root.file)
                Connections { target: root; function onFileChanged() { pics.show(root.file); } }

                Repeater {
                    id: slots
                    model: 2
                    Image {
                        id: img
                        required property int index
                        property string want: ""
                        property int candidate: 0
                        readonly property bool isFront: pics.front === index
                        anchors.fill: parent
                        asynchronous: true
                        cache: false
                        autoTransform: true
                        sourceSize.width: win.width
                        fillMode: root.opts.fit === "fit" ? Image.PreserveAspectFit : root.opts.fit === "center" ? Image.Pad : Image.PreserveAspectCrop
                        // a bundled name is looked up in each wallpaper folder in turn; a full path is used as it is
                        source: {
                            if (want === "") return "";
                            if (want.charAt(0) === "/") return "file://" + want;
                            return "file://" + root.photoDirs[Math.min(candidate, root.photoDirs.length - 1)] + "/" + want;
                        }
                        onWantChanged: candidate = 0
                        onStatusChanged: {
                            if (status === Image.Error && want.charAt(0) !== "/" && candidate < root.photoDirs.length - 1) candidate++;
                            else if (status === Image.Ready && want !== "" && !isFront) { pics.front = index; }
                        }
                        opacity: isFront && want !== "" ? 1 : 0
                        visible: opacity > 0.001
                        z: isFront ? 1 : 0
                        Behavior on opacity { NumberAnimation { duration: 450; easing.type: Easing.InOutQuad } }
                        // the picture that faded out lets go of its memory
                        onOpacityChanged: if (opacity === 0 && !isFront) { want = ""; }
                    }
                }
                // nothing to show (a color, the logo): both slots fade out
                Connections {
                    target: root
                    function onFileChanged() { if (root.file === "") { slots.itemAt(0).want = ""; slots.itemAt(1).want = ""; pics.shown = ""; } }
                }
            }

            // theme background (the logo)
            Item {
                anchors.fill: parent
                property bool on: root.mode === "theme" || (root.mode === "image" && !root.opts.path)
                opacity: on ? 1 : 0
                visible: opacity > 0.001
                Behavior on opacity { NumberAnimation { duration: 450; easing.type: Easing.InOutQuad } }
                Rectangle {
                    anchors.fill: parent
                    gradient: Gradient {
                        GradientStop { position: 0; color: Theme.bg }
                        GradientStop { position: 1; color: Qt.darker(Theme.bg, Theme.light ? 1.06 : 1.25) }
                    }
                }
                // the tiles, small and quiet in the lower right
                Item {
                    id: glyph
                    readonly property real u: Math.min(parent.width, parent.height) * 0.30
                    width: u * 0.86; height: u * 0.86
                    x: parent.width - width - parent.width * 0.05
                    y: parent.height - height - parent.height * 0.08
                    readonly property real r: Math.max(2, Theme.rBox * 0.9)
                    Rectangle { x: 0; y: 0; width: glyph.u * 0.43; height: glyph.u * 0.86; radius: glyph.r; color: "transparent"; border.color: Theme.line; border.width: 2 }
                    Rectangle { x: glyph.u * 0.5; y: 0; width: glyph.u * 0.36; height: glyph.u * 0.41; radius: glyph.r; color: Theme.alpha(Theme.accent, 0.14); border.color: Theme.alpha(Theme.accent, 0.5); border.width: 2 }
                    Rectangle { x: glyph.u * 0.5; y: glyph.u * 0.45; width: glyph.u * 0.36; height: glyph.u * 0.41; radius: glyph.r; color: "transparent"; border.color: Theme.line; border.width: 2 }
                }
            }
        }
    }
}
