// An on/off switch. The knob glides; the track takes the accent only while on.
// Expressive: the Material 3 switch: an outlined track with a small knob when off; when on, the knob grows and
// shows a check; pressing makes it bigger still.
import QtQuick

Rectangle {
    id: root
    property bool checked: false
    signal toggled(bool value)

    readonly property bool m3: Theme.expressive
    implicitWidth: m3 ? 52 : 40
    implicitHeight: m3 ? 32 : 22
    radius: height / 2
    color: checked ? Theme.accent : (m3 ? Theme.surfaceHighest : Theme.raised)
    border.width: m3 ? 2 : 1
    border.color: checked ? Theme.accent : (m3 ? Theme.outline : Theme.line)
    Behavior on color { ColorAnimation { duration: Theme.normal } }
    Behavior on border.color { ColorAnimation { duration: Theme.normal } }

    Rectangle {
        id: knob
        property real size: !root.m3 ? root.height - 8 : press.pressed ? 28 : root.checked ? 24 : 16
        width: size; height: size; radius: size / 2
        anchors.verticalCenter: parent.verticalCenter
        x: root.checked ? parent.width - (root.height + size) / 2 : (root.height - size) / 2
        color: root.checked ? Theme.ink : (root.m3 ? Theme.outline : Theme.muted)
        Behavior on x { NumberAnimation { duration: Theme.spatial; easing.type: Easing.OutBack; easing.overshoot: root.m3 ? 1.5 : 1.4 } }
        Behavior on size { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 2.0 } }
        Behavior on color { ColorAnimation { duration: Theme.normal } }
        Icon {
            visible: root.m3
            anchors.centerIn: parent
            name: "check"
            size: 14
            color: Theme.accent
            opacity: root.checked ? 1 : 0
            scale: root.checked ? 1 : 0.4
            Behavior on opacity { NumberAnimation { duration: Theme.fast } }
            Behavior on scale { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 2.2 } }
        }
    }
    MouseArea {
        id: press
        anchors.fill: parent
        anchors.margins: -4
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.checked)
    }
}
