// Time and date; click opens the calendar. options: format24, seconds, date, weekday
import Quickshell
import QtQuick

Rectangle {
    id: root
    property var opts: ({})
    property var shell
    implicitWidth: row.width + 18
    implicitHeight: 28
    radius: Theme.rControl
    color: mouse.containsMouse || (shell && shell.calendarShown) ? Theme.raised : Theme.clear(Theme.raised)
    Behavior on color { ColorAnimation { duration: Theme.fast } }

    SystemClock { id: clock; precision: root.opts.seconds ? SystemClock.Seconds : SystemClock.Minutes }
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 10
        Txt {
            text: Qt.formatDateTime(clock.date, (root.opts.format24 === false ? "h:mm" : "HH:mm") + (root.opts.seconds ? ":ss" : "") + (root.opts.format24 === false ? " AP" : ""))
            font { family: Theme.mono; pixelSize: 14; weight: Font.Medium }
        }
        Txt {
            visible: root.opts.date !== false
            anchors.baseline: parent.children[0].baseline
            text: Qt.formatDateTime(clock.date, root.opts.weekday === false ? "d MMM" : "ddd d MMM")
            color: Theme.muted
            font.pixelSize: 14
        }
    }
    MouseArea { id: mouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.shell.toggleCalendar(true) }
    Elastic { radius: root.radius }
}
