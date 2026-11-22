// System state used by several bar modules and by quick settings, in one place.
pragma Singleton
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower
import Quickshell.Services.Mpris
import Quickshell.Bluetooth
import Quickshell.Networking
import QtQuick

Singleton {
    id: root

    // ---- audio ----
    readonly property var sink: Pipewire.defaultAudioSink
    PwObjectTracker { objects: [root.sink] }
    readonly property real volume: sink && sink.audio ? sink.audio.volume : 0
    readonly property bool muted: sink && sink.audio ? sink.audio.muted : false
    readonly property string volumeIcon: muted || volume <= 0 ? "volume-off" : volume < 0.34 ? "volume-low" : volume < 0.67 ? "volume-medium" : "volume-high"
    function setVolume(v) { if (sink && sink.audio) { sink.audio.muted = false; sink.audio.volume = Math.max(0, Math.min(1, v)); } }
    function toggleMute() { if (sink && sink.audio) sink.audio.muted = !sink.audio.muted; }

    // ---- battery ----
    readonly property var battery: UPower.displayDevice
    readonly property bool hasBattery: battery && battery.isPresent && battery.isLaptopBattery
    readonly property int batteryPct: battery ? Math.round(battery.percentage * 100) : 0
    readonly property bool charging: battery && battery.state === UPowerDeviceState.Charging
    readonly property string batteryIcon: charging ? "battery-charging" : batteryPct < 15 ? "battery-alert-variant-outline" : batteryPct < 40 ? "battery-low" : batteryPct < 75 ? "battery-medium" : "battery-high"

    // ---- network and bluetooth ----
    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property var wifi: Networking.devices.values.find(d => d.type === DeviceType.Wifi)
    readonly property var wifiNet: wifi ? wifi.networks.values.find(n => n.connected) : null
    readonly property string wifiIcon: {
        if (!Networking.wifiEnabled) return "wifi-off";
        if (!wifiNet) return "wifi-strength-off-outline";
        const s = wifiNet.signalStrength;
        return s >= 0.75 ? "wifi-strength-4" : s >= 0.5 ? "wifi-strength-3" : s >= 0.25 ? "wifi-strength-2" : "wifi-strength-1";
    }

    // ---- media ----
    readonly property var player: Mpris.players && Mpris.players.values.length > 0 ? Mpris.players.values[0] : null

    // ---- cpu and memory (sampled every 2 s, only while a module asks for it) ----
    property int watchers: 0
    property real cpu: 0
    property real memory: 0
    property var lastCpu: null
    Process {
        id: sampler
        command: ["sh", "-c", "head -1 /proc/stat; grep -E '^(MemTotal|MemAvailable)' /proc/meminfo"]
        property var mem: ({})
        stdout: SplitParser {
            onRead: line => {
                if (line.startsWith("cpu ")) {
                    const f = line.trim().split(/\s+/).slice(1).map(Number);
                    const idle = f[3] + f[4], total = f.reduce((a, b) => a + b, 0);
                    if (root.lastCpu) {
                        const dt = total - root.lastCpu.total, di = idle - root.lastCpu.idle;
                        if (dt > 0) root.cpu = 1 - di / dt;
                    }
                    root.lastCpu = { "idle": idle, "total": total };
                } else if (line.startsWith("MemTotal")) {
                    sampler.mem.total = parseInt(line.split(/\s+/)[1]);
                } else if (line.startsWith("MemAvailable")) {
                    sampler.mem.avail = parseInt(line.split(/\s+/)[1]);
                    if (sampler.mem.total) root.memory = 1 - sampler.mem.avail / sampler.mem.total;
                }
            }
        }
    }
    Timer { interval: 2000; running: root.watchers > 0; repeat: true; triggeredOnStart: true; onTriggered: sampler.running = true }
}
