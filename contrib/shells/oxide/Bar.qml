// The bar. Layout, size and style come from BarConfig (bar.json); the three lanes hold the modules.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    required property var shell      // the ShellRoot: modules use it to open popups

    readonly property bool top: BarConfig.position !== "bottom"
    readonly property bool floating: BarConfig.style === "floating"
    readonly property bool flat: BarConfig.style === "flat"
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

    LanePill { target: left }
    LanePill { target: center }
    LanePill { target: right }
    Lane {
        id: left
        modules: BarConfig.left
        shell: win.shell
        window: win
        x: BarConfig.inset + (win.flat ? 10 : 8) + (win.pill ? 8 : 0)
        anchors.verticalCenter: parent.verticalCenter
    }
    Lane {
        id: center
        modules: BarConfig.center
        shell: win.shell
        window: win
        anchors.centerIn: parent
    }
    Lane {
        id: right
        modules: BarConfig.right
        shell: win.shell
        window: win
        anchors { right: parent.right; rightMargin: BarConfig.inset + (win.flat ? 10 : 8) + (win.pill ? 8 : 0); verticalCenter: parent.verticalCenter }
    }
}
