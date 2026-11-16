// The bar. Layout, size and style come from BarConfig (bar.json); the three lanes hold the modules.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    required property var shell      // the ShellRoot: modules use it to open popups

    readonly property bool top: BarConfig.position !== "bottom"
    readonly property bool floating: BarConfig.look === "floating"
    readonly property bool flat: BarConfig.look === "flat"
    readonly property bool pill: floating || BarConfig.lanes === "pill"

    anchors { left: true; right: true; top: top; bottom: !top }
    implicitHeight: BarConfig.height + BarConfig.inset * 2
    color: "transparent"
    WlrLayershell.namespace: "oxide-bar"
    visible: !BarConfig.hidden

    // one background for flat and island styles
    Rectangle {
        visible: !win.floating
        anchors { fill: parent; margins: BarConfig.inset }
        radius: win.flat ? 0 : BarConfig.cornerRadius
        color: Theme.alpha(Theme.bg, BarConfig.opacity)
        border.color: win.flat ? "transparent" : Theme.line
        border.width: 1
        // flat bars get a single hairline on the edge facing the desktop
        Rectangle {
            visible: win.flat
            x: 0; width: parent.width; height: 1
            y: win.top ? parent.height - 1 : 0
            color: Theme.line
        }
    }

    // Expressive: the islands drop in one after another when the bar appears
    property real arrive: 0
    Component.onCompleted: arrive = 1
    Behavior on arrive { NumberAnimation { duration: Theme.expressive ? 700 * Theme.motionScale : 0; easing.type: Easing.Linear } }
    function drop(i) { return Theme.expressive ? Math.max(0, Math.min(1, arrive * 2.2 - i * 0.45)) : 1; }

    LanePill { target: left; enter: win.drop(0) }
    LanePill { target: center; enter: win.drop(1) }
    LanePill { target: right; enter: win.drop(2) }
    Lane {
        id: left
        modules: BarConfig.left
        readonly property real e: win.drop(0)
        opacity: e
        transform: Translate { y: (1 - left.e) * (win.top ? -14 : 14) }
        shell: win.shell
        window: win
        x: BarConfig.inset + (win.flat ? 10 : 8) + (win.pill ? 8 : 0)
        anchors.verticalCenter: parent.verticalCenter
    }
    Lane {
        id: center
        modules: BarConfig.center
        readonly property real e: win.drop(1)
        opacity: e
        transform: Translate { y: (1 - center.e) * (win.top ? -14 : 14) }
        shell: win.shell
        window: win
        anchors.centerIn: parent
    }
    Lane {
        id: right
        modules: BarConfig.right
        readonly property real e: win.drop(2)
        opacity: e
        transform: Translate { y: (1 - right.e) * (win.top ? -14 : 14) }
        shell: win.shell
        window: win
        anchors { right: parent.right; rightMargin: BarConfig.inset + (win.flat ? 10 : 8) + (win.pill ? 8 : 0); verticalCenter: parent.verticalCenter }
    }
}
