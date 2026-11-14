// An on/off switch. The knob glides; the track takes the accent only while on.
import QtQuick

Rectangle {
    id: root
    property bool checked: false
    signal toggled(bool value)

    implicitWidth: 40
    implicitHeight: 22
    radius: height / 2
    color: checked ? Theme.accent : Theme.raised
    border.color: checked ? Theme.accent : Theme.line
    Behavior on color { ColorAnimation { duration: Theme.normal } }

    Rectangle {
        width: parent.height - 8; height: width; radius: width / 2
        y: 4
        x: root.checked ? parent.width - width - 4 : 4
        color: root.checked ? Theme.ink : Theme.muted
        Behavior on x { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 1.4 } }
        Behavior on color { ColorAnimation { duration: Theme.normal } }
    }
    MouseArea {
        anchors.fill: parent
        anchors.margins: -4
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.checked)
    }
}
