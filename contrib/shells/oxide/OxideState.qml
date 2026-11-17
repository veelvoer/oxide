// Live state of the Oxide compositor.
//
// Oxide listens on $XDG_RUNTIME_DIR/oxide.sock (newline separated JSON). We connect once and send
// {"cmd":"subscribe"}: Oxide answers with the current state and then pushes a new line on every change
// (workspace switch, focus change, new window, title change ...). To make Oxide do something, run
// `oxide-ctl` (see dispatch() below).
pragma Singleton
import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property bool connected: false
    property int active: 1
    property string layout: ""
    property var workspaces: []     // [{id, windows, visible, special}]
    property var focused: null      // {app_id, title} or null
    property var windows: []
    property var outputs: []
    // the config file has an error ({message, line, path}) or null; Super held alone (ambient HUD)
    property var configError: null
    property var whatsNew: null
    property bool superHeld: false
    // the screen new popups open on: the one the pointer / focus is on
    readonly property var activeScreen: {
        const o = outputs.find(o => o.focused);
        return (o && Quickshell.screens.find(s => s.name === o.name)) || Quickshell.screens[0];
    }
    function workspaceOn(screenName) {
        const o = outputs.find(o => o.name === screenName);
        return o && o.workspace ? o.workspace : active;
    }

    // Ask Oxide to run an action, e.g. dispatch({type: "workspace", arg: 3}) or dispatch({type: "close"}).
    function dispatch(action) {
        ctl.command = ["oxide-ctl", "dispatch", JSON.stringify(action)];
        ctl.running = true;
    }
    function workspace(n) { dispatch({ "type": "workspace", "arg": n }); }

    // the compositor asks the shell to open a panel (a key bind): the shell runs the function of that name
    signal call(string name)
    property bool plain: false

    Socket {
        id: sock
        path: Quickshell.env("OXIDE_SOCKET") || (Quickshell.env("XDG_RUNTIME_DIR") + "/oxide.sock")
        connected: true
        onConnectedChanged: {
            root.connected = connected;
            if (connected) {
                // as the shell: key binds then reach us directly (see `call`); an older Oxide does not know that, and
                // the plain subscription follows its error
                root.plain = false;
                write("{\"cmd\":\"subscribe_shell\"}\n");
                flush();
            }
        }
        parser: SplitParser {
            onRead: line => {
                try {
                    const msg = JSON.parse(line);
                    if (msg.status === "call") { root.call(msg.call); return; }
                    if (msg.status === "error" && !root.plain) { root.plain = true; sock.write("{\"cmd\":\"subscribe\"}\n"); sock.flush(); return; }
                    if (msg.status !== "state") return;
                    const s = msg.state;
                    root.active = s.active_workspace;
                    root.layout = s.layout;
                    root.workspaces = s.workspaces;
                    root.focused = s.focused;
                    root.windows = s.windows;
                    root.outputs = s.outputs;
                    root.configError = s.config_error || null;
                    root.whatsNew = s.whats_new || null;
                    root.superHeld = !!s.super_held;
                } catch (e) {
                    console.log("oxide state: bad line", line);
                }
            }
        }
    }
    // Oxide restarted, or is not up yet: keep trying.
    Timer {
        interval: 1000
        running: !sock.connected
        repeat: true
        onTriggered: sock.connected = true
    }
    Process { id: ctl }
}
