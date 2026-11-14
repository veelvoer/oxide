// CPU load.
import QtQuick

Row {
    property var opts: ({})
    property var shell
    spacing: 6
    Component.onCompleted: Sys.watchers++
    Component.onDestruction: Sys.watchers--
    Icon { anchors.verticalCenter: parent.verticalCenter; name: "speedometer"; size: 15; color: Theme.muted }
    Txt { anchors.verticalCenter: parent.verticalCenter; text: Math.round(Sys.cpu * 100) + "%"; width: 34; font { family: Theme.mono; pixelSize: 13 } color: Sys.cpu > 0.85 ? Theme.warn : Theme.text }
}
