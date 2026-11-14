// A popup panel: a transparent full-screen layer that closes when you click outside, with a content box
// that fades in and slides a few pixels. Put your content inside.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    property bool shown: false
    // "top-right" (next to the bar), "top-center", "center" or "right" (full-height side panel)
    property string place: "top-right"
    property real contentWidth: 360
    property real contentHeight: 300
    // place "window": hangs from the top edge of a window, centred on anchorX (screen coordinates)
    property real anchorX: 0
    property real anchorY: 0
    property bool wantsKeyboard: false
    // modal: a click outside closes it. Not modal (the assistant): the rest of the screen keeps working under it
    property bool modal: true
    // close when the pointer leaves the box (after it has been inside once): quick settings, calendar
    property bool closeOnLeave: false
    property bool entered: false
    // set by the shell when a click in the bar opened it: if the pointer never comes onto it, it goes away
    property bool mouseOpened: false
    default property alias content: box.data
    signal dismissed()

    readonly property bool barOnTop: BarConfig.position !== "bottom"

    screen: Quickshell.screens[0]
    // follows the active monitor while closed; once open it stays where it is
    Connections {
        target: OxideState
        function onActiveScreenChanged() { if (!win.shown && box.opacity < 0.01) win.screen = OxideState.activeScreen; }
    }
    // unmapped while hidden, so a hidden popup can never sit over the visible one (the compositor tolerates the
    // destroy now; it used to drop clients that did that)
    visible: shown || box.opacity > 0.01
    color: "transparent"
    mask: modal ? null : boxRegion
    Region { id: boxRegion; item: box }
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "oxide-popup"
    WlrLayershell.keyboardFocus: shown && wantsKeyboard ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None

    // "is the pointer on the box?": every pointer move is checked against the box rectangle, from two sources
    // (the layer's own mouse area and a hover handler) so that child items that take hover events cannot hide it
    property bool pointerInside: false
    function track(x, y) {
        if (!shown) return;
        const m = 6;
        const inside = x >= box.x - m && x <= box.x + box.width + m && y >= box.y - m && y <= box.y + box.height + m;
        pointerInside = inside;
        if (inside) { entered = true; leaveTimer.stop(); }
        else if (closeOnLeave && entered && !leaveTimer.running) leaveTimer.start();
    }
    Timer {
        id: leaveTimer
        interval: 500
        onTriggered: if (win.shown && !win.pointerInside) win.dismissed()
    }
    HoverHandler { onPointChanged: win.track(point.position.x, point.position.y) }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        onPositionChanged: m => win.track(m.x, m.y)
        onClicked: if (win.modal) win.dismissed()
    }

    Item {
        id: box
        width: win.contentWidth
        height: win.place === "right" ? parent.height - BarConfig.barSpace - 12 : win.contentHeight
        x: win.place === "window" ? Math.max(8, Math.min(parent.width - width - 8, win.anchorX - width / 2))
           : win.place === "center" || win.place === "top-center" ? (parent.width - width) / 2 : parent.width - width - 8
        y: win.place === "window" ? Math.max(BarConfig.barSpace + 6, Math.min(parent.height - height - 8, win.anchorY))
           : win.place === "center" ? Math.max(60, (parent.height - height) / 3)
           : (win.barOnTop ? BarConfig.barSpace + 6 : parent.height - height - BarConfig.barSpace - 6)

        opacity: win.shown ? 1 : 0
        // it grows out of the corner it hangs from (the bar button you clicked) and settles with a tiny overshoot
        transformOrigin: win.place === "top-center" || win.place === "window" ? Item.Top : win.place === "center" ? Item.Center
                       : win.place === "right" ? Item.Right : (win.barOnTop ? Item.TopRight : Item.BottomRight)
        scale: win.shown ? 1 : 0.9
        Behavior on opacity { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic } }
        Behavior on scale { NumberAnimation { duration: 280; easing.type: win.shown ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
        // when the content changes size (a list opens, results come in) the panel morphs to the new size
        Behavior on height { enabled: win.shown; NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }

        // clicks inside the box must not close it
        MouseArea { anchors.fill: parent }

    }

    onShownChanged: { if (!shown) { entered = false; pointerInside = false; leaveTimer.stop(); } }

    Timer {
        interval: 2200
        running: win.shown && win.closeOnLeave && win.mouseOpened && !win.entered
        onTriggered: win.dismissed()
    }

    Shortcut { sequence: "Escape"; enabled: win.shown && win.modal; onActivated: win.dismissed() }
}
