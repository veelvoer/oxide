// Notification daemon: toasts slide in under the bar, history shows in quick settings.
// (Only one program can own org.freedesktop.Notifications at a time: stop mako/dunst/swaync to use this.)
import Quickshell
import Quickshell.Wayland
import Quickshell.Services.Notifications
import QtQuick

Item {
    id: root
    property bool dnd: false
    property var history: []               // newest first: {uid, appName, summary, body}
    readonly property int count: history.length
    property int nextUid: 1
    property var live: ({})                // uid -> Notification object (needed for actions)

    function clear() { history = []; }
    // remove one entry (and tell the sending app it was dismissed)
    function remove(uid) {
        history = history.filter(h => h.uid !== uid);
        const n = live[uid];
        if (n) { try { n.dismiss(); } catch (e) {} delete live[uid]; }
    }

    ListModel { id: toasts }

    NotificationServer {
        keepOnReload: false
        actionsSupported: true
        bodySupported: true
        bodyMarkupSupported: false
        imageSupported: true
        onNotification: n => {
            n.tracked = true;
            const uid = root.nextUid++;
            root.live[uid] = n;
            root.history = [{ "uid": uid, "appName": n.appName, "summary": n.summary, "body": n.body }].concat(root.history).slice(0, 30);
            if (root.dnd && n.urgency !== NotificationUrgency.Critical) return;
            const actions = [];
            for (const a of n.actions) actions.push({ "id": a.identifier, "text": a.text });
            toasts.append({ "uid": uid, "app": n.appName, "summary": n.summary, "body": n.body, "icon": n.appIcon, "actions": JSON.stringify(actions), "critical": n.urgency === NotificationUrgency.Critical });
        }
    }

    PanelWindow {
        id: toastWin
        screen: Quickshell.screens[0]
        Connections {
            target: OxideState
            function onActiveScreenChanged() { if (toasts.count === 0) toastWin.screen = OxideState.activeScreen; }
        }
        visible: toasts.count > 0
        color: "transparent"
        anchors { top: true; right: true }
        margins { top: BarConfig.barSpace + 18; right: 12 }
        implicitWidth: 372
        implicitHeight: Math.max(1, col.implicitHeight)
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "oxide-toasts"

        Column {
            id: col
            width: parent.width
            spacing: 10

            Repeater {
                model: toasts
                Rectangle {
                    id: toast
                    required property int index
                    required property int uid
                    required property string app
                    required property string summary
                    required property string body
                    required property string icon
                    required property string actions
                    required property bool critical
                    readonly property var actionList: JSON.parse(actions)
                    property bool gone: false

                    width: col.width
                    height: inner.implicitHeight + 24
                    radius: Theme.rBox
                    color: Theme.surface
                    border.color: critical ? Theme.bad : Theme.line
                    opacity: gone ? 0 : 1
                    x: gone ? 40 : 0
                    Behavior on opacity { NumberAnimation { duration: Theme.normal } }
                    Behavior on x { NumberAnimation { duration: Theme.slow; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.curve } }
                    Component.onCompleted: { x = 60; opacity = 0; x = 0; opacity = 1; }

                    function dismiss() { gone = true; kill.start(); }
                    Timer { running: !toast.critical; interval: 7000; onTriggered: toast.dismiss() }
                    Timer { id: kill; interval: 320; onTriggered: { for (let i = 0; i < toasts.count; i++) if (toasts.get(i).uid === toast.uid) { toasts.remove(i); break; } } }

                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: toast.dismiss() }

                    Row {
                        id: inner
                        x: 14; y: 12
                        width: parent.width - 28
                        spacing: 12
                        Rectangle {
                            width: 38; height: 38; radius: Theme.rControl
                            color: "transparent"
                            Image {
                                anchors.centerIn: parent
                                width: 24; height: 24
                                visible: status === Image.Ready
                                source: toast.icon !== "" ? Quickshell.iconPath(toast.icon, true) : ""
                                sourceSize: Qt.size(48, 48)
                            }
                            Icon { anchors.centerIn: parent; visible: toast.icon === ""; name: "bell-outline"; size: 20; color: Theme.muted }
                        }
                        Column {
                            width: parent.width - 50
                            spacing: 3
                            Txt { text: toast.app; color: Theme.muted; font.pixelSize: 11 }
                            Txt { width: parent.width; wrapMode: Text.Wrap; maximumLineCount: 2; elide: Text.ElideRight; text: toast.summary; color: Theme.text; font { pixelSize: 14; weight: Font.DemiBold } }
                            Txt { visible: toast.body !== ""; width: parent.width; wrapMode: Text.Wrap; maximumLineCount: 3; elide: Text.ElideRight; text: toast.body; color: Theme.muted; font.pixelSize: 13 }
                            Row {
                                visible: toast.actionList.length > 0
                                spacing: 6
                                topPadding: 4
                                Repeater {
                                    model: toast.actionList
                                    Rectangle {
                                        required property var modelData
                                        width: label.implicitWidth + 22; height: 28; radius: Theme.rControl
                                        color: am.containsMouse ? Theme.accent : Theme.raised
                                        Behavior on color { ColorAnimation { duration: Theme.fast } }
                                        Txt { id: label; anchors.centerIn: parent; text: modelData.text; color: am.containsMouse ? Theme.ink : Theme.text; font.pixelSize: 12 }
                                        MouseArea {
                                            id: am
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                const n = root.live[toast.uid];
                                                if (n) for (const a of n.actions) if (a.identifier === modelData.id) a.invoke();
                                                toast.dismiss();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
