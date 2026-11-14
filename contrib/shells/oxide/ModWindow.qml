// Title of the focused window; click opens its options. options: showIcon (bool), maxWidth (px)
import QtQuick

Item {
    id: root
    property var opts: ({})
    property var shell
    readonly property bool wanted: OxideState.focused !== null
    visible: wanted
    opacity: OxideState.focused ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: Theme.normal } }
    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight

    Row {
        id: row
        spacing: 8
        Icon { visible: root.opts.showIcon !== false; anchors.verticalCenter: parent.verticalCenter; name: "application-outline"; size: 15; color: Theme.muted }
        Txt {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, root.opts.maxWidth || 280)
            elide: Text.ElideRight
            text: OxideState.focused ? (OxideState.focused.title || OxideState.focused.app_id) : ""
            color: Theme.muted
            font.pixelSize: 14
        }
    }
    MouseArea {
        anchors.fill: parent
        anchors.margins: -4
        cursorShape: Qt.PointingHandCursor
        onClicked: if (root.shell) root.shell.toggleWindow()
    }
}
