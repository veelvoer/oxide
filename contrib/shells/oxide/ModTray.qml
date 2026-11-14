// System tray icons. Left click activates, right click opens the app's menu.
import Quickshell
import Quickshell.Services.SystemTray
import QtQuick

Row {
    id: root
    property var opts: ({})
    property var shell
    property var window: null
    spacing: 2
    visible: SystemTray.items.values.length > 0

    Repeater {
        model: SystemTray.items
        Rectangle {
            id: slot
            required property var modelData
            width: 28; height: 28
            radius: Theme.rControl
            color: hover.hovered ? Theme.raised : Theme.clear(Theme.raised)
            Image {
                id: img
                anchors.centerIn: parent
                width: 17; height: 17
                visible: status === Image.Ready
                source: slot.modelData.icon
                sourceSize: Qt.size(34, 34)
                smooth: true
            }
            // icon that could not be loaded: a quiet placeholder instead of the checkerboard
            Icon { anchors.centerIn: parent; visible: img.status !== Image.Ready; name: "apps"; size: 16; color: Theme.muted }
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler { acceptedButtons: Qt.LeftButton; onTapped: slot.modelData.onlyMenu ? slot.modelData.display(root.window, slot.x, slot.height) : slot.modelData.activate() }
            TapHandler { acceptedButtons: Qt.RightButton; onTapped: if (slot.modelData.hasMenu) slot.modelData.display(root.window, slot.mapToItem(null, 0, 0).x, slot.height + 8) }
        }
    }
}
