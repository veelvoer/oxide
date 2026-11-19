// Quick settings: Wi-Fi, Bluetooth, volume, brightness, media, recent notifications and power buttons.
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import Quickshell.Services.Mpris
import Quickshell.Bluetooth
import Quickshell.Networking
import QtQuick

Pop {
    id: root
    warm: true
    closeOnLeave: true
    required property var shell
    contentWidth: 384
    contentHeight: body.implicitHeight + 24

    readonly property var sink: Pipewire.defaultAudioSink
    PwObjectTracker { objects: [root.sink] }
    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property var wifi: Networking.devices.values.find(d => d.type === DeviceType.Wifi)
    readonly property var wifiNet: wifi ? wifi.networks.values.find(n => n.connected) : null
    readonly property var player: Mpris.players && Mpris.players.values.length > 0 ? Mpris.players.values[0] : null
    property real brightness: 0.5
    property string confirm: ""
    // which tile is opened up into a list: "" | "wifi" | "bt"
    property string expand: ""
    onExpandChanged: if (adapter) adapter.discovering = (expand === "bt" && shown)

    onShownChanged: {
        confirm = "";
        if (!shown) expand = "";
        if (shown) { getBright.running = true; }
    }
    Process {
        id: getBright
        command: ["brightnessctl", "-m"]
        stdout: SplitParser { onRead: l => { const p = parseInt(l.split(",")[3]); if (!isNaN(p)) root.brightness = p / 100; } }
    }
    Process { id: setBright }
    Process { id: power }
    function run(cmd) { power.command = cmd; power.running = true; root.dismissed(); }

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        Column {
            id: body
            x: 12; y: 12
            width: parent.width - 24
            spacing: 10

            // ---- tiles ----
            Grid {
                width: parent.width
                columns: 2
                spacing: 8
                component Tile: Rectangle {
                    id: tile
                    property string icon: ""
                    property string title: ""
                    property string sub: ""
                    property bool on: false
                    // switchable tiles: a click on the tile opens its list, the switch turns it on or off
                    property bool switchable: false
                    property bool open: false
                    property int order: 0
                    signal clicked()
                    signal toggled()
                    width: (body.width - 8) / 2
                    height: Theme.expressive ? 72 : 84
                    // Expressive: an on tile fills with the container color and turns squarer; pressing squishes it
                    radius: !Theme.expressive ? Theme.rBox : (tm.pressed || dm.pressed) ? 14 : tile.on ? 20 : 32
                    color: Theme.expressive ? (tile.on ? (tm.containsMouse ? Qt.tint(Theme.container, Theme.alpha(Theme.onContainer, 0.08)) : Theme.container)
                                                       : (tm.containsMouse ? Theme.surfaceHighest : Theme.surfaceHigh))
                         : (tm.containsMouse ? Theme.line : Theme.raised)
                    border.color: Theme.expressive ? (open ? Theme.accent : "transparent") : (open ? Theme.alpha(Theme.accent, 0.6) : Theme.alpha(Theme.line, 0.6))
                    border.width: Theme.expressive && open ? 2 : 1
                    Behavior on color { ColorAnimation { duration: Theme.normal } }
                    Behavior on border.color { ColorAnimation { duration: Theme.fast } }
                    Behavior on radius { NumberAnimation { duration: Theme.spatial; easing.type: Easing.OutBack; easing.overshoot: 1.4 } }
                    MouseArea { id: tm; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: tile.clicked() }
                    // the round icon is the switch (on = accent); the rest of the tile opens its list
                    Rectangle {
                        id: dot
                        // Expressive: icon on the left, text beside it; otherwise icon on top, text below
                        x: Theme.expressive ? 14 : 12
                        y: Theme.expressive ? (tile.height - height) / 2 : 10
                        width: Theme.expressive ? 36 : 30; height: width
                        radius: Theme.expressive && tile.on ? 12 : width / 2
                        Behavior on radius { NumberAnimation { duration: Theme.spatial; easing.type: Easing.OutBack; easing.overshoot: 1.4 } }
                        color: tile.on ? Theme.accent : (Theme.expressive ? Theme.surfaceHighest : Theme.surface)
                        Behavior on color { ColorAnimation { duration: Theme.normal } }
                        scale: dm.pressed ? 0.92 : 1
                        Behavior on scale { NumberAnimation { duration: Theme.fast; easing.type: Easing.OutCubic } }
                        Icon { anchors.centerIn: parent; name: tile.icon; size: 17; color: tile.on ? Theme.ink : Theme.muted }
                        MouseArea { id: dm; anchors.fill: parent; anchors.margins: -3; cursorShape: Qt.PointingHandCursor; onClicked: tile.switchable ? tile.toggled() : tile.clicked() }
                    }
                    Icon {
                        visible: tile.switchable
                        anchors { right: parent.right; rightMargin: 12; top: Theme.expressive ? undefined : parent.top; topMargin: 14; verticalCenter: Theme.expressive ? parent.verticalCenter : undefined }
                        name: "chevron-down"; size: 16; color: Theme.muted
                        rotation: tile.open ? 180 : 0
                        Behavior on rotation { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic } }
                    }
                    Column {
                        x: Theme.expressive ? dot.x + dot.width + 12 : 14
                        y: Theme.expressive ? (tile.height - height) / 2 : tile.height - height - 10
                        width: parent.width - x - (Theme.expressive && tile.switchable ? 34 : 14)
                        spacing: 0
                        Txt { width: parent.width; elide: Text.ElideRight; text: tile.title; color: Theme.expressive && tile.on ? Theme.onContainer : Theme.text; font { pixelSize: 14; weight: Font.DemiBold } }
                        Txt { width: parent.width; elide: Text.ElideRight; text: tile.sub; color: Theme.expressive && tile.on ? Theme.alpha(Theme.onContainer, 0.75) : Theme.muted; font.pixelSize: 12 }
                    }
                    Arrive { when: root.shown; order: tile.order }
                    Elastic { radius: tile.radius }
                }

                Tile {
                    order: 0
                    icon: Networking.wifiEnabled ? "wifi" : "wifi-off"
                    title: "Wi-Fi"
                    sub: !Networking.wifiEnabled ? "Off" : (root.wifiNet ? root.wifiNet.name : "Not connected")
                    on: Networking.wifiEnabled
                    switchable: true
                    open: root.expand === "wifi"
                    onClicked: root.expand = root.expand === "wifi" ? "" : "wifi"
                    onToggled: Networking.wifiEnabled = !Networking.wifiEnabled
                }
                Tile {
                    order: 1
                    visible: !!root.adapter
                    icon: root.adapter && root.adapter.enabled ? "bluetooth" : "bluetooth-off"
                    title: "Bluetooth"
                    sub: !root.adapter || !root.adapter.enabled ? "Off" : (root.adapter.devices.values.filter(d => d.connected).length + " connected")
                    on: !!root.adapter && root.adapter.enabled
                    switchable: true
                    open: root.expand === "bt"
                    onClicked: root.expand = root.expand === "bt" ? "" : "bt"
                    onToggled: if (root.adapter) root.adapter.enabled = !root.adapter.enabled
                }
                Tile {
                    order: 2
                    icon: root.shell.dnd ? "bell-off-outline" : "bell-outline"
                    title: "Do not disturb"
                    sub: root.shell.dnd ? "On" : "Off"
                    on: root.shell.dnd
                    onClicked: root.shell.dnd = !root.shell.dnd
                }
                Tile {
                    order: 3
                    icon: "cog-outline"
                    title: "Settings"
                    sub: "Oxide"
                    onClicked: { Quickshell.execDetached(["oxide-settings"]); root.dismissed(); }
                }
            }

            // ---- performance: eco / normal / performance ----
            Rectangle {
                id: perf
                width: parent.width
                height: 64
                radius: Theme.rBox
                color: Theme.alpha(Theme.text, 0.05)
                readonly property var items: [
                    { id: "eco", icon: "leaf", label: "Eco", hint: "Longest battery" },
                    { id: "normal", icon: "speedometer", label: "Normal", hint: "Balanced" },
                    { id: "performance", icon: "rocket-launch-outline", label: "Performance", hint: "Full speed" }
                ]
                readonly property int cur: Math.max(0, items.findIndex(i => i.id === OxideState.profileChoice))
                readonly property real cellW: (width - 8) / 3
                // auto-eco on a low battery: the picked one stays marked, the one in force gets a dot
                readonly property bool forced: OxideState.profile !== OxideState.profileChoice
                Rectangle {
                    y: 4; height: parent.height - 8; width: perf.cellW; radius: Theme.rBox - 2
                    x: 4 + perf.cur * perf.cellW
                    color: Theme.expressive ? Theme.container : Theme.accent
                    Behavior on x { NumberAnimation { duration: 240 * Theme.motionScale; easing.type: Easing.OutBack; easing.overshoot: 0.8 } }
                }
                Row {
                    x: 4; y: 4
                    Repeater {
                        model: perf.items
                        delegate: Item {
                            required property var modelData
                            required property int index
                            width: perf.cellW; height: perf.height - 8
                            readonly property bool sel: index === perf.cur
                            Column {
                                anchors.centerIn: parent
                                spacing: 1
                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 6
                                    Icon { name: modelData.icon; size: 16; color: sel ? (Theme.expressive ? Theme.onContainer : Theme.onAccent) : Theme.muted; anchors.verticalCenter: parent.verticalCenter }
                                    Txt { text: modelData.label; color: sel ? (Theme.expressive ? Theme.onContainer : Theme.onAccent) : Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
                                }
                                Txt {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: sel && perf.forced ? "Eco: battery low" : modelData.hint
                                    color: sel ? Theme.alpha(Theme.expressive ? Theme.onContainer : Theme.onAccent, 0.75) : Theme.muted
                                    font.pixelSize: 11
                                }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: OxideState.dispatch({ "type": "set_profile", "arg": modelData.id })
                            }
                        }
                    }
                }
            }

            // ---- the opened tile: networks or devices to connect to ----
            Rectangle {
                id: listBox
                // folds open and shut like a drawer
                visible: height > 0.5
                opacity: root.expand !== "" ? 1 : 0
                width: parent.width
                height: root.expand !== "" ? Math.min(6, Math.max(netList.count, 1)) * 40 + 12 : 0
                Behavior on height { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
                Behavior on opacity { NumberAnimation { duration: 180 } }
                radius: Theme.rBox
                color: Theme.alpha(Theme.text, 0.05)
                clip: true

                ListView {
                    id: netList
                    x: 6; y: 6
                    width: parent.width - 12
                    height: parent.height - 12
                    clip: true
                    model: root.expand === "wifi" ? (root.wifi && Networking.wifiEnabled ? root.wifi.networks : null)
                         : root.expand === "bt" ? (root.adapter && root.adapter.enabled ? root.adapter.devices : null) : null
                    delegate: Rectangle {
                        id: nrow
                        required property var modelData
                        readonly property bool isWifi: root.expand === "wifi"
                        readonly property bool connected: !!modelData.connected
                        width: netList.width
                        height: 40
                        radius: Theme.rControl
                        color: nm.containsMouse ? Theme.raised : Theme.clear(Theme.raised)
                        Behavior on color { ColorAnimation { duration: Theme.fast } }
                        Icon {
                            x: 10; anchors.verticalCenter: parent.verticalCenter
                            size: 18
                            name: nrow.isWifi ? "wifi" : (nrow.connected ? "bluetooth-connect" : "bluetooth")
                            color: nrow.connected ? Theme.accent : Theme.muted
                        }
                        Txt {
                            x: 38; anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 38 - 70
                            elide: Text.ElideRight
                            text: nrow.modelData.name || nrow.modelData.address || "Unknown"
                            color: Theme.text
                            font.pixelSize: 13
                        }
                        Txt {
                            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                            text: nrow.connected ? "Connected" : ""
                            color: Theme.accent
                            font.pixelSize: 12
                        }
                        MouseArea {
                            id: nm
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                try {
                                    if (nrow.connected) nrow.modelData.disconnect();
                                    else nrow.modelData.connect();
                                } catch (e) {
                                    // networks that need a password: the settings app asks for it
                                    Quickshell.execDetached(["oxide-settings", nrow.isWifi ? "wifi" : "bluetooth"]);
                                    root.dismissed();
                                }
                            }
                        }
                    }
                }
                Txt {
                    visible: netList.count === 0
                    x: 14; anchors.verticalCenter: parent.verticalCenter
                    text: root.expand === "wifi" ? (Networking.wifiEnabled ? "No networks found." : "Turn Wi-Fi on to see networks.")
                                                 : (root.adapter && root.adapter.enabled ? "Searching for devices..." : "Turn Bluetooth on to see devices.")
                    color: Theme.muted
                    font.pixelSize: 13
                }
            }

            // ---- sliders ----
            PillSlider {
                Arrive { when: root.shown; order: 4 }
                width: parent.width
                icon: root.sink && root.sink.audio && (root.sink.audio.muted || root.sink.audio.volume <= 0) ? "volume-off" : "volume-high"
                value: root.sink && root.sink.audio ? root.sink.audio.volume : 0
                muted: root.sink && root.sink.audio ? root.sink.audio.muted : false
                onMoved: v => { if (root.sink && root.sink.audio) { root.sink.audio.muted = false; root.sink.audio.volume = v; } }
                onIconClicked: if (root.sink && root.sink.audio) root.sink.audio.muted = !root.sink.audio.muted
            }
            PillSlider {
                Arrive { when: root.shown; order: 5 }
                width: parent.width
                icon: "brightness-6"
                value: root.brightness
                onMoved: v => { root.brightness = v; setBright.command = ["brightnessctl", "set", Math.max(1, Math.round(v * 100)) + "%"]; setBright.running = true; }
            }

            // ---- media ----
            Rectangle {
                visible: !!root.player
                width: parent.width
                height: visible ? 66 : 0
                radius: Theme.rBox
                color: Theme.alpha(Theme.text, 0.06)
                Rectangle {
                    x: 12; anchors.verticalCenter: parent.verticalCenter
                    width: 42; height: 42; radius: Theme.rControl
                    color: Theme.alpha(Theme.accent, 0.2)
                    Icon { anchors.centerIn: parent; name: "music-note-outline"; size: 22; color: Theme.accent }
                }
                Column {
                    x: 64; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 64 - 112
                    Txt { width: parent.width; elide: Text.ElideRight; text: root.player ? root.player.trackTitle : ""; color: Theme.text; font { pixelSize: 14; weight: Font.DemiBold } }
                    Txt { width: parent.width; elide: Text.ElideRight; text: root.player ? root.player.trackArtist : ""; color: Theme.muted; font.pixelSize: 12 }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    IconButton { icon: "skip-previous"; onClicked: if (root.player) root.player.previous() }
                    IconButton { icon: root.player && root.player.isPlaying ? "pause" : "play"; onClicked: if (root.player) root.player.togglePlaying() }
                    IconButton { icon: "skip-next"; onClicked: if (root.player) root.player.next() }
                }
            }

            // ---- notifications ----
            Column {
                visible: root.shell.notificationCount > 0
                width: parent.width
                spacing: 6
                Item {
                    width: parent.width; height: 24
                    Txt { text: "Notifications"; color: Theme.muted; font { pixelSize: 12; weight: Font.DemiBold } anchors.verticalCenter: parent.verticalCenter; x: 4 }
                    Txt {
                        anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
                        text: "Clear"
                        color: clearMouse.containsMouse ? Theme.accent : Theme.muted
                        font.pixelSize: 12
                        MouseArea { id: clearMouse; anchors.fill: parent; anchors.margins: -6; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.shell.clearNotifications() }
                    }
                }
                Repeater {
                    model: root.shell.recentNotifications
                    Rectangle {
                        required property var modelData
                        width: body.width
                        height: nCol.implicitHeight + 18
                        radius: Theme.rBox
                        color: Theme.alpha(Theme.text, 0.06)
                        Column {
                            id: nCol
                            x: 12; y: 9
                            width: parent.width - 24
                            spacing: 2
                            Txt { width: parent.width; elide: Text.ElideRight; text: modelData.summary; color: Theme.text; font { pixelSize: 13; weight: Font.DemiBold } }
                            Txt { visible: modelData.body !== ""; width: parent.width; elide: Text.ElideRight; maximumLineCount: 2; wrapMode: Text.Wrap; text: modelData.body; color: Theme.muted; font.pixelSize: 12 }
                        }
                    }
                }
            }

            // ---- power ----
            Row {
                width: parent.width
                spacing: 8
                component PowerBtn: Rectangle {
                    id: pb
                    property string icon: ""
                    property string label: ""
                    property var cmd: []
                    property bool armed: root.confirm === label
                    property int order: 6
                    width: (body.width - 24) / 4
                    height: 52
                    radius: Theme.expressive ? (pm.pressed ? 12 : height / 2) : Theme.rBox
                    color: armed ? Theme.bad : Theme.expressive ? (pm.containsMouse ? Theme.surfaceHighest : Theme.surfaceHigh)
                                                            : (pm.containsMouse ? Theme.raised : Theme.alpha(Theme.text, 0.06))
                    Behavior on color { ColorAnimation { duration: Theme.fast } }
                    Behavior on radius { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack; easing.overshoot: 1.6 } }
                    Column {
                        anchors.centerIn: parent
                        spacing: 2
                        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: pb.icon; size: 19; color: pb.armed ? Theme.ink : Theme.text }
                        Txt { anchors.horizontalCenter: parent.horizontalCenter; text: pb.armed ? "Sure?" : pb.label; color: pb.armed ? Theme.ink : Theme.muted; font.pixelSize: 11 }
                    }
                    Timer { id: disarm; interval: 3000; onTriggered: if (root.confirm === pb.label) root.confirm = "" }
                    MouseArea {
                        id: pm
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (pb.label === "Lock" || pb.armed) root.run(pb.cmd);
                            else { root.confirm = pb.label; disarm.restart(); }
                        }
                    }
                    Arrive { when: root.shown; order: pb.order }
                    Elastic { radius: pb.radius }
                }
                PowerBtn { order: 6; icon: "lock-outline"; label: "Lock"; cmd: ["sh", "-c", "command -v hyprlock >/dev/null && hyprlock || loginctl lock-session"] }
                PowerBtn { order: 7; icon: "logout"; label: "Log out"; cmd: ["oxide-ctl", "quit"] }
                PowerBtn { order: 8; icon: "restart"; label: "Restart"; cmd: ["systemctl", "reboot"] }
                PowerBtn { order: 9; icon: "power"; label: "Shut down"; cmd: ["systemctl", "poweroff"] }
            }
        }
    }
}
