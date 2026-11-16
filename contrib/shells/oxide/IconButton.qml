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

    implicitWidth: Theme.expressive ? 34 : 30
    implicitHeight: Theme.expressive ? 32 : 28
    // Expressive: round; pressing squashes it into a rounded square and back (a shape morph)
    radius: Theme.expressive ? (mouse.pressed ? 10 : height / 2) : Theme.rControl
    scale: Theme.expressive && mouse.pressed ? 0.92 : 1
    color: active ? (Theme.expressive ? Theme.container : Theme.alpha(Theme.accent, 0.2))
         : (mouse.containsMouse ? (Theme.expressive ? Theme.surfaceHighest : Theme.raised) : Theme.clear(Theme.raised))
    Behavior on color { ColorAnimation { duration: Theme.fast } }
    Behavior on radius { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 1.6 } }
    Behavior on scale { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 2.0 } }

    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: root.iconSize
        color: root.active ? (Theme.expressive ? Theme.onContainer : Theme.accent) : (mouse.containsMouse ? Theme.text : root.tint)
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
