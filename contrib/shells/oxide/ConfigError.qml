// Shown while oxide.toml has an error. Oxide keeps the last good settings meanwhile; this says what and where,
// and opens the file in your editor at that line. It goes away by itself once the file is fixed.
import Quickshell
import Quickshell.Wayland
import QtQuick

PanelWindow {
    id: win
    readonly property var err: OxideState.configError
    // dismissed for this exact message (a new error shows again)
    property string dismissedFor: ""
    readonly property bool on: !!err && dismissedFor !== err.message + err.line
    property real t: on ? 1 : 0
    Behavior on t { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }

    screen: OxideState.activeScreen
    visible: on || t > 0.01
    color: "transparent"
    anchors { bottom: true }
    margins.bottom: 24
    implicitWidth: 520
    implicitHeight: card.height + 8
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "oxide-config-error"

    Rectangle {
        id: card
        width: parent.width
        height: col.implicitHeight + 28
        y: (1 - win.t) * 30
        opacity: win.t
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.alpha(Theme.bad, 0.7)
        Column {
            id: col
            x: 16; y: 14
            width: parent.width - 32
            spacing: 6
            Row {
                spacing: 8
                Icon { name: "alert-circle-outline"; size: 18; color: Theme.bad; anchors.verticalCenter: parent.verticalCenter }
                Txt {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Config not applied" + (win.err && win.err.line ? "  ·  line " + win.err.line : "")
                    font { pixelSize: 14; weight: Font.DemiBold }
                }
            }
            Txt { width: parent.width; wrapMode: Text.Wrap; text: win.err ? win.err.message : ""; color: Theme.text; font { family: Theme.mono; pixelSize: 12 } }
            Txt { width: parent.width; wrapMode: Text.Wrap; text: "Oxide keeps your last working settings until the file is fixed."; color: Theme.muted; font.pixelSize: 12 }
            Row {
                spacing: 8
                topPadding: 4
                Chip { icon: "text-box-edit-outline"; label: "Open in editor"; onClicked: Quickshell.execDetached(["oxide-ctl", "edit-config"].concat(win.err && win.err.line ? [String(win.err.line)] : [])) }
                Chip { label: "Dismiss"; onClicked: win.dismissedFor = win.err.message + win.err.line }
            }
        }
    }
}
