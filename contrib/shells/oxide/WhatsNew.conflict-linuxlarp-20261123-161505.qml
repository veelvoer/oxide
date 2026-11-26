// "Oxide was updated": shown once, at the first login after an automatic update, with what changed.
import Quickshell
import Quickshell.Wayland
import QtQuick
import Oxide

PanelWindow {
    id: win
    readonly property var notes: OxideState.whatsNew
    property bool closed: false
    readonly property bool on: !!notes && !closed
    property real t: on ? 1 : 0
    Behavior on t { NumberAnimation { duration: 260; easing.type: Easing.OutBack; easing.overshoot: 0.8 } }

    function done() {
        closed = true;
        Quickshell.execDetached(["oxide-ctl", "update", "seen"]);
    }

    screen: OxideState.activeScreen
    visible: on || t > 0.01
    color: "transparent"
    anchors { bottom: true }
    margins.bottom: 24
    implicitWidth: 560
    implicitHeight: card.height + 8
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "oxide-whats-new"

    Rectangle {
        id: card
        width: parent.width
        height: col.implicitHeight + 32
        y: (1 - win.t) * 30
        opacity: Math.min(1, win.t)
        scale: 0.96 + 0.04 * win.t
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.alpha(Theme.accent, 0.55)
        Column {
            id: col
            x: 18; y: 16
            width: parent.width - 36
            spacing: 8
            Row {
                spacing: 10
                Rectangle {
                    width: 32; height: 32; radius: 16
                    color: Theme.alpha(Theme.accent, 0.18)
                    Icon { anchors.centerIn: parent; name: "rocket-launch-outline"; size: 18; color: Theme.accent }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    Txt { text: "Oxide was updated"; font { pixelSize: 15; weight: Font.DemiBold } }
                    Txt { text: win.notes ? "Version " + win.notes.version : ""; color: Theme.muted; font.pixelSize: 12 }
                }
            }
            Flickable {
                width: parent.width
                height: Math.min(list.implicitHeight, 260)
                contentHeight: list.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Column {
                    id: list
                    width: parent.width
                    spacing: 6
                    Repeater {
                        model: win.notes ? win.notes.changes : []
                        Row {
                            required property string modelData
                            width: list.width
                            spacing: 8
                            Rectangle { width: 5; height: 5; radius: 2.5; color: Theme.accent; y: 7 }
                            Txt { width: parent.width - 13; wrapMode: Text.Wrap; text: modelData; font.pixelSize: 13 }
                        }
                    }
                    Txt {
                        visible: !!win.notes && win.notes.changes.length === 0
                        text: "Small fixes and improvements."
                        color: Theme.muted; font.pixelSize: 13
                    }
                }
            }
            Row {
                spacing: 8
                topPadding: 4
                Chip { icon: "check"; label: "Got it"; on: true; onClicked: win.done() }
            }
        }
    }
}
