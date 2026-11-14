// Workspaces. Oxide has unlimited ones; only those with windows (and the current one) exist.
// options: style "numbers" | "dots" | "bars"   showEmpty (bool)
// The highlight is one shape that glides from workspace to workspace and changes size on the way.
import Quickshell
import QtQuick

Item {
    id: root
    property var opts: ({})
    property var shell
    readonly property string kind: opts.style || "numbers"
    readonly property bool showEmpty: opts.showEmpty === true
    implicitWidth: row.implicitWidth
    implicitHeight: 26

    // the item of the current workspace, for the sliding highlight
    property Item cur: null
    function refreshCurrent() {
        cur = null;
        for (let i = 0; i < reps.count; i++) {
            const it = reps.itemAt(i);
            if (it && it.current) { cur = it; break; }
        }
    }
    Connections { target: OxideState; function onActiveChanged() { Qt.callLater(root.refreshCurrent); } function onWorkspacesChanged() { Qt.callLater(root.refreshCurrent); } }
    Component.onCompleted: Qt.callLater(refreshCurrent)

    readonly property string screenName: QsWindow.window && QsWindow.window.screen ? QsWindow.window.screen.name : ""
    readonly property int shownHere: OxideState.workspaceOn(screenName)

    Row {
        id: row
        spacing: root.kind === "numbers" ? 2 : 6
        height: parent.height

    Repeater {
        id: reps
        model: OxideState.workspaces.filter(w => root.showEmpty || w.windows > 0 || w.visible)
        onItemAdded: Qt.callLater(root.refreshCurrent)
        onItemRemoved: Qt.callLater(root.refreshCurrent)

        Item {
            id: ws
            required property var modelData
            readonly property bool current: modelData.id === root.shownHere
            readonly property bool occupied: modelData.windows > 0
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
            height: 26
            width: root.kind === "numbers" ? 26 : root.kind === "dots" ? 12 : (current ? 26 : 14)
            Behavior on width { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
            // arriving workspaces pop in, leaving ones shrink away
            opacity: 0; scale: 0.6
            Component.onCompleted: { opacity = 1; scale = 1; }
            Behavior on opacity { NumberAnimation { duration: 200 } }
            Behavior on scale { NumberAnimation { duration: 260; easing.type: Easing.OutBack } }

            // hover tint under the digit
            Rectangle {
                visible: root.kind === "numbers"
                anchors.fill: parent
                radius: Theme.rControl
                color: !ws.current && hover.hovered ? Theme.raised : Theme.clear(Theme.raised)
                Behavior on color { ColorAnimation { duration: Theme.fast } }
            }
            Txt {
                visible: root.kind === "numbers"
                anchors.centerIn: parent
                text: ws.modelData.special ? "S" : ws.modelData.id
                color: ws.current ? Theme.ink : (ws.occupied ? Theme.text : Theme.muted)
                Behavior on color { ColorAnimation { duration: 160 } }
                font { family: Theme.mono; pixelSize: 13; weight: ws.current ? Font.Medium : Font.Normal }
            }
            // dots and bars rest in their own colors; the highlight shape slides over the current one
            Rectangle {
                visible: root.kind === "dots"
                anchors.centerIn: parent
                width: 8; height: 8; radius: 4
                color: ws.occupied ? Theme.text : Theme.line
                Behavior on color { ColorAnimation { duration: 160 } }
            }
            Rectangle {
                visible: root.kind === "bars"
                anchors.centerIn: parent
                width: parent.width; height: 4; radius: 2
                color: ws.occupied ? Theme.muted : Theme.line
                Behavior on color { ColorAnimation { duration: 160 } }
            }
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: ws.modelData.special ? OxideState.dispatch({ "type": "toggle_special" }) : OxideState.workspace(ws.modelData.id) }
        }
    }

    }

    // the sliding highlight, laid out by hand over the row
    Rectangle {
        id: marker
        z: -1
        visible: !!root.cur
        // numbers: a rounded square behind the digit; dots: a dot; bars: a bar
        x: root.cur ? root.cur.x + (root.cur.width - width) / 2 : 0
        y: root.cur ? root.cur.y + (root.cur.height - height) / 2 : 0
        width: root.cur ? (root.kind === "numbers" ? root.cur.width : root.kind === "dots" ? 10 : root.cur.width) : 0
        height: root.kind === "numbers" ? 26 : root.kind === "dots" ? 10 : 4
        radius: root.kind === "numbers" ? Theme.rControl : height / 2
        color: Theme.accent
        Behavior on x { NumberAnimation { duration: 320; easing.type: Easing.OutBack; easing.overshoot: 0.9 } }
        Behavior on width { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
    }
    WheelHandler { onWheel: e => OxideState.dispatch({ "type": "workspace_existing", "arg": e.angleDelta.y < 0 ? 1 : -1 }) }
}
