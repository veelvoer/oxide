// Clipboard history. A background watcher (oxide-ctl clip watch, started in shell.qml) stores what you copy;
// this panel searches it, pastes an entry back into the clipboard, pins or deletes entries.
import Quickshell
import Quickshell.Io
import QtQuick

Pop {
    id: root
    place: "center"
    contentWidth: 520
    contentHeight: 72 + Math.min(8, Math.max(items.length, 1)) * 56 + 52
    wantsKeyboard: true

    property var items: []
    property string query: ""
    readonly property var shownItems: {
        const q = query.trim().toLowerCase();
        return q === "" ? items : items.filter(e => e.kind === "text" && e.preview.toLowerCase().includes(q));
    }

    onShownChanged: if (shown) { query = ""; input.text = ""; input.forceActiveFocus(); list.currentIndex = 0; refresh(); }

    function refresh() { lister.running = true; }
    function act(args) { runner.command = ["oxide-ctl", "clip"].concat(args); runner.running = true; }
    function pick(e) { act(["copy", e.id]); root.dismissed(); }

    Process {
        id: lister
        command: ["oxide-ctl", "clip", "list"]
        stdout: StdioCollector {
            onStreamFinished: { try { root.items = JSON.parse(text); } catch (e) { root.items = []; } }
        }
    }
    Process { id: runner; onExited: root.refresh() }

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        Rectangle {
            x: 12; y: 12
            width: parent.width - 24; height: 44
            radius: Theme.rBox
            color: Theme.raised
            Icon { x: 14; anchors.verticalCenter: parent.verticalCenter; name: "magnify"; size: 18; color: Theme.muted }
            TextInput {
                id: input
                x: 42
                width: parent.width - 56
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.text
                selectionColor: Theme.accent
                font.pixelSize: 15
                clip: true
                onTextChanged: { root.query = text; list.currentIndex = 0; }
                Keys.onDownPressed: list.currentIndex = Math.min(list.count - 1, list.currentIndex + 1)
                Keys.onUpPressed: list.currentIndex = Math.max(0, list.currentIndex - 1)
                Keys.onReturnPressed: { const e = root.shownItems[list.currentIndex]; if (e) root.pick(e); }
                Keys.onDeletePressed: { const e = root.shownItems[list.currentIndex]; if (e) root.act(["delete", e.id]); }
            }
            Txt { visible: input.text === ""; x: 42; anchors.verticalCenter: parent.verticalCenter; text: "Search what you copied"; color: Theme.muted; font.pixelSize: 15 }
        }

        ListView {
            id: list
            x: 12; y: 64
            width: parent.width - 24
            height: Math.min(8, Math.max(root.shownItems.length, 1)) * 56
            model: root.shownItems
            clip: true
            currentIndex: 0
            // one highlight that glides to the selected row
            highlight: Rectangle { radius: Theme.rBox; color: Theme.alpha(Theme.accent, 0.2) }
            highlightMoveDuration: 150
            highlightMoveVelocity: -1
            highlightResizeDuration: 150
            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                width: list.width
                height: 56
                radius: Theme.rBox
                color: "transparent"
                Behavior on color { ColorAnimation { duration: Theme.fast } }

                Image {
                    visible: row.modelData.kind === "image"
                    x: 10; anchors.verticalCenter: parent.verticalCenter
                    width: 60; height: 40
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    source: row.modelData.kind === "image" ? "file://" + row.modelData.path : ""
                    sourceSize.height: 80
                }
                Txt {
                    x: row.modelData.kind === "image" ? 80 : 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - 76
                    text: row.modelData.kind === "image" ? "Image, " + Math.round(row.modelData.size / 1024) + " KB" : row.modelData.preview.trim().replace(/\s+/g, " ")
                    elide: Text.ElideRight
                    maximumLineCount: 2
                    wrapMode: Text.Wrap
                    color: Theme.text
                    font.pixelSize: 13
                }
                Row {
                    anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    opacity: hover.hovered || row.modelData.pinned || ListView.isCurrentItem ? 1 : 0
                    IconButton {
                        icon: row.modelData.pinned ? "pin" : "pin-outline"
                        active: row.modelData.pinned
                        onClicked: root.act(["pin", row.modelData.id])
                    }
                    IconButton { icon: "delete-outline"; onClicked: root.act(["delete", row.modelData.id]) }
                }
                HoverHandler { id: hover; onPointChanged: if (hovered) list.currentIndex = row.index }
                TapHandler { onTapped: { list.currentIndex = row.index; root.pick(row.modelData); } }
            }
        }

        Txt {
            visible: root.shownItems.length === 0
            x: 24; y: 76
            text: root.query === "" ? "Nothing copied yet." : "No matches."
            color: Theme.muted
            font.pixelSize: 13
        }

        Rectangle { x: 12; y: parent.height - 48; width: parent.width - 24; height: 1; color: Theme.line }
        Txt {
            x: 16; y: parent.height - 34
            text: "Enter to paste back, Delete to remove"
            color: Theme.muted
            font.pixelSize: 12
        }
        Txt {
            anchors { right: parent.right; rightMargin: 16 }
            y: parent.height - 34
            text: "Clear all"
            color: clearHover.hovered ? Theme.text : Theme.muted
            font.pixelSize: 12
            HoverHandler { id: clearHover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: root.act(["clear"]) }
        }
    }
}
