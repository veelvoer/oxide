// Notification history: everything that came in, newest first, with do-not-disturb and clear all.
import Quickshell
import QtQuick

Pop {
    id: root
    required property var shell
    closeOnLeave: true
    place: "top-right"
    contentWidth: 380
    contentHeight: 56 + Math.min(7, Math.max(list.count, 1)) * 70 + 12

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        Txt { x: 16; y: 16; text: "Notifications"; color: Theme.text; font { pixelSize: 15; weight: Font.DemiBold } }
        Row {
            anchors { right: parent.right; rightMargin: 12; top: parent.top; topMargin: 10 }
            spacing: 2
            IconButton {
                icon: root.shell.dnd ? "bell-off-outline" : "bell-outline"
                active: root.shell.dnd
                onClicked: root.shell.dnd = !root.shell.dnd
            }
            IconButton { icon: "delete-outline"; onClicked: root.shell.clearNotifications() }
        }
        Rectangle { x: 12; y: 48; width: parent.width - 24; height: 1; color: Theme.line }

        ListView {
            id: list
            x: 12; y: 56
            width: parent.width - 24
            height: Math.min(7, Math.max(count, 1)) * 70
            clip: true
            model: root.shell.allNotifications
            delegate: Item {
                required property var modelData
                width: list.width
                height: 70
                Column {
                    x: 4; y: 8
                    width: parent.width - 8 - 34
                    spacing: 2
                    Txt { text: modelData.appName; color: Theme.muted; font.pixelSize: 11 }
                    Txt { width: parent.width; elide: Text.ElideRight; text: modelData.summary; color: Theme.text; font { pixelSize: 13; weight: Font.DemiBold } }
                    Txt { visible: modelData.body !== ""; width: parent.width; elide: Text.ElideRight; text: modelData.body.replace(/\s+/g, " "); color: Theme.muted; font.pixelSize: 12 }
                }
                IconButton {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    icon: "close"
                    onClicked: root.shell.dismissNotification(modelData.uid)
                }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.alpha(Theme.line, 0.6) }
            }
        }
        Txt { visible: list.count === 0; x: 16; y: 68; text: root.shell.dnd ? "Do not disturb is on." : "Nothing new."; color: Theme.muted; font.pixelSize: 13 }
    }
}
