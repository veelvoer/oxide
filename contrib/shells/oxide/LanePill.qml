// Rounded background behind one lane of the bar (floating style, "pill" lanes, and every Expressive bar).
// Expressive: a tonal island that follows its lane's size with a soft spring and drops in when the bar appears.
import QtQuick

Rectangle {
    id: pill
    required property Item target
    property real enter: 1
    readonly property var bar: target.window
    readonly property bool onFloating: bar ? bar.floating : false
    readonly property real h: Theme.expressive ? BarConfig.height + 4 : onFloating ? BarConfig.height : Math.max(20, BarConfig.height - 10)
    readonly property real pad: Theme.expressive ? 12 : 8
    visible: bar ? bar.pill && target.children.length > 1 : false
    x: target.x - pad
    y: target.y + (target.height - h) / 2 + (1 - enter) * (bar && !bar.top ? 14 : -14)
    width: target.width + pad * 2
    height: h
    opacity: enter
    radius: Theme.expressive ? h / 2 : Math.min(BarConfig.cornerRadius, h / 2)
    color: Theme.expressive ? Theme.alpha(hover.hovered ? Theme.surfaceHigh : Theme.surface, Math.max(0.9, BarConfig.opacity))
         : onFloating ? Theme.alpha(Theme.bg, BarConfig.opacity) : Theme.alpha(Theme.raised, 0.7)
    border.color: Theme.expressive ? "transparent" : Theme.line
    Behavior on color { ColorAnimation { duration: Theme.normal } }
    Behavior on width { enabled: Theme.expressive; NumberAnimation { duration: Theme.spatial; easing.type: Theme.spatialType; easing.overshoot: 0.8 } }
    Behavior on x { enabled: Theme.expressive; NumberAnimation { duration: Theme.spatial; easing.type: Theme.spatialType; easing.overshoot: 0.8 } }
    HoverHandler { id: hover; enabled: Theme.expressive }
}
