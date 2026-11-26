// "A new version of Oxide is available": a card with an Update button, a little after login (the compositor looks for
// updates then and every six hours; the answer is in ~/.local/state/oxide/update.json). Update runs `oxide-ctl update run`.
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import Oxide

PanelWindow {
    id: win
    property var st: ({})
    property string dismissed: ""
    // 0 offer, 1 updating, 2 done
    property int phase: 0
    property bool failed: false
    readonly property string stateDir: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/oxide"
    readonly property bool newer: !!st.latest && !!st.installed && st.latest !== st.installed && !String(st.installed).endsWith("-dirty")
    readonly property bool on: phase > 0 || (newer && dismissed !== st.latest && !st.running)
    property real t: on ? 1 : 0
    Behavior on t { NumberAnimation { duration: 260; easing.type: Easing.OutBack; easing.overshoot: 0.8 } }

    FileView {
        path: win.stateDir + "/update.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { try { win.st = JSON.parse(text()); } catch (e) { win.st = ({}); } }
    }

    Process {
        id: runner
        command: ["oxide-ctl", "update", "run"]
        onExited: code => { win.failed = code !== 0; win.phase = code === 0 ? 2 : 0; }
    }

    function update() { failed = false; phase = 1; runner.running = true; }

    screen: OxideState.activeScreen
    visible: on || t > 0.01
    color: "transparent"
    anchors { bottom: true }
    margins.bottom: 24
    implicitWidth: 460
    implicitHeight: card.height + 8
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "oxide-update-offer"

    Rectangle {
        id: card
        width: parent.width
        height: col.implicitHeight + 32
        y: (1 - win.t) * 30
        opacity: Math.min(1, win.t)
        scale: 0.96 + 0.04 * win.t
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.alpha(Theme.accent, 0.55)
        Column {
            id: col
            x: 18; y: 16
            width: parent.width - 36
            spacing: 12
            Row {
                spacing: 10
                Rectangle {
                    width: 32; height: 32; radius: 16
                    color: Theme.alpha(Theme.accent, 0.18)
                    Icon { anchors.centerIn: parent; name: "rocket-launch-outline"; size: 18; color: Theme.accent }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    Txt {
                        font { pixelSize: 15; weight: Font.DemiBold }
                        text: win.phase === 1 ? "Updating Oxide…" : win.phase === 2 ? "Oxide is updated" : "A new version of Oxide is available"
                    }
                    Txt {
                        color: Theme.muted; font.pixelSize: 12
                        width: col.width - 42
                        wrapMode: Text.Wrap
                        text: win.phase === 1 ? "This takes a few minutes. You can keep working."
                            : win.phase === 2 ? "Log out and back in to use it."
                            : "Update now, or later from Settings > Config."
                    }
                }
            }
            Txt {
                visible: win.failed && win.phase === 0
                width: parent.width; wrapMode: Text.Wrap
                color: Theme.bad; font.pixelSize: 12
                text: "The update did not finish. Settings > Config shows why."
            }
            Row {
                spacing: 8
                Chip { visible: win.phase === 0; icon: "arrow-down"; label: "Update"; on: true; onClicked: win.update() }
                Chip { visible: win.phase === 0; label: "Later"; onClicked: win.dismissed = win.st.latest }
                Chip { visible: win.phase === 2; icon: "check"; label: "OK"; on: true; onClicked: { win.phase = 0; win.dismissed = win.st.latest; } }
            }
        }
    }
}
