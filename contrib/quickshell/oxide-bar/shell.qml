// Oxide default bar -- a small Quickshell example that shows how Oxide and Quickshell talk to each other.
//
// HOW IT WORKS
//   1. Oxide serves a unix socket at $XDG_RUNTIME_DIR/oxide.sock (newline-delimited JSON).
//   2. READING state: we connect a Socket and send {"cmd":"subscribe"}. Oxide answers with the current
//      state and then pushes a new line on every change (workspace switched, window focused, title changed...).
//      Each line looks like:
//        {"status":"state","state":{"active_workspace":1,"layout":"dwindle",
//          "workspaces":[{"id":1,"windows":2,"visible":true,"special":false}, ...],
//          "focused":{"app_id":"kitty","title":"~"}, "windows":[...], "outputs":[...]}}
//   3. DOING things: we run the `oxide-ctl` command line tool, e.g. `oxide-ctl workspace 3`
//      or `oxide-ctl dispatch '{"type":"toggle_special"}'`. Any action from the Oxide settings can be dispatched.
//   4. The bar itself is an ordinary layer-shell panel (PanelWindow). Oxide reserves its space automatically,
//      so tiled windows never overlap it.
//
// Start it with:  quickshell -p ~/.config/oxide/bar     (Oxide does this by default via `exec_once`)
// Edit the copy in ~/.config/oxide/bar and change anything you like (`oxide-ctl reload` does not restart it; relaunch the bar).

import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io

ShellRoot {
    id: root

    // ---- state received from Oxide -------------------------------------------------
    property var oxide: ({ active_workspace: 1, layout: "", workspaces: [], focused: null })
    property bool connected: false

    // ---- colors (change to taste) ---------------------------------------------------
    readonly property color bg: "#e6141420"
    readonly property color fg: "#cdd6f4"
    readonly property color dim: "#7f849c"
    readonly property color accent: "#89b4fa"
    readonly property color chip: "#313244"

    // Persistent connection used for state updates.
    Socket {
        id: sock
        path: Quickshell.env("XDG_RUNTIME_DIR") + "/oxide.sock"
        connected: true

        onConnectedChanged: {
            root.connected = connected
            if (connected) {
                write("{\"cmd\":\"subscribe\"}\n")
                flush()
            }
        }

        parser: SplitParser {
            onRead: line => {
                try {
                    const msg = JSON.parse(line)
                    if (msg.status === "state") root.oxide = msg.state
                } catch (e) {
                    console.log("oxide-bar: bad line", line)
                }
            }
        }
    }

    // If Oxide restarts (or is not up yet) keep trying to connect.
    Timer {
        interval: 1000
        running: !sock.connected
        repeat: true
        onTriggered: sock.connected = true
    }

    // Runs `oxide-ctl <args>` to ask Oxide to do something.
    Process {
        id: ctl
        function run(args) {
            command = ["oxide-ctl"].concat(args)
            running = true
        }
    }

    // Only show workspaces that hold windows or are on screen (like most bars).
    readonly property var shownWorkspaces: (oxide.workspaces || []).filter(w => w.windows > 0 || w.visible)

    // One bar per monitor.
    Variants {
        model: Quickshell.screens

        PanelWindow {
            required property var modelData
            screen: modelData

            anchors { top: true; left: true; right: true }
            implicitHeight: 40
            color: "transparent"

            Rectangle {
                anchors { fill: parent; margins: 6; bottomMargin: 0 }
                radius: 12
                color: root.bg

                // ---------- left: workspaces ----------
                Row {
                    anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
                    spacing: 6

                    Repeater {
                        model: root.shownWorkspaces

                        Rectangle {
                            id: pill
                            required property var modelData
                            readonly property bool active: modelData.id === root.oxide.active_workspace

                            height: 24
                            width: active ? 40 : 26          // the active pill grows smoothly
                            radius: 12
                            color: active ? root.accent : (hover.hovered ? "#45475a" : root.chip)
                            opacity: modelData.windows > 0 || active ? 1 : 0.5

                            Behavior on width { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                            Behavior on color { ColorAnimation { duration: 160 } }

                            Text {
                                anchors.centerIn: parent
                                text: pill.modelData.special ? "★" : pill.modelData.id
                                color: pill.active ? "#11111b" : root.fg
                                font { pixelSize: 13; bold: pill.active }
                            }

                            HoverHandler { id: hover }
                            TapHandler {
                                onTapped: pill.modelData.special
                                    ? ctl.run(["dispatch", "{\"type\":\"toggle_special\"}"])
                                    : ctl.run(["workspace", String(pill.modelData.id)])
                            }
                        }
                    }
                }

                // ---------- center: focused window ----------
                Text {
                    anchors.centerIn: parent
                    width: parent.width * 0.4
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    color: root.fg
                    font.pixelSize: 13
                    text: root.oxide.focused ? root.oxide.focused.title || root.oxide.focused.app_id : "Oxide"
                    opacity: root.oxide.focused ? 1 : 0.5
                    Behavior on opacity { NumberAnimation { duration: 200 } }
                }

                // ---------- right: layout, clock ----------
                Row {
                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 14

                    Text {
                        // click: cycle between dwindle and master
                        text: root.connected ? root.oxide.layout : "not connected"
                        color: root.dim
                        font.pixelSize: 12
                        TapHandler { onTapped: ctl.run(["dispatch", "{\"type\":\"cycle_layout\"}"]) }
                    }

                    SystemClock { id: clock; precision: SystemClock.Minutes }
                    Text {
                        text: Qt.formatDateTime(clock.date, "ddd d MMM  HH:mm")
                        color: root.fg
                        font.pixelSize: 13
                    }
                }
            }
        }
    }
}
