// Memory in use.
import QtQuick
import Oxide

Row {
    property var opts: ({})
    property var shell
    spacing: 6
    Component.onCompleted: Sys.watchers++
    Component.onDestruction: Sys.watchers--
    Icon { anchors.verticalCenter: parent.verticalCenter; name: "cached"; size: 15; color: Theme.muted }
    Txt { anchors.verticalCenter: parent.verticalCenter; text: Math.round(Sys.memory * 100) + "%"; width: 34; font { family: Theme.mono; pixelSize: 13 } color: Sys.memory > 0.9 ? Theme.warn : Theme.text }
}
