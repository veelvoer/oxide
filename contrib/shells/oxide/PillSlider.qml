// A slider with an icon on the left. The icon also mutes (volume) when clicked.
import QtQuick

Item {
    id: root
    property string icon: ""
    property real value: 0          // 0..1
    property bool muted: false
    // show the number at the right
    property bool showValue: true
    property color fill: Theme.accent
    signal moved(real v)
    signal iconClicked()
    implicitHeight: 34

    Rectangle {
        id: track
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; right: parent.right; rightMargin: root.showValue ? 46 : 0 }
        // a pill when the theme is round, squarer as roundness goes down
        radius: Math.min(height / 2, Theme.roundness * 2.2)
        color: Theme.raised
        Rectangle {
            // its own rounded ends (clipping would cut square corners); never narrower than its radius * 2
            readonly property real frac: root.muted ? 0 : root.value
            height: parent.height
            width: frac <= 0.001 ? 0 : Math.max(height, parent.width * frac)
            radius: track.radius
            color: root.muted ? Theme.line : root.fill
            Behavior on width { NumberAnimation { duration: drag.pressed ? 0 : 140; easing.type: Easing.OutCubic } }
        }
        MouseArea {
            id: drag
            anchors.fill: parent
            anchors.leftMargin: 36
            cursorShape: Qt.PointingHandCursor
            function set(mx) { root.moved(Math.max(0, Math.min(1, (mx + 36) / track.width))); }
            onPressed: m => set(m.x)
            onPositionChanged: m => { if (pressed) set(m.x); }
        }
    }
    Item {
        width: 36; height: parent.height
        Icon {
            anchors.centerIn: parent
            name: root.icon
            size: 17
            color: !root.muted && root.value > 0.07 ? Theme.ink : Theme.text
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.iconClicked() }
    }

    Txt {
        visible: root.showValue
        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
        width: 40
        horizontalAlignment: Text.AlignRight
        text: root.muted ? "Muted" : Math.round(root.value * 100) + "%"
        color: Theme.muted
        font { pixelSize: 12; weight: Font.Medium }
    }
}
