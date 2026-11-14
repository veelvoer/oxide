// Square-ish icon button: hover fill, optional highlighted state. No bounce.
import QtQuick

Rectangle {
    id: root
    property string icon: ""
    property bool active: false
    property color tint: Theme.muted
    property real iconSize: 17
    signal clicked()
    signal rightClicked()

    implicitWidth: 30
    implicitHeight: 28
    radius: Theme.rControl
    color: active ? Theme.alpha(Theme.accent, 0.2) : (mouse.containsMouse ? Theme.raised : Theme.clear(Theme.raised))
    Behavior on color { ColorAnimation { duration: Theme.fast } }

    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: root.iconSize
        color: root.active ? Theme.accent : (mouse.containsMouse ? Theme.text : root.tint)
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => m.button === Qt.RightButton ? root.rightClicked() : root.clicked()
    }
}
