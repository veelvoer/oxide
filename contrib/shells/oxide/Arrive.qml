// The expressive entrance (as in Hearth): when `when` turns true the item rises a little and grows into place,
// settling softly; items that arrive together come one after another (`order`, 38 ms apart, at most 300 ms).
// Only in the Material Expressive style. Put it inside the item: `Arrive { when: panel.shown; order: 2 }`.
import QtQuick

Item {
    id: arrive
    property Item target: parent
    property bool when: true
    property int order: 0
    property bool fade: true
    readonly property bool on: Theme.expressive && Theme.motionScale > 0
    property real a: 1
    property real o: 1

    onWhenChanged: if (when && on) go()
    function go() {
        anim.stop();
        a = 0; o = 0;
        wait.interval = Math.min(300, order * 38) + 1;
        wait.restart();
    }
    Timer { id: wait; onTriggered: anim.restart() }
    ParallelAnimation {
        id: anim
        SpringAnimation { target: arrive; property: "a"; to: 1; spring: 3.0; damping: 0.34; epsilon: 0.002 }
        NumberAnimation { target: arrive; property: "o"; to: 1; duration: 200 * Theme.motionScale; easing.type: Easing.OutCubic }
    }

    property Translate tr: Translate { y: (1 - arrive.a) * 26 }
    property Scale sc: Scale {
        origin.x: arrive.target ? arrive.target.width / 2 : 0
        origin.y: 0
        xScale: 0.94 + 0.06 * arrive.a
        yScale: 0.94 + 0.06 * arrive.a
    }
    Binding { target: arrive.target; property: "opacity"; value: arrive.o; when: arrive.fade && arrive.on }
    Component.onCompleted: {
        if (!target) return;
        const l = [];
        for (let i = 0; i < target.transform.length; i++) l.push(target.transform[i]);
        l.push(tr, sc);
        target.transform = l;
    }
}
