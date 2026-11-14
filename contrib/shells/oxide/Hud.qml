// The ambient HUD: hold Super on its own and it fades in at the top and bottom edges (time, workspaces, status,
// key hints); let go, or press a shortcut, and it is gone. With the bar hidden ("zero bar") it is the bar.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    readonly property bool on: BarConfig.hud && OxideState.superHeld
    property real t: on ? 1 : 0
    Behavior on t { NumberAnimation { duration: win.on ? 140 : 90; easing.type: Easing.OutCubic } }

    screen: Quickshell.screens[0]
    Connections {
        target: OxideState
        function onActiveScreenChanged() { if (win.t < 0.01) win.screen = OxideState.activeScreen; }
    }
    visible: on || t > 0.01
    color: "transparent"
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    mask: Region {}                       // never takes clicks
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "oxide-hud"

    property date now: new Date()
    Timer { running: win.visible; repeat: true; interval: 1000; triggeredOnStart: true; onTriggered: win.now = new Date() }

    component Glass: Rectangle {
        radius: Theme.rPop
        color: Theme.alpha(Theme.bg, 0.78)
        border.color: Theme.alpha(Theme.line, 0.9)
    }

    // ---- top: time, workspaces, status ----
    Glass {
        id: top
        anchors.horizontalCenter: parent.horizontalCenter
        y: 14 - (1 - win.t) * 18
        opacity: win.t
        width: topRow.implicitWidth + 36
        height: 64
        Row {
            id: topRow
            anchors.centerIn: parent
            spacing: 22
            Column {
                anchors.verticalCenter: parent.verticalCenter
                Txt { text: Qt.formatTime(win.now, "HH:mm"); font { family: Theme.mono; pixelSize: 26; weight: Font.DemiBold } }
                Txt { text: Qt.formatDate(win.now, "dddd d MMMM"); color: Theme.muted; font.pixelSize: 12 }
            }
            Rectangle { width: 1; height: 36; color: Theme.line; anchors.verticalCenter: parent.verticalCenter }
            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                Repeater {
                    model: OxideState.workspaces.filter(w => !w.special)
                    Rectangle {
                        required property var modelData
                        readonly property bool cur: modelData.id === OxideState.active
                        width: 30; height: 30; radius: 15
                        color: cur ? Theme.accent : (modelData.windows > 0 ? Theme.raised : Theme.clear(Theme.raised))
                        border.color: cur ? Theme.accent : Theme.line
                        Txt { anchors.centerIn: parent; text: String(parent.modelData.id); color: parent.cur ? Theme.ink : Theme.text; font { pixelSize: 13; weight: Font.DemiBold } }
                    }
                }
            }
            Rectangle { width: 1; height: 36; color: Theme.line; anchors.verticalCenter: parent.verticalCenter }
            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                Icon { anchors.verticalCenter: parent.verticalCenter; name: Sys.wifiIcon; size: 18 }
                Icon { anchors.verticalCenter: parent.verticalCenter; name: Sys.volumeIcon; size: 18 }
                Row {
                    visible: Sys.hasBattery
                    spacing: 4
                    anchors.verticalCenter: parent.verticalCenter
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: Sys.batteryIcon; size: 18 }
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: Sys.batteryPct + "%"; font.pixelSize: 13 }
                }
            }
        }
    }

    // ---- focused window ----
    Txt {
        anchors.horizontalCenter: parent.horizontalCenter
        y: top.y + top.height + 10
        opacity: win.t * 0.9
        visible: !!OxideState.focused
        text: OxideState.focused ? (OxideState.focused.title || OxideState.focused.app_id) : ""
        color: Theme.text
        font.pixelSize: 13
        style: Text.Outline; styleColor: Theme.alpha(Theme.bg, 0.6)
    }

    // ---- bottom: key hints ----
    Glass {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height - height - 16 + (1 - win.t) * 18
        opacity: win.t
        width: hints.implicitWidth + 28
        height: 46
        Row {
            id: hints
            anchors.centerIn: parent
            spacing: 14
            Repeater {
                model: [["Q", "Terminal"], ["R", "Apps"], ["O", "Window"], ["Tab", "Overview"], ["Z", "Quick settings"], [",", "Settings"], ["A", "Assistant"], ["1-9", "Workspaces"]]
                Row {
                    required property var modelData
                    spacing: 6
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.max(22, keyTxt.implicitWidth + 12); height: 22; radius: 6
                        color: Theme.raised; border.color: Theme.line
                        Txt { id: keyTxt; anchors.centerIn: parent; text: parent.parent.modelData[0]; font { family: Theme.mono; pixelSize: 12; weight: Font.DemiBold } }
                    }
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData[1]; color: Theme.muted; font.pixelSize: 12 }
                }
            }
        }
    }
}
