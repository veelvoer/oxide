// Opens the AI assistant panel.
import QtQuick

IconButton {
    property var opts: ({})
    property var shell
    icon: "robot-outline"
    active: shell ? shell.aiShown : false
    onClicked: shell.toggleAi()
}
