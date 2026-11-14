// Notifications: shows a dot when there are unread ones, opens the notification list.
import QtQuick

Item {
    property var opts: ({})
    property var shell
    implicitWidth: 30
    implicitHeight: 28
    IconButton {
        anchors.fill: parent
        icon: shell && shell.dnd ? "bell-off-outline" : "bell-outline"
        tint: shell && shell.notificationCount > 0 ? Theme.text : Theme.muted
        onClicked: shell.toggleNotifications(true)
    }
    Rectangle {
        visible: shell && shell.notificationCount > 0 && !shell.dnd
        x: 18; y: 5
        width: 7; height: 7; radius: 3.5
        color: Theme.accent
    }
}
