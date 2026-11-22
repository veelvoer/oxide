// Oxide default shell. Everything lives in this folder: edit freely, then restart the shell from
// Oxide Settings > Shell (or `oxide-ctl restart-shell`).
//
//   Theme.qml        colors, sizes, animation curve
//   OxideState.qml   live data from the compositor (workspaces, windows) + how to send actions
//   Bar.qml          the bar (Workspaces, Tray, clock, status cluster)
//   ControlCenter    quick settings         Calendar     month view
//   Notifications    toasts + history       Launcher     app launcher
//   LockScreen       lock screen (Super+N)
//   Clipboard        clipboard history       Wallpaper    desktop background
//   AIClient + AIPanel   the AI assistant that can control the computer (see AIClient.qml)
//
// Key binds (Oxide Settings > Shortcuts):  Super+Z quick settings, Super+D launcher, Super+A assistant.
// They call `quickshell -p ~/.config/oxide/active-shell ipc call <target> <function>` (see IpcHandler below).
import Quickshell
import Quickshell.Io
import QtQuick
import Oxide

ShellRoot {
    id: root

    property bool controlShown: false
    property bool calendarShown: false
    property bool launcherShown: false
    property bool aiShown: false
    property bool windowShown: false
    property bool clipboardShown: false
    property bool notifShown: false
    // opened by clicking in the bar (not by a key): the panel then closes once the pointer is away from it
    property bool openedByMouse: false

    property alias dnd: notifs.dnd
    readonly property int notificationCount: notifs.count
    readonly property var recentNotifications: notifs.history.slice(0, 3)
    readonly property var allNotifications: notifs.history

    function closeMenus() { windowShown = false; controlShown = false; calendarShown = false; launcherShown = false; clipboardShown = false; notifShown = false; }
    function toggleControl(mouse) { const v = !controlShown; closeMenus(); controlShown = v; openedByMouse = mouse === true; }
    function toggleCalendar(mouse) { const v = !calendarShown; closeMenus(); calendarShown = v; openedByMouse = mouse === true; }
    function toggleLauncher() { const v = !launcherShown; closeMenus(); launcherShown = v; }
    function toggleNotifications(mouse) { const v = !notifShown; closeMenus(); notifShown = v; openedByMouse = mouse === true; }
    function toggleWindow() { const v = !windowShown; closeMenus(); windowShown = v; }
    function toggleClipboard() { const v = !clipboardShown; closeMenus(); clipboardShown = v; }
    // Super+A: open it, or put the cursor back into the chat if it is open but you were elsewhere; closed by pressing it again
    function toggleAi() {
        if (aiShown && !aiPanel.inputFocused) { aiPanel.refocus(); return; }
        aiShown = !aiShown;
        if (aiShown) closeMenus();
    }
    function clearNotifications() { notifs.clear(); }
    function dismissNotification(uid) { notifs.remove(uid); }

    Wallpaper { }
    LockScreen { id: lockScreen }
    Notifications { id: notifs }
    AIClient { id: ai }
    // remembers what you copy, for the clipboard panel (stops with the shell)
    Process { running: true; command: ["oxide-ctl", "clip", "watch"] }

    Variants {
        model: Quickshell.screens
        Bar {
            required property var modelData
            screen: modelData
            shell: root
        }
    }

    ControlCenter { shell: root; mouseOpened: root.openedByMouse; shown: root.controlShown; onDismissed: root.controlShown = false }
    Calendar { mouseOpened: root.openedByMouse; shown: root.calendarShown; onDismissed: root.calendarShown = false }
    Launcher { shown: root.launcherShown; onDismissed: root.launcherShown = false }
    NotificationCenter { shell: root; mouseOpened: root.openedByMouse; shown: root.notifShown; onDismissed: root.notifShown = false }
    Hud {}
    ConfigError {}
    WhatsNew {}
    WindowOptions { shown: root.windowShown; onDismissed: root.windowShown = false }
    Clipboard { shown: root.clipboardShown; onDismissed: root.clipboardShown = false }
    AIPanel { id: aiPanel; ai: ai; shown: root.aiShown; onDismissed: root.aiShown = false }

    // `quickshell ipc call shell toggleControl` etc.
    // key binds arrive straight from the compositor: no process has to start first
    Connections {
        target: OxideState
        function onCall(name) { if (typeof root[name] === "function") root[name](); }
    }

    IpcHandler {
        target: "shell"
        function toggleControl(): void { root.toggleControl(); }
        function toggleCalendar(): void { root.toggleCalendar(); }
        function toggleLauncher(): void { root.toggleLauncher(); }
        function toggleClipboard(): void { root.toggleClipboard(); }
        function toggleWindow(): void { root.toggleWindow(); }
        function toggleNotifications(): void { root.toggleNotifications(); }
        function lock(): void { root.closeMenus(); lockScreen.lock(); }
    }
    IpcHandler {
        target: "ai"
        function toggle(): void { root.toggleAi(); }
        function open(): void { root.aiShown = true; root.closeMenus(); }
        function ask(text: string): void { root.aiShown = true; ai.send(text); }
    }
}
