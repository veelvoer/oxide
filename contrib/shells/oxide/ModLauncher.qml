// Opens the app launcher.
import QtQuick

IconButton {
    property var opts: ({})
    property var shell
    icon: "oxide"
    tint: Theme.accent
    active: shell ? shell.launcherShown : false
    onClicked: shell.toggleLauncher()
}
