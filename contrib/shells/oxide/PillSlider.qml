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
    implicitHeight: Theme.expressive ? 40 : 34

    // Expressive (Material 3): filled part, a gap, a tall handle, a gap, the rest; the handle thins while dragged
    Item {
        id: m3
        visible: Theme.expressive
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; right: parent.right; rightMargin: root.showValue ? 46 : 0 }
        readonly property real frac: root.muted ? 0 : root.value
        // the filled part always holds the icon (Material's slider with an inset icon)
        readonly property real start: 44
        readonly property real hx: start + (width - start - 8) * frac
        readonly property real gap: 6
        Segment {
            id: active
            x: 0; height: parent.height - 6
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(0, m3.hx - m3.gap)
            visible: width > 1
            roundLeft: true
            color: root.muted ? Theme.outline : root.fill
            Behavior on width { NumberAnimation { duration: m3drag.pressed ? 0 : Theme.normal; easing.type: Easing.OutCubic } }
        }
        Segment {
            x: m3.hx + m3.gap; height: parent.height - 6
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(0, parent.width - x)
            visible: width > 1
            roundLeft: false
            color: Theme.secondaryContainer
            Behavior on x { NumberAnimation { duration: m3drag.pressed ? 0 : Theme.normal; easing.type: Easing.OutCubic } }
            // the end stop
            Rectangle { width: 4; height: 4; radius: 2; anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter } color: root.fill; opacity: 0.8 }
        }
        Rectangle {
            id: handle
            width: m3drag.pressed ? 2 : 4
            height: parent.height + 8
            radius: width / 2
            anchors.verticalCenter: parent.verticalCenter
            x: m3.hx - width / 2
            color: root.muted ? Theme.outline : root.fill
            Behavior on width { NumberAnimation { duration: Theme.fast } }
            Behavior on x { NumberAnimation { duration: m3drag.pressed ? 0 : Theme.normal; easing.type: Easing.OutCubic } }
        }
        Icon {
            anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
            name: root.icon
            size: 17
            color: root.muted ? Theme.surface : Theme.ink
            MouseArea { anchors.fill: parent; anchors.margins: -8; cursorShape: Qt.PointingHandCursor; onClicked: root.iconClicked() }
        }
        MouseArea {
            id: m3drag
            anchors.fill: parent
            anchors.leftMargin: 38
            cursorShape: Qt.PointingHandCursor
            function set(mx) { root.moved(Math.max(0, Math.min(1, (mx + 38 - m3.start) / (m3.width - m3.start - 8)))); }
            onPressed: m => set(m.x)
            onPositionChanged: m => { if (pressed) set(m.x); }
        }
    }

    Rectangle {
        id: track
        visible: !Theme.expressive
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
        visible: !Theme.expressive
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
