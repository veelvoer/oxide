// Lock screen: the themed background, a clock and a password field (checked by PAM, like a login).
// Lock with `oxide-ctl lock` (Super+N). If the shell restarts while locked it takes the lock back over
// (a marker file in $XDG_RUNTIME_DIR remembers it). Emergency from another TTY: `oxide-ctl unlock`.
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Pam
import QtQuick

Scope {
    id: root
    property bool locked: false
    property string typed: ""
    property bool busy: false
    property string failure: ""
    // one marker per compositor (a nested test window must not lock the real session)
    readonly property string marker: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/oxide-locked-" + (Quickshell.env("WAYLAND_DISPLAY") || "x")

    function lock() { if (!locked) { markerSet.running = true; locked = true; typed = ""; failure = ""; } }
    function unlocked() { locked = false; typed = ""; failure = ""; busy = false; markerClear.running = true; }

    Process { id: markerSet; command: ["touch", root.marker] }
    Process { id: markerClear; command: ["rm", "-f", root.marker] }
    // the shell was restarted while the screen was locked: lock again at once
    FileView {
        path: root.marker
        onLoaded: if (!root.locked) { root.locked = true; }
    }

    PamContext {
        id: pam
        configDirectory: Qt.resolvedUrl("pam").toString().replace("file://", "")
        config: "password.conf"
        onPamMessage: { if (responseRequired) respond(root.typed); }
        onCompleted: result => {
            if (result === PamResult.Success) root.unlocked();
            else { root.typed = ""; root.failure = "Wrong password"; root.busy = false; }
        }
        onError: { root.failure = "Could not check the password"; root.busy = false; }
    }

    function submit() {
        if (busy || typed.length === 0) return;
        busy = true; failure = "";
        pam.start();
    }

    WlSessionLock {
        id: sessionLock
        locked: root.locked

        WlSessionLockSurface {
            id: surf
            color: Theme.bg

            // the same background as the desktop: the Oxide tiles, quiet in the corner
            Item {
                anchors.fill: parent
                Rectangle {
                    anchors.fill: parent
                    gradient: Gradient {
                        GradientStop { position: 0; color: Theme.bg }
                        GradientStop { position: 1; color: Qt.darker(Theme.bg, Theme.light ? 1.06 : 1.25) }
                    }
                }
                Item {
                    id: glyph
                    readonly property real u: Math.min(parent.width, parent.height) * 0.30
                    width: u * 0.86; height: u * 0.86
                    x: parent.width - width - parent.width * 0.05
                    y: parent.height - height - parent.height * 0.08
                    readonly property real r: Math.max(2, Theme.rBox * 0.9)
                    Rectangle { x: 0; y: 0; width: glyph.u * 0.43; height: glyph.u * 0.86; radius: glyph.r; color: "transparent"; border.color: Theme.line; border.width: 2 }
                    Rectangle { x: glyph.u * 0.5; y: 0; width: glyph.u * 0.36; height: glyph.u * 0.41; radius: glyph.r; color: Theme.alpha(Theme.accent, 0.14); border.color: Theme.alpha(Theme.accent, 0.5); border.width: 2 }
                    Rectangle { x: glyph.u * 0.5; y: glyph.u * 0.45; width: glyph.u * 0.36; height: glyph.u * 0.41; radius: glyph.r; color: "transparent"; border.color: Theme.line; border.width: 2 }
                }
            }

            SystemClock { id: clock; precision: SystemClock.Minutes }

            Column {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -parent.height * 0.06
                spacing: 6
                Txt {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Qt.formatDateTime(clock.date, "HH:mm")
                    color: Theme.text
                    font { family: Theme.mono; pixelSize: Math.round(surf.height * 0.13); weight: Font.Light }
                }
                Txt {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Qt.formatDateTime(clock.date, "dddd d MMMM")
                    color: Theme.muted
                    font.pixelSize: 18
                }
                Item { width: 1; height: 28 }

                Rectangle {
                    id: field
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 320; height: 46
                    radius: Math.min(height / 2, Theme.roundness * 2.2)
                    color: Theme.surface
                    border.color: root.failure !== "" ? Theme.bad : (input.activeFocus ? Theme.alpha(Theme.accent, 0.8) : Theme.line)
                    Behavior on border.color { ColorAnimation { duration: Theme.fast } }
                    opacity: root.busy ? 0.6 : 1

                    Icon { x: 16; anchors.verticalCenter: parent.verticalCenter; name: "lock-outline"; size: 18; color: Theme.muted }
                    Row {
                        anchors.centerIn: parent
                        spacing: 6
                        Repeater {
                            model: root.typed.length
                            Rectangle { width: 8; height: 8; radius: 4; color: Theme.text }
                        }
                    }
                    Txt {
                        visible: root.typed.length === 0
                        anchors.centerIn: parent
                        text: root.busy ? "Checking..." : "Password"
                        color: Theme.muted
                        font.pixelSize: 14
                    }
                    TextInput {
                        id: input
                        anchors.fill: parent
                        // invisible but still an item that can hold keyboard focus (opacity 0 would not)
                        opacity: 0.01
                        color: "transparent"
                        selectionColor: "transparent"
                        echoMode: TextInput.Password
                        focus: true
                        text: root.typed
                        onTextChanged: if (text !== root.typed) { root.typed = text; root.failure = ""; }
                        Keys.onReturnPressed: root.submit()
                        Keys.onEnterPressed: root.submit()
                    }
                    Component.onCompleted: input.forceActiveFocus()
                    Connections {
                        target: root
                        function onTypedChanged() { if (root.typed === "" && input.text !== "") input.text = ""; }
                        function onLockedChanged() { if (root.locked) input.forceActiveFocus(); }
                    }
                }
                Txt {
                    anchors.horizontalCenter: parent.horizontalCenter
                    height: 20
                    text: root.failure
                    color: Theme.bad
                    font.pixelSize: 13
                }
            }
            MouseArea { anchors.fill: parent; onClicked: input.forceActiveFocus() }
        }
    }
}
