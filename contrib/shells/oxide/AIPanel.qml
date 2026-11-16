// The assistant chat panel. Uses AIClient for everything that is not drawing.
import Quickshell
import QtQuick

Dock {
    id: root
    required property var ai
    // a sidebar: windows make room for it; close it with the X, Esc or Super+A
    contentWidth: 440
    wantsKeyboard: true

    onShownChanged: if (shown) { input.forceActiveFocus(); chat.positionViewAtEnd(); }
    readonly property bool inputFocused: input.activeFocus
    // ask the compositor for the keyboard again: drop and re-request the layer's keyboard focus
    function refocus() { wantsKeyboard = false; refocusTimer.start(); }
    Timer { id: refocusTimer; interval: 60; onTriggered: { root.wantsKeyboard = true; input.forceActiveFocus(); } }


    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        // ---------------- header ----------------
        Item {
            id: head
            x: 16; y: 12
            width: parent.width - 32; height: 44
            Rectangle {
                id: badge
                width: 34; height: 34; radius: Theme.rControl
                anchors.verticalCenter: parent.verticalCenter
                color: "transparent"
                Icon { anchors.centerIn: parent; name: "robot-outline"; size: 20; color: Theme.muted }
            }
            Column {
                x: 44
                anchors.verticalCenter: parent.verticalCenter
                Txt { text: root.ai.name !== "" ? root.ai.name : "Assistant"; color: Theme.text; font { pixelSize: 16; weight: Font.DemiBold } }
                Txt {
                    text: !root.ai.ready ? "starting..." : (root.ai.model + "  -  " + (root.ai.permission === "full" ? "full control" : root.ai.permission === "ask" ? "asks every time" : "asks for risky actions"))
                    color: Theme.muted
                    font.pixelSize: 11
                }
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                IconButton { icon: "delete-outline"; tint: Theme.muted; onClicked: root.ai.reset() }
                IconButton { icon: "cog-outline"; tint: Theme.muted; onClicked: Quickshell.execDetached(["oxide-settings", "assistant"]) }
                IconButton { icon: "close"; tint: Theme.muted; onClicked: root.dismissed() }
            }
        }
        Rectangle { x: 16; y: 60; width: parent.width - 32; height: 1; color: Theme.line }

        // ---------------- messages ----------------
        ListView {
            id: chat
            x: 12; y: 68
            width: parent.width - 24
            height: parent.height - 68 - 76
            clip: true
            spacing: 10
            // new messages rise in
            add: Transition { ParallelAnimation { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200 } NumberAnimation { property: "y"; from: 14; duration: 260; easing.type: Easing.OutCubic } } }
            displaced: Transition { NumberAnimation { property: "y"; duration: 220; easing.type: Easing.OutCubic } }
            model: root.ai.messages
            boundsBehavior: Flickable.StopAtBounds
            onCountChanged: positionViewAtEnd()
            onContentHeightChanged: if (atYEnd || root.ai.busy) positionViewAtEnd()

            delegate: Item {
                id: msg
                required property int index
                required property string kind
                required property string text
                required property int tid
                required property string name
                required property string input
                required property string status
                required property string output
                required property string summary
                required property string risk
                width: chat.width
                height: loader.item ? loader.item.implicitHeight : 0
                opacity: 0
                Component.onCompleted: opacity = 1
                Behavior on opacity { NumberAnimation { duration: Theme.normal } }

                Loader {
                    id: loader
                    width: parent.width
                    sourceComponent: msg.kind === "user" ? userBubble : msg.kind === "assistant" ? assistantText : msg.kind === "tool" ? toolCard : errorBox
                }

                Component {
                    id: userBubble
                    Item {
                        implicitHeight: bubble.height
                        Rectangle {
                            id: bubble
                            anchors.right: parent.right
                            width: Math.min(parent.width * 0.85, userText.implicitWidth + 28)
                            height: userText.implicitHeight + 20
                            radius: Theme.rBox
                            color: Theme.accent
                            Txt {
                                id: userText
                                x: 14; y: 10
                                width: Math.min(implicitWidth, root.contentWidth * 0.85 - 28)
                                text: msg.text
                                wrapMode: Text.Wrap
                                color: Theme.ink
                                font.pixelSize: 14
                            }
                        }
                    }
                }
                Component {
                    id: assistantText
                    Txt {
                        x: 6
                        width: msg.width - 12
                        text: msg.text
                        textFormat: Text.MarkdownText
                        wrapMode: Text.Wrap
                        color: Theme.text
                        font.pixelSize: 14
                        lineHeight: 1.15
                        linkColor: Theme.accentSoft
                    }
                }
                Component {
                    id: errorBox
                    Rectangle {
                        implicitHeight: errText.implicitHeight + 20
                        radius: Theme.rBox
                        color: Theme.alpha(Theme.bad, 0.14)
                        border.color: Theme.alpha(Theme.bad, 0.5)
                        Row {
                            x: 12; y: 10
                            spacing: 10
                            Icon { name: "alert-circle-outline"; size: 18; color: Theme.bad }
                            Txt { id: errText; width: msg.width - 70; text: msg.text; wrapMode: Text.Wrap; color: Theme.text; font.pixelSize: 13 }
                        }
                    }
                }
                Component {
                    id: toolCard
                    Rectangle {
                        id: card
                        property bool open: false
                        implicitHeight: cardCol.implicitHeight + 20
                        radius: Theme.rBox
                        color: msg.status === "confirm" ? Theme.alpha(Theme.warn, 0.12) : Theme.alpha(Theme.text, 0.05)
                        border.color: msg.status === "confirm" ? Theme.alpha(Theme.warn, 0.5) : Theme.alpha(Theme.warn, 0)
                        Behavior on color { ColorAnimation { duration: Theme.fast } }
                        Column {
                            id: cardCol
                            x: 12; y: 10
                            width: parent.width - 24
                            spacing: 8
                            Row {
                                spacing: 10
                                Rectangle {
                                    width: 28; height: 28; radius: Theme.rControl
                                    color: Theme.alpha(msg.status === "failed" || msg.status === "denied" ? Theme.bad : Theme.accent, 0.18)
                                    Icon {
                                        anchors.centerIn: parent
                                        size: 16
                                        color: msg.status === "failed" || msg.status === "denied" ? Theme.bad : Theme.accent
                                        name: msg.name === "shell" ? "console-line" : msg.name === "screenshot" ? "monitor-screenshot" : msg.name === "input" ? "cursor-default-click-outline" : msg.name === "oxide_config" ? "tune-variant" : "view-grid-outline"
                                    }
                                }
                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: card.width - 24 - 28 - 10 - 34
                                    Txt {
                                        width: parent.width
                                        elide: Text.ElideRight
                                        text: msg.name === "shell" ? (JSON.parse(msg.input).command || "") : msg.name === "oxide_dispatch" ? "Oxide: " + JSON.stringify(JSON.parse(msg.input).action) : msg.name === "screenshot" ? "Looking at the screen" : msg.name === "oxide_state" ? "Checking windows" : msg.name === "oxide_config" ? "Oxide settings" : msg.name
                                        color: Theme.text
                                        font { pixelSize: 13; family: msg.name === "shell" ? "monospace" : "" }
                                    }
                                    Txt {
                                        text: msg.status === "running" ? "Working..." : msg.status === "ok" ? "Done" : msg.status === "failed" ? "Failed" : msg.status === "denied" ? "Not allowed" : "Needs your OK"
                                        color: msg.status === "confirm" ? Theme.warn : Theme.muted
                                        font.pixelSize: 11
                                    }
                                }
                                Item {
                                    width: 34; height: 28
                                    Icon { visible: msg.status === "ok"; anchors.centerIn: parent; name: "check"; size: 18; color: Theme.good }
                                    Rectangle {
                                        visible: msg.status === "running"
                                        anchors.centerIn: parent
                                        width: 8; height: 8; radius: 4
                                        color: Theme.accent
                                        SequentialAnimation on opacity { running: msg.status === "running"; loops: Animation.Infinite; NumberAnimation { to: 0.2; duration: 500 } NumberAnimation { to: 1; duration: 500 } }
                                    }
                                }
                            }
                            // permission question
                            Column {
                                visible: msg.status === "confirm"
                                width: parent.width
                                spacing: 8
                                Txt {
                                    width: parent.width
                                    text: msg.summary
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 4
                                    elide: Text.ElideRight
                                    color: Theme.text
                                    font { pixelSize: 12; family: "monospace" }
                                }
                                Row {
                                    spacing: 8
                                    Rectangle {
                                        width: 96; height: 32; radius: Theme.rControl
                                        color: allow.containsMouse ? Theme.accentSoft : Theme.accent
                                        Txt { anchors.centerIn: parent; text: msg.risk === "high" ? "Allow (risky)" : "Allow"; color: Theme.ink; font { pixelSize: 13; weight: Font.DemiBold } }
                                        MouseArea { id: allow; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.ai.confirm(msg.tid, true) }
                                    }
                                    Rectangle {
                                        width: 72; height: 32; radius: Theme.rControl
                                        color: deny.containsMouse ? Theme.raised : Theme.alpha(Theme.text, 0.08)
                                        Txt { anchors.centerIn: parent; text: "Deny"; color: Theme.text; font.pixelSize: 13 }
                                        MouseArea { id: deny; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.ai.confirm(msg.tid, false) }
                                    }
                                }
                            }
                            Txt {
                                visible: card.open && msg.output !== ""
                                width: parent.width
                                text: msg.output
                                wrapMode: Text.Wrap
                                maximumLineCount: 14
                                elide: Text.ElideRight
                                color: Theme.muted
                                font { pixelSize: 11; family: "monospace" }
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            z: -1
                            cursorShape: msg.output !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: card.open = !card.open
                        }
                    }
                }
            }

            // thinking indicator
            footer: Item {
                width: chat.width
                height: root.ai.thinking ? 34 : 0
                visible: height > 0
                Row {
                    x: 8; y: 6
                    spacing: 5
                    Repeater {
                        model: 3
                        Rectangle {
                            required property int index
                            width: 8; height: 8; radius: 4
                            color: Theme.accent
                            SequentialAnimation on opacity {
                                running: root.ai.thinking
                                loops: Animation.Infinite
                                PauseAnimation { duration: index * 160 }
                                NumberAnimation { to: 0.2; duration: 380 }
                                NumberAnimation { to: 1; duration: 380 }
                                PauseAnimation { duration: (2 - index) * 160 }
                            }
                        }
                    }
                }
            }

            // empty state: one plain line, no hero
            Txt {
                visible: root.ai.messages.count === 0
                x: 16; y: 16
                width: parent.width - 32
                wrapMode: Text.Wrap
                text: root.ai.hasKey ? "It can change settings, open apps and read your system state. It asks before anything risky."
                                     : "Add your Anthropic API key in Settings > Assistant to start."
                color: Theme.muted
                font.pixelSize: 13
            }
        }

        // ---------------- input ----------------
        Rectangle {
            x: 12; y: parent.height - 64
            width: parent.width - 24; height: 52
            radius: Theme.rBox
            color: Theme.raised
            border.color: input.activeFocus ? Theme.alpha(Theme.accent, 0.7) : Theme.alpha(Theme.accent, 0)
            Behavior on border.color { ColorAnimation { duration: Theme.fast } }
            TextInput {
                id: input
                x: 16
                width: parent.width - 16 - 56
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.text
                selectionColor: Theme.accent
                font.pixelSize: 14
                clip: true
                enabled: root.ai.ready
                Keys.onReturnPressed: { root.ai.send(text); text = ""; }
                Keys.onEnterPressed: { root.ai.send(text); text = ""; }
            }
            Txt {
                visible: input.text === ""
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                text: root.ai.busy ? "Working..." : "Ask anything..."
                color: Theme.muted
                font.pixelSize: 14
            }
            Rectangle {
                width: 38; height: 38; radius: Theme.rControl
                anchors { right: parent.right; rightMargin: 7; verticalCenter: parent.verticalCenter }
                color: root.ai.busy ? Theme.surface : Theme.accent

                Icon { anchors.centerIn: parent; name: root.ai.busy ? "stop-circle-outline" : "send"; size: 19; color: root.ai.busy ? Theme.text : Theme.ink }
                MouseArea {
                    id: sendMouse
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: { if (root.ai.busy) root.ai.cancel(); else { root.ai.send(input.text); input.text = ""; } }
                }
            }
        }
    }
}
