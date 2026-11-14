// Power button: opens quick settings (lock, log out, restart, shut down are at the bottom).
import QtQuick

IconButton {
    property var opts: ({})
    property var shell
    icon: "power"
    onClicked: shell.toggleControl(true)
}
