// Rounded background behind one lane of the bar (floating style, or "pill" lanes).
import QtQuick

Rectangle {
    required property Item target
    readonly property var bar: target.window
    readonly property bool onFloating: bar ? bar.floating : false
    readonly property real h: onFloating ? BarConfig.height : Math.max(20, BarConfig.height - 10)
    visible: bar ? bar.pill && target.children.length > 1 : false
    x: target.x - 8
    y: target.y + (target.height - h) / 2
    width: target.width + 16
    height: h
    radius: Math.min(BarConfig.cornerRadius, h / 2)
    color: onFloating ? Theme.alpha(Theme.bg, BarConfig.opacity) : Theme.alpha(Theme.raised, 0.7)
    border.color: Theme.line
}
