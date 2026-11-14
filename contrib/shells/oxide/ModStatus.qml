// Wi-Fi, Bluetooth, volume and battery icons in one button; click opens quick settings, wheel changes volume.
// options: wifi, bluetooth, volume, battery, batteryPercent (all default on)
import QtQuick

Rectangle {
    id: root
    property var opts: ({})
    property var shell
    implicitWidth: row.width + 18
    implicitHeight: 28
    radius: Theme.rControl
    color: mouse.containsMouse || (shell && shell.controlShown) ? Theme.raised : Theme.clear(Theme.raised)
    Behavior on color { ColorAnimation { duration: Theme.fast } }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 11
        Icon { visible: root.opts.wifi !== false; anchors.verticalCenter: parent.verticalCenter; name: Sys.wifiIcon; size: 16 }
        Icon {
            visible: root.opts.bluetooth !== false && !!Sys.adapter
            anchors.verticalCenter: parent.verticalCenter
            name: Sys.adapter && Sys.adapter.enabled ? "bluetooth" : "bluetooth-off"
            size: 15
            color: Sys.adapter && Sys.adapter.enabled ? Theme.text : Theme.muted
        }
        Icon { visible: root.opts.volume !== false; anchors.verticalCenter: parent.verticalCenter; name: Sys.volumeIcon; size: 16; color: Sys.muted ? Theme.muted : Theme.text }
        Row {
            visible: root.opts.battery !== false && Sys.hasBattery
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4
            Icon { anchors.verticalCenter: parent.verticalCenter; name: Sys.batteryIcon; size: 17; color: Sys.charging ? Theme.good : (Sys.batteryPct < 15 ? Theme.bad : Theme.text) }
            Txt { visible: root.opts.batteryPercent !== false; anchors.verticalCenter: parent.verticalCenter; text: Sys.batteryPct + "%"; font { family: Theme.mono; pixelSize: 13 } }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.shell.toggleControl(true)
        onWheel: w => Sys.setVolume(Sys.volume + (w.angleDelta.y > 0 ? 0.04 : -0.04))
    }
}
