// Options for the focused window (Super+O). It hangs from the window's top edge.
// "This window" acts now; "Always" is remembered for every window of the app (a rule in the config).
import Quickshell
import QtQuick

Pop {
    id: root
    place: "window"
    wantsKeyboard: true
    contentWidth: 352
    contentHeight: col.implicitHeight + 28

    readonly property var win: OxideState.windows.find(w => w.focused) || null
    readonly property string app: win ? win.app_id : ""
    readonly property var rule: win && win.rule ? win.rule : null
    // a readable app name: "org.kde.dolphin" -> "Dolphin"
    readonly property string appName: {
        if (!app) return "";
        const last = app.split(".").pop().replace(/[-_]/g, " ");
        return last.charAt(0).toUpperCase() + last.slice(1);
    }

    // undo: what the rule was before the last change (null = there was none)
    property var undoRule: undefined
    property bool opacityOn: rule ? rule.opacity !== null && rule.opacity !== undefined : false

    function snapshot() { undoRule = rule ? { floating: rule.floating, workspace: rule.workspace, opacity: rule.opacity, blur: rule.blur } : null; }
    function setRule(fields) {
        if (!app) return;
        snapshot();
        const args = ["oxide-ctl", "rule", app];
        for (const k in fields) args.push(k + "=" + (fields[k] === null ? "-" : fields[k]));
        Quickshell.execDetached(args);
    }
    function restore() {
        if (undoRule === undefined || !app) return;
        const r = undoRule;
        const f = {};
        for (const k of ["floating", "workspace", "opacity", "blur"]) f[k] = (r && r[k] !== null && r[k] !== undefined) ? r[k] : null;
        undoRule = undefined;
        const args = ["oxide-ctl", "rule", app];
        for (const k in f) args.push(k + "=" + (f[k] === null ? "-" : f[k]));
        Quickshell.execDetached(args);
    }

    // hang from the focused window
    function place_() {
        if (!win) return;
        const o = OxideState.outputs.find(o => o.name === (screen ? screen.name : "")) || { x: 0, y: 0 };
        anchorX = win.geometry[0] + win.geometry[2] / 2 - o.x;
        anchorY = win.geometry[1] + 10 - o.y;
    }
    onShownChanged: if (shown) { place_(); undoRule = undefined; keys.forceActiveFocus(); }

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        Item {
            id: keys
            anchors.fill: parent
            focus: true
            Keys.onPressed: e => {
                if (e.key === Qt.Key_F) { OxideState.dispatch({ "type": "toggle_floating" }); e.accepted = true; }
                else if (e.key === Qt.Key_Z) { root.restore(); e.accepted = true; }
            }
        }

        Column {
            id: col
            x: 16; y: 14
            width: parent.width - 32
            spacing: 12

            // who this is
            Row {
                spacing: 10
                height: 36
                Arrive { when: root.shown; order: 0 }
                Rectangle {
                    width: 36; height: 36; radius: Theme.expressive ? 18 : Theme.rBox
                    color: Theme.expressive ? Theme.container : Theme.raised
                    Icon { anchors.centerIn: parent; name: "application-outline"; size: 18; color: Theme.muted }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: col.width - 46
                    Txt { width: parent.width; text: root.win ? root.appName : "No window"; elide: Text.ElideRight; font { pixelSize: 14; weight: Font.DemiBold } }
                    Txt { width: parent.width; text: root.win ? root.win.title : "Focus a window and press Super+O"; elide: Text.ElideRight; color: Theme.muted; font.pixelSize: 12 }
                }
            }

            // ----- this window
            Column {
                visible: !!root.win
                width: parent.width
                spacing: 8
                Arrive { when: root.shown; order: 1 }
                Txt { text: "This window"; color: Theme.expressive ? Theme.accent : Theme.muted; font { pixelSize: Theme.expressive ? 13 : 11; weight: Font.Medium } }
                Flow {
                    width: parent.width
                    spacing: 6
                    Chip { icon: "application-outline"; label: "Float"; on: root.win ? root.win.floating : false; onClicked: OxideState.dispatch({ "type": "toggle_floating" }) }
                    Chip { icon: "monitor-screenshot"; label: "Fullscreen"; on: root.win ? root.win.fullscreen : false; onClicked: { OxideState.dispatch({ "type": "toggle_fullscreen" }); root.dismissed(); } }
                    Chip { icon: "close"; label: "Close"; danger: true; onClicked: { root.dismissed(); OxideState.dispatch({ "type": "close" }); } }
                }
                Row {
                    spacing: 6
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: "Send to"; color: Theme.muted; font.pixelSize: 12; width: 52 }
                    Repeater {
                        model: 6
                        Chip {
                            required property int index
                            label: String(index + 1)
                            on: root.win ? root.win.workspace === index + 1 : false
                            onClicked: OxideState.dispatch({ "type": "move_to_workspace", "arg": index + 1 })
                        }
                    }
                }
            }

            Rectangle { visible: !!root.win; width: parent.width; height: 1; color: Theme.line }

            // ----- always, for this app
            Column {
                visible: !!root.win
                width: parent.width
                spacing: 10
                Arrive { when: root.shown; order: 2 }
                Row {
                    width: parent.width
                    Txt { text: "Always, for " + root.appName; color: Theme.expressive ? Theme.accent : Theme.muted; font { pixelSize: Theme.expressive ? 13 : 11; weight: Font.Medium }
                        width: parent.width - undoChip.width }
                    Chip {
                        id: undoChip
                        height: 22
                        visible: root.undoRule !== undefined
                        label: "Undo"
                        onClicked: root.restore()
                    }
                }

                // open floating
                Item {
                    width: parent.width; height: 28
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: "Open floating" }
                    Toggle {
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        checked: root.rule ? root.rule.floating === true : false
                        onToggled: v => root.setRule({ floating: v ? true : null })
                    }
                }
                // open on a workspace
                Item {
                    width: parent.width; height: 28
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: "Open on workspace" }
                    Row {
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        readonly property int ws: root.rule && root.rule.workspace ? root.rule.workspace : 0
                        IconButton { icon: "chevron-left"; onClicked: root.setRule({ workspace: parent.ws <= 1 ? null : parent.ws - 1 }) }
                        Txt { width: 34; anchors.verticalCenter: parent.verticalCenter; horizontalAlignment: Text.AlignHCenter; text: parent.ws === 0 ? "Any" : String(parent.ws); color: parent.ws === 0 ? Theme.muted : Theme.text }
                        IconButton { icon: "chevron-right"; onClicked: root.setRule({ workspace: Math.min(9, parent.ws + 1) }) }
                    }
                }
                // see-through
                Item {
                    width: parent.width; height: 28
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: "See-through" }
                    Toggle {
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        checked: root.opacityOn
                        onToggled: v => root.setRule({ opacity: v ? 0.9 : null })
                    }
                }
                Item {
                    width: parent.width
                    height: root.opacityOn ? 34 : 0
                    opacity: root.opacityOn ? 1 : 0
                    clip: true
                    Behavior on height { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                    Behavior on opacity { NumberAnimation { duration: 180 } }
                    PillSlider {
                        showValue: false
                        width: parent.width
                        icon: "brightness-6"
                        value: root.rule && root.rule.opacity ? (root.rule.opacity - 0.4) / 0.6 : 0.8
                        onMoved: v => root.setRule({ opacity: (0.4 + 0.6 * v).toFixed(2) })
                    }
                }
                // blur
                Item {
                    width: parent.width; height: 28
                    Txt { anchors.verticalCenter: parent.verticalCenter; text: "Blur behind" }
                    Toggle {
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        checked: root.rule ? root.rule.blur === true : false
                        onToggled: v => root.setRule({ blur: v ? true : null })
                    }
                }

                Item {
                    width: parent.width
                    height: root.rule ? 30 : 0
                    opacity: root.rule ? 1 : 0
                    clip: true
                    Behavior on height { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                    Behavior on opacity { NumberAnimation { duration: 180 } }
                    Chip { label: "Forget these settings"; onClicked: root.setRule({ floating: null, workspace: null, opacity: null, blur: null }) }
                }
                Txt {
                    visible: !root.rule
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: "Nothing is remembered for " + root.appName + " yet. Switch something on and every new " + root.appName + " window follows it."
                    color: Theme.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
