// A bar that is fully round at one end and only slightly round at the other (Expressive slider parts).
// Built from two rectangles, so it works on every Qt 6 (per-corner radii need Qt 6.7).
import QtQuick

Item {
    id: seg
    property color color: Theme.accent
    // which end is the round one
    property bool roundLeft: true
    readonly property real cap: Math.min(height, width)
    Rectangle {
        width: seg.cap; height: seg.height; radius: height / 2
        x: seg.roundLeft ? 0 : seg.width - width
        color: seg.color
    }
    Rectangle {
        visible: seg.width > seg.height / 2
        x: seg.roundLeft ? seg.height / 2 : 0
        width: seg.width - seg.height / 2
        height: seg.height
        radius: Math.min(4, height / 2)
        color: seg.color
    }
}
