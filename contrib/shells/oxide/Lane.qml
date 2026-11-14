// One third of the bar: builds the modules listed in bar.json, in order.
import QtQuick

Row {
    id: lane
    property var modules: []
    property var shell
    property var window: null
    spacing: BarConfig.spacing

    Repeater {
        model: lane.modules
        Item {
            id: cell
            required property var modelData
            // per-module size: 70-160 % of normal, and the layout follows it
            readonly property real k: (modelData.scale || 100) / 100
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
            // a module with nothing to show (no player, no window title) takes no room, not even spacing
            readonly property bool wanted: slot.item && slot.item.wanted !== undefined ? slot.item.wanted : true
            visible: wanted || width > 0.5
            width: wanted ? slot.implicitWidth * k : 0
            height: slot.implicitHeight
            Behavior on width { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }

            Loader {
                id: slot
                scale: cell.k
                transformOrigin: Item.Left
                anchors.verticalCenter: parent.verticalCenter
                // "workspaces" -> ModWorkspaces.qml; an unknown id simply shows nothing
                source: "Mod" + cell.modelData.id.charAt(0).toUpperCase() + cell.modelData.id.slice(1) + ".qml"
                onLoaded: {
                    item.opts = cell.modelData;
                    item.shell = lane.shell;
                    if (item.window !== undefined) item.window = lane.window;
                }
            }
        }
    }
}
