// The expressive press (as in Hearth): the surface leans toward the pointer and stretches a little that way, a soft
// light follows the pointer and a ring spreads from where it was pressed; on release it springs back past its rest
// and settles. Only in the Material Expressive style, and never while motion is off.
// Put it inside the item that should react: `Elastic { radius: 16 }`. It only watches presses (the item's own
// MouseArea or handlers still get the click).
import QtQuick
import QtQuick.Shapes

Item {
    id: e
    anchors.fill: parent
    property Item target: parent
    property real radius: 16
    // how far the surface leans, in pixels, and how much it stretches toward the pointer
    property real travel: 2
    property real stretch: 0.06
    readonly property bool on: Theme.expressive && Theme.motionScale > 0
    readonly property color light: Theme.light ? Theme.accent : "white"

    property real dx: 0
    property real dy: 0
    property real gx: width / 2
    property real gy: height / 2
    // a big card gives a little, a small button more: the same few pixels of travel either way
    readonly property real squeeze: Math.max(0.94, Math.min(0.985, 1 - 10 / Math.max(1, Math.max(width, height))))
    property real sq: ph.active ? squeeze : 1
    property real glow: ph.active ? 1 : 0

    function lean(p) {
        const cx = width / 2, cy = height / 2;
        if (cx <= 0 || cy <= 0) return;
        dx = Math.max(-1, Math.min(1, (p.x - cx) / cx));
        dy = Math.max(-1, Math.min(1, (p.y - cy) / cy));
        gx = p.x; gy = p.y;
    }

    // following the pointer: firm; letting go: springy, past the rest and back
    Behavior on dx { SpringAnimation { spring: ph.active ? 6 : 3.4; damping: ph.active ? 0.62 : 0.2; epsilon: 0.002 } }
    Behavior on dy { SpringAnimation { spring: ph.active ? 6 : 3.4; damping: ph.active ? 0.62 : 0.2; epsilon: 0.002 } }
    Behavior on sq { SpringAnimation { spring: ph.active ? 9 : 3.6; damping: ph.active ? 0.8 : 0.24; epsilon: 0.0005 } }
    Behavior on glow { NumberAnimation { duration: ph.active ? 120 : 280; easing.type: Easing.OutCubic } }

    PointHandler {
        id: ph
        enabled: e.on
        acceptedButtons: Qt.LeftButton
        onActiveChanged: {
            if (active) { e.lean(point.position); ring.go(point.position.x, point.position.y); }
            else { e.dx = 0; e.dy = 0; }
        }
        onPointChanged: if (active) e.lean(point.position)
    }

    property Scale sc: Scale {
        origin.x: e.target ? e.target.width * Math.max(0.15, Math.min(0.85, 0.5 - 0.3 * e.dx)) : 0
        origin.y: e.target ? e.target.height * Math.max(0.15, Math.min(0.85, 0.5 - 0.3 * e.dy)) : 0
        xScale: e.sq * (1 + e.stretch * Math.abs(e.dx) - 0.03 * Math.abs(e.dy))
        yScale: e.sq * (1 + e.stretch * 1.06 * Math.abs(e.dy) - 0.03 * Math.abs(e.dx))
    }
    property Translate tr: Translate { x: e.dx * e.travel; y: e.dy * e.travel }
    // added to the item's transforms (Arrive may add its own too)
    Component.onCompleted: {
        if (!target) return;
        const l = [];
        for (let i = 0; i < target.transform.length; i++) l.push(target.transform[i]);
        l.push(sc, tr);
        target.transform = l;
    }

    // the light under the pointer and the ring from the press: radial gradients filling the item's own rounded
    // shape (the outline is drawn with arcs, so it works on every Qt 6)
    readonly property real rr: Math.max(0, Math.min(radius, width / 2, height / 2))
    component Rounded: ShapePath {
        strokeColor: "transparent"
        startX: e.rr; startY: 0
        PathLine { x: e.width - e.rr; y: 0 }
        PathArc { x: e.width; y: e.rr; radiusX: e.rr; radiusY: e.rr }
        PathLine { x: e.width; y: e.height - e.rr }
        PathArc { x: e.width - e.rr; y: e.height; radiusX: e.rr; radiusY: e.rr }
        PathLine { x: e.rr; y: e.height }
        PathArc { x: 0; y: e.height - e.rr; radiusX: e.rr; radiusY: e.rr }
        PathLine { x: 0; y: e.rr }
        PathArc { x: e.rr; y: 0; radiusX: e.rr; radiusY: e.rr }
    }
    Shape {
        anchors.fill: parent
        visible: e.glow > 0.01
        opacity: e.glow
        preferredRendererType: Shape.CurveRenderer
        Rounded {
            fillGradient: RadialGradient {
                centerX: e.gx; centerY: e.gy; focalX: e.gx; focalY: e.gy
                centerRadius: Math.max(e.width, e.height) * 0.75
                GradientStop { position: 0; color: Theme.alpha(e.light, Theme.light ? 0.16 : 0.2) }
                GradientStop { position: 0.45; color: Theme.alpha(e.light, Theme.light ? 0.07 : 0.08) }
                GradientStop { position: 1; color: Theme.alpha(e.light, 0) }
            }
        }
    }
    Shape {
        id: ring
        property real p: 1
        property real ox: 0
        property real oy: 0
        function go(x, y) { ox = x; oy = y; anim.restart(); }
        anchors.fill: parent
        visible: p < 1
        preferredRendererType: Shape.CurveRenderer
        Rounded {
            fillGradient: RadialGradient {
                centerX: ring.ox; centerY: ring.oy; focalX: ring.ox; focalY: ring.oy
                centerRadius: Math.max(1, Math.max(e.width, e.height) * 1.3 * ring.p)
                GradientStop { position: 0; color: Theme.alpha(e.light, 0) }
                GradientStop { position: 0.55; color: Theme.alpha(e.light, 0.1 * (1 - ring.p)) }
                GradientStop { position: 0.85; color: Theme.alpha(e.light, 0.16 * (1 - ring.p)) }
                GradientStop { position: 1; color: Theme.alpha(e.light, 0) }
            }
        }
        NumberAnimation { id: anim; target: ring; property: "p"; from: 0; to: 1; duration: 460; easing.type: Easing.OutCubic }
    }
}
