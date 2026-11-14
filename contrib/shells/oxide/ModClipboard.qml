// Opens the clipboard history.
import QtQuick

IconButton {
    property var opts: ({})
    property var shell
    icon: "clipboard-text-outline"
    active: shell ? shell.clipboardShown : false
    onClicked: shell.toggleClipboard()
}
