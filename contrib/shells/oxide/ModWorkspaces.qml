// Workspaces. Oxide has unlimited ones; only those with windows (and the current one) exist.
// options: style "numbers" | "dots" | "buttons" | "bars" | "scroll" | "stretch"
//          count (how many entries show at once, bars/scroll/stretch)   showEmpty (bool)
//          highlightDelay (ms the highlight takes to follow a switch; 0 = it follows at once)
//
// numbers: plain digits with an underline under the current one    dots: dots, the current one wide with a glow
// buttons: filled pills with the digit                              bars: a carousel of ticks around the current one
// scroll: a window of numbers that slides along                      stretch: dots with a ring that stretches to the next one
// The carousels are driven by one smoothly moving position, so a switch glides and can be interrupted at any moment.
import Quickshell
import QtQuick

Item {
    id: root
    property var opts: ({})
    property var shell
    readonly property string kind: ["numbers", "dots", "buttons", "bars", "scroll", "stretch"].indexOf(opts.style) >= 0 ? opts.style : "numbers"
    readonly property bool carousel: kind === "bars" || kind === "scroll" || kind === "stretch"
    readonly property bool showEmpty: opts.showEmpty === true
    // the time the highlight takes to follow a switch; 0 = at once
    readonly property int hl: Math.max(0, opts.highlightDelay !== undefined ? opts.highlightDelay : 380) * Theme.motionScale
    readonly property int slots: Math.max(2, Math.min(12, opts.count !== undefined ? opts.count : 5))
    readonly property real unit: 1

    readonly property string screenName: QsWindow.window && QsWindow.window.screen ? QsWindow.window.screen.name : ""
    readonly property int shownHere: OxideState.workspaceOn(screenName)
    readonly property int active: Math.max(0, shownHere - 1)
    readonly property int highest: {
        let h = 0;
        for (const w of OxideState.workspaces) if (!w.special) h = Math.max(h, w.id);
        return h;
    }
    function has(i) { return !!OxideState.workspaces.find(w => w.id === i + 1 && (w.windows > 0 || w.visible)); }
    function go(i) { OxideState.workspace(i + 1); }

    implicitHeight: 26
    implicitWidth: carousel ? (kind === "stretch" ? slots * 18 : slots * (kind === "scroll" ? 20 : 18)) : row.implicitWidth
    clip: carousel

    // ------------------------------------------------------------ the position everything follows
    property real pos: 0
    Behavior on pos { NumberAnimation { duration: root.hl; easing.type: Easing.OutCubic } }
    // stretch: the head of the ring leads and the tail follows, so the ring stretches while it moves
    property real head: 0
    property real tail: 0
    Behavior on head { NumberAnimation { duration: root.hl * 0.63; easing.type: Easing.OutCubic } }
    Behavior on tail { NumberAnimation { duration: root.hl * 1.2; easing.type: Easing.OutCubic } }
    onActiveChanged: { pos = active; head = active; tail = active; }
    Component.onCompleted: { pos = active; head = active; tail = active; Qt.callLater(refreshCurrent); }

    function near(i) { return Math.max(0, 1 - Math.abs(i - pos)); }

    // ------------------------------------------------------------ bars + scroll: a window sliding along the line
    readonly property real step: (kind === "scroll" ? 20 : 18) * unit
    readonly property real half: (slots - 1) / 2
    readonly property int lineCount: Math.max(slots, highest, active + 1) + slots
    readonly property real centre: Math.max(half, pos)
    function fade(i) { return Math.max(0, Math.min(1, slots / 2 + 0.2 - Math.abs(i - centre))); }

    Repeater {
        model: root.kind === "bars" || root.kind === "scroll" ? root.lineCount : 0
        delegate: Item {
            id: cell
            required property int index
            readonly property real nr: root.near(index)
            x: root.width / 2 + (index - root.centre) * root.step - root.step / 2
            width: root.step; height: root.height
            opacity: root.fade(index) * (root.has(index) || nr > 0.3 ? 1 : 0.5)
            visible: opacity > 0.01

            Rectangle {
                visible: root.kind === "bars"
                anchors.centerIn: parent
                width: 2 + cell.nr * 1.2; height: 6 + cell.nr * 9; radius: width / 2
                color: Qt.tint(root.has(index) ? Theme.text : Theme.muted, Theme.alpha(Theme.accent, cell.nr))
            }
            Txt {
                visible: root.kind === "scroll"
                anchors.centerIn: parent
                text: cell.index + 1
                color: Qt.tint(root.has(index) ? Theme.text : Theme.muted, Theme.alpha(Theme.accent, cell.nr * 0.9))
                font { family: Theme.mono; pixelSize: Math.round(13 * (0.66 + 0.42 * cell.nr)); weight: cell.nr > 0.5 ? Font.Bold : Font.Medium }
            }
            TapHandler { onTapped: root.go(cell.index) }
            HoverHandler { cursorShape: Qt.PointingHandCursor }
        }
    }

    // ------------------------------------------------------------ stretch
    Item {
        visible: root.kind === "stretch"
        anchors.fill: parent
        readonly property real first: Math.max(0, Math.min(Math.max(1, root.slots, root.highest, root.active + 1) - root.slots, Math.round(root.active) - Math.floor(root.slots / 2)))
        function cx(i) { return (i - first + 0.5) * 18; }
        readonly property real h: 10

        // the line left behind the ring
        Rectangle {
            readonly property real x0: Math.max(0, parent.cx(0))
            x: x0; y: (parent.height - height) / 2
            width: Math.max(0, parent.cx(root.head) - x0); height: 1.5; radius: 1
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: parent.parent.first > 0 ? "transparent" : Theme.alpha(Theme.accent, 0.75) }
                GradientStop { position: Math.min(0.9, 22 / Math.max(22, parent.width)); color: Theme.alpha(Theme.accent, 0.75) }
                GradientStop { position: 1; color: Theme.alpha(Theme.accent, 0.75) }
            }
        }
        Repeater {
            model: root.kind === "stretch" ? root.slots : 0
            delegate: Rectangle {
                required property int index
                readonly property int ws: index + parent.first
                readonly property bool lit: root.has(ws)
                x: parent.cx(ws) - width / 2; y: (parent.height - height) / 2
                width: lit ? 5 : 4; height: width; radius: width / 2
                color: ws <= root.head + 0.01 ? Theme.accent : (lit ? Theme.text : Theme.muted)
                opacity: lit || ws <= root.head ? 1 : 0.6
                Behavior on color { ColorAnimation { duration: root.hl * 0.5 } }
            }
        }
        // the ring, stretched between tail and head
        Rectangle {
            readonly property real a: Math.min(root.tail, root.head)
            readonly property real b: Math.max(root.tail, root.head)
            x: parent.cx(a) - parent.h / 2; y: (parent.height - height) / 2
            width: parent.cx(b) - parent.cx(a) + parent.h; height: parent.h; radius: height / 2
            color: Theme.alpha(Theme.bg, 0.9)
            border.width: 1.5; border.color: Theme.accent
        }
        Repeater {
            model: root.kind === "stretch" ? root.slots : 0
            delegate: Item {
                required property int index
                x: index * 18; width: 18; height: parent.height
                TapHandler { onTapped: root.go(index + parent.first) }
                HoverHandler { cursorShape: Qt.PointingHandCursor }
            }
        }
    }

    // ------------------------------------------------------------ numbers, dots, buttons: one button per workspace
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

    Row {
        id: row
        visible: !root.carousel
        spacing: root.kind === "dots" ? 6 : 2
        height: parent.height

        Repeater {
            id: reps
            model: root.carousel ? [] : OxideState.workspaces.filter(w => root.showEmpty || w.windows > 0 || w.visible)
            onItemAdded: Qt.callLater(root.refreshCurrent)
            onItemRemoved: Qt.callLater(root.refreshCurrent)

            Item {
                id: ws
                required property var modelData
                readonly property bool current: modelData.id === root.shownHere
                readonly property bool occupied: modelData.windows > 0
                anchors.verticalCenter: parent ? parent.verticalCenter : undefined
                height: 26
                // the current one is wider (dots: a pill), the others small
                width: root.kind === "dots" ? (current ? 24 : 12) : (current ? 30 : 26)
                Behavior on width { NumberAnimation { duration: root.hl * 0.7; easing.type: Easing.OutCubic } }
                // arriving workspaces pop in
                opacity: 0
                Component.onCompleted: opacity = 1
                Behavior on opacity { NumberAnimation { duration: root.hl > 0 ? 200 : 0 } }

                // buttons: a rounded square behind the digit (the highlight slides over the current one)
                Rectangle {
                    visible: root.kind === "buttons"
                    anchors.fill: parent
                    radius: Theme.rControl
                    color: !ws.current && hover.hovered ? Theme.raised : Theme.clear(Theme.raised)
                    Behavior on color { ColorAnimation { duration: Theme.fast } }
                }
                Txt {
                    visible: root.kind === "numbers" || root.kind === "buttons"
                    anchors.centerIn: parent
                    text: ws.modelData.special ? "S" : ws.modelData.id
                    color: root.kind === "buttons" ? (ws.current ? Theme.ink : (ws.occupied ? Theme.text : Theme.muted)) : (ws.current ? Theme.accent : (ws.occupied ? Theme.text : Theme.muted))
                    Behavior on color { ColorAnimation { duration: root.hl * 0.6 } }
                    font { family: Theme.mono; pixelSize: 13; weight: ws.current ? Font.Bold : Font.Normal }
                }
                // numbers: the underline grows under the current one
                Rectangle {
                    visible: root.kind === "numbers"
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 1
                    anchors.horizontalCenter: parent.horizontalCenter
                    height: 2; radius: 1; color: Theme.accent
                    width: ws.current ? parent.width - 8 : 0
                    Behavior on width { NumberAnimation { duration: root.hl * 0.6; easing.type: Easing.OutCubic } }
                }
                // dots: a dot that rests in its own color; the current one is the glowing pill (the marker)
                Rectangle {
                    visible: root.kind === "dots" && !ws.current
                    anchors.centerIn: parent
                    width: 8; height: 8; radius: 4
                    color: ws.occupied ? Theme.text : Theme.line
                    Behavior on color { ColorAnimation { duration: root.hl * 0.5 } }
                }
                HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: ws.modelData.special ? OxideState.dispatch({ "type": "toggle_special" }) : OxideState.workspace(ws.modelData.id) }
            }
        }
    }

    // the sliding highlight of buttons and dots, laid out by hand over the row
    Rectangle {
        id: marker
        z: -1
        visible: (root.kind === "buttons" || root.kind === "dots") && !!root.cur
        x: root.cur ? root.cur.x + (root.cur.width - width) / 2 : 0
        y: root.cur ? root.cur.y + (root.cur.height - height) / 2 : 0
        width: root.cur ? (root.kind === "buttons" ? root.cur.width : root.cur.width - 2) : 0
        height: root.kind === "buttons" ? 26 : 8
        radius: root.kind === "buttons" ? Math.min(Theme.rControl, height / 2) : height / 2
        color: Theme.accent
        Behavior on x { NumberAnimation { duration: root.hl; easing.type: Easing.OutCubic } }
        Behavior on width { NumberAnimation { duration: root.hl; easing.type: Easing.OutCubic } }
    }
    WheelHandler { onWheel: e => OxideState.dispatch({ "type": "workspace_existing", "arg": e.angleDelta.y < 0 ? 1 : -1 }) }
}
