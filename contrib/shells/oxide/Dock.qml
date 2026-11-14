// A panel docked to the right edge of the screen, like an app sidebar: windows make room for it (it is not an
// overlay), everything inside takes clicks, and it slides in and out. Used by the assistant.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    property bool shown: false
    property real contentWidth: 440
    property bool wantsKeyboard: false
    default property alias content: box.data
    signal dismissed()

    // 0 = hidden, 1 = fully in; drives the slide and the fade
    property real t: shown ? 1 : 0
    Behavior on t { NumberAnimation { duration: 280; easing.type: win.shown ? Easing.OutCubic : Easing.InCubic } }

    screen: Quickshell.screens[0]
    Connections {
        target: OxideState
        function onActiveScreenChanged() { if (!win.shown && win.t < 0.01) win.screen = OxideState.activeScreen; }
    }
    visible: shown || t > 0.01
    color: "transparent"
    anchors { top: true; bottom: true; right: true }
    implicitWidth: contentWidth + 16
    // windows move aside while it is open (the space is given back when it closes)
    exclusiveZone: shown ? contentWidth + 10 : 0
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.namespace: "oxide-dock"
    WlrLayershell.keyboardFocus: shown && wantsKeyboard ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None

    Item {
        id: box
        width: win.contentWidth
        height: parent.height - 12
        y: 6
        x: 8 + (1 - win.t) * (win.contentWidth + 24)
        opacity: Math.min(1, win.t * 1.4)
        // every click inside lands here, never on the window below
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; onWheel: w => w.accepted = false }
    }

    Shortcut { sequence: "Escape"; enabled: win.shown; onActivated: win.dismissed() }
}
