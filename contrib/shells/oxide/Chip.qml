// A small pill button: optional icon, label, an "on" state (the only place the accent shows) and hover.
import QtQuick

Rectangle {
    id: root
    property string icon: ""
    property string label: ""
    property bool on: false
    property bool danger: false
    signal clicked()

    implicitHeight: 30
    implicitWidth: row.implicitWidth + 20
    radius: height / 2
    color: on ? Theme.alpha(Theme.accent, 0.2) : (mouse.containsMouse ? Theme.raised : Theme.clear(Theme.raised))
    border.color: on ? Theme.alpha(Theme.accent, 0.5) : Theme.line
    Behavior on color { ColorAnimation { duration: Theme.fast } }
    Behavior on border.color { ColorAnimation { duration: Theme.fast } }
    scale: mouse.pressed ? 0.96 : 1
    Behavior on scale { NumberAnimation { duration: Theme.fast; easing.type: Easing.OutCubic } }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            size: 15
            color: root.on ? Theme.accent : (root.danger && mouse.containsMouse ? Theme.bad : Theme.muted)
        }
        Txt {
            visible: root.label !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            font { pixelSize: 13; weight: Font.Medium }
            color: root.on ? Theme.text : (root.danger && mouse.containsMouse ? Theme.bad : Theme.text)
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
