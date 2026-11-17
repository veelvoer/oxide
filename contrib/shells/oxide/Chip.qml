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
    // Expressive (Material filter chip): 8 px corners that round into a pill when chosen, a tonal fill, a check, and
    // a small pop when it gets chosen
    radius: Theme.expressive ? (on ? height / 2 : 8) : height / 2
    Behavior on radius { NumberAnimation { duration: Theme.fastMs; easing.type: Easing.OutBack; easing.overshoot: Theme.fastOvershoot } }
    color: Theme.expressive ? (on ? Theme.secondaryContainer : (mouse.containsMouse ? Theme.alpha(Theme.text, 0.06) : Theme.clear(Theme.text)))
         : on ? Theme.alpha(Theme.accent, 0.2) : (mouse.containsMouse ? Theme.raised : Theme.clear(Theme.raised))
    border.color: Theme.expressive ? (on ? Theme.clear(Theme.line) : Theme.line) : on ? Theme.alpha(Theme.accent, 0.5) : Theme.line
    Behavior on color { ColorAnimation { duration: Theme.fast } }
    Behavior on border.color { ColorAnimation { duration: Theme.fast } }
    scale: (!Theme.expressive && mouse.pressed ? 0.96 : 1) * pop
    Behavior on scale { NumberAnimation { duration: Theme.fast; easing.type: Easing.OutCubic } }
    property real pop: 1
    onOnChanged: if (on && Theme.expressive && Theme.motionScale > 0) popAnim.restart()
    SequentialAnimation {
        id: popAnim
        NumberAnimation { target: root; property: "pop"; to: 0.9; duration: 1 }
        SpringAnimation { target: root; property: "pop"; to: 1; spring: 5; damping: 0.28; epsilon: 0.001 }
    }
    transformOrigin: Item.Center

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: root.icon !== "" || (Theme.expressive && root.on)
            anchors.verticalCenter: parent.verticalCenter
            name: Theme.expressive && root.on ? "check" : root.icon
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
    Elastic { radius: root.radius }
}
