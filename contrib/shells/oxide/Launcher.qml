// Launcher: the apps you start most come first ("sort": "recent" puts the latest first instead), plus a calculator, ">" commands and web search.
// Options come from ~/.config/oxide/launcher.json (Settings > Launcher):
//   { "calculator": true, "history": true, "web": true, "sort": "used", "searchUrl": "https://duckduckgo.com/?q=%s", "maxResults": 7 }
import Quickshell
import Quickshell.Io
import QtQuick

Pop {
    id: root
    place: "center"
    contentWidth: 560
    contentHeight: 76 + list.height + (list.count > 0 ? 14 : 0)
    wantsKeyboard: true
    warm: true

    property string query: ""
    property var opts: ({})
    property var counts: ({})
    readonly property int maxResults: opts.maxResults || 7
    onShownChanged: if (shown) { counts = Object.assign({}, counts); query = ""; input.text = ""; input.forceActiveFocus(); list.currentIndex = 0; }

    FileView {
        path: Theme.configDir + "/launcher.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { try { root.opts = JSON.parse(text()); } catch (e) { root.opts = ({}); } }
    }
    FileView {
        id: hist
        path: Theme.configDir + "/launcher-history.json"
        onLoaded: { try { root.counts = JSON.parse(text()); } catch (e) { root.counts = ({}); } }
    }

    function calc(q) {
        if (opts.calculator === false) return null;
        if (!/^[0-9+\-*\/().%^ ,]+$/.test(q) || !/[0-9]/.test(q) || !/[+\-*\/%^]/.test(q.replace(/^-/, ""))) return null;
        try {
            const v = eval(q.replace(/,/g, ".").replace(/\^/g, "**"));
            return typeof v === "number" && isFinite(v) ? String(Math.round(v * 1e10) / 1e10) : null;
        } catch (e) { return null; }
    }

    // every app once, with the lowercase texts the search compares against
    property var appIndex: []
    function buildIndex() {
        appIndex = DesktopEntries.applications.values.filter(a => !a.noDisplay).map(a => ({
            entry: a, name: a.name.toLowerCase(), generic: (a.genericName || "").toLowerCase(),
            keywords: (a.keywords || []).map(k => k.toLowerCase())
        }));
    }
    Component.onCompleted: buildIndex()
    Connections { target: DesktopEntries.applications; function onValuesChanged() { root.buildIndex(); } }

    readonly property var results: {
        const q = query.trim(), ql = q.toLowerCase();
        const out = [];
        if (q.startsWith(">")) {
            const c = q.slice(1).trim();
            if (c) out.push({ kind: "cmd", name: "Run " + c, sub: "in a shell", arg: c });
            return out;
        }
        const r = calc(q);
        if (r !== null) out.push({ kind: "calc", name: r, sub: q + " =", arg: r });
        const hist = opts.history !== false;
        // the index is built once (and when apps change): opening and typing stay fast
        const apps = appIndex.filter(x => ql === "" || x.name.includes(ql) || x.generic.includes(ql) || x.keywords.some(k => k.includes(ql)));
        // most used first (or most recent first with "sort": "recent"), then alphabetical
        const info = id => {
            const v = counts[id];
            return typeof v === "number" ? { n: v, t: 0 } : (v || { n: 0, t: 0 });
        };
        apps.sort((a, b) => {
            const as = ql !== "" && a.name.startsWith(ql) ? 0 : 1, bs = ql !== "" && b.name.startsWith(ql) ? 0 : 1;
            const ia = hist ? info(a.entry.id) : { n: 0, t: 0 }, ib = hist ? info(b.entry.id) : { n: 0, t: 0 };
            const byUse = ib.n - ia.n, byTime = ib.t - ia.t;
            // plain comparison of prepared lowercase names (localeCompare is slow in QML's JavaScript)
            return as - bs || (opts.sort === "recent" ? byTime || byUse : byUse || byTime) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
        });
        // enough to scroll through with the arrow keys; building rows nobody sees only costs time
        for (const x of apps.slice(0, 50)) out.push({ kind: "app", name: x.entry.name, sub: x.entry.genericName || "", icon: x.entry.icon, entry: x.entry });
        if (q !== "" && opts.web !== false && out.length < root.maxResults) out.push({ kind: "web", name: "Search the web for " + q, sub: "", arg: q });
        return out;
    }

    function launch() {
        const e = results[list.currentIndex];
        if (!e) return;
        if (e.kind === "app") {
            e.entry.execute();
            if (opts.history !== false) { const v = counts[e.entry.id]; const n = (typeof v === "number" ? v : (v ? v.n : 0)) + 1; counts[e.entry.id] = { n: n, t: Date.now() }; hist.setText(JSON.stringify(counts)); }
        } else if (e.kind === "cmd") Quickshell.execDetached(["sh", "-c", e.arg]);
        else if (e.kind === "calc") Quickshell.execDetached(["sh", "-c", "command -v wl-copy >/dev/null && printf %s \"$0\" | wl-copy", e.arg]);
        else if (e.kind === "web") Quickshell.execDetached(["xdg-open", (opts.searchUrl || "https://duckduckgo.com/?q=%s").replace("%s", encodeURIComponent(e.arg))]);
        root.dismissed();
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.expressive ? "transparent" : Theme.line

        Rectangle {
            x: 14; y: 14
            width: parent.width - 28; height: 48
            radius: Theme.rBox
            color: Theme.raised
            Icon { x: 16; anchors.verticalCenter: parent.verticalCenter; name: "magnify"; size: 20; color: Theme.muted }
            TextInput {
                id: input
                x: 48
                width: parent.width - 64
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.text
                selectionColor: Theme.accent
                font.pixelSize: 17
                clip: true
                onTextChanged: { root.query = text; list.currentIndex = 0; }
                Keys.onDownPressed: list.currentIndex = Math.min(list.count - 1, list.currentIndex + 1)
                Keys.onUpPressed: list.currentIndex = Math.max(0, list.currentIndex - 1)
                Keys.onReturnPressed: root.launch()
                Keys.onEnterPressed: root.launch()
            }
            Txt {
                visible: input.text === ""
                x: 48
                anchors.verticalCenter: parent.verticalCenter
                text: "Search apps, do math, or type > to run a command"
                color: Theme.muted
                font.pixelSize: 16
            }
        }

        ListView {
            id: list
            x: 14; y: 72
            width: parent.width - 28
            height: Math.min(count, root.maxResults) * 52
            model: root.results
            clip: true
            currentIndex: 0
            // one highlight that glides to the selected row
            // Expressive: a tonal pill that springs to the row (a hair past it, then back)
            highlightFollowsCurrentItem: !Theme.expressive
            highlight: Rectangle {
                width: list.width
                y: Theme.expressive && list.currentItem ? list.currentItem.y : y
                height: Theme.expressive && list.currentItem ? list.currentItem.height : height
                radius: Theme.expressive ? height / 2 : Theme.rBox
                color: Theme.expressive ? Theme.secondaryContainer : Theme.alpha(Theme.accent, 0.2)
                Behavior on y { enabled: Theme.expressive; NumberAnimation { duration: Theme.fastMs; easing.type: Easing.OutBack; easing.overshoot: 1.0 } }
            }
            highlightMoveDuration: 150
            highlightMoveVelocity: -1
            highlightResizeDuration: 150
            add: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 130 } }
            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
            Behavior on height { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic } }
            delegate: Rectangle {
                id: item
                required property var modelData
                required property int index
                width: list.width
                height: 52
                radius: Theme.rBox
                color: "transparent"
                Behavior on color { ColorAnimation { duration: Theme.fast } }
                Row {
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 14
                    Image {
                        visible: item.modelData.kind === "app"
                        width: 30; height: 30
                        source: item.modelData.icon ? Quickshell.iconPath(item.modelData.icon, true) : ""
                        sourceSize: Qt.size(60, 60)
                        smooth: true
                        // decoded off the main thread: the list shows at once, icons follow a moment later
                        asynchronous: true
                        cache: true
                    }
                    Icon {
                        visible: item.modelData.kind !== "app"
                        anchors.verticalCenter: parent.verticalCenter
                        name: item.modelData.kind === "web" ? "web" : item.modelData.kind === "cmd" ? "console-line" : "check"
                        size: 22; color: Theme.muted
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        Txt { text: item.modelData.name; color: Theme.text; font { pixelSize: item.modelData.kind === "calc" ? 18 : 15; weight: Font.Medium; family: item.modelData.kind === "calc" ? Theme.mono : Theme.sans } }
                        Txt { visible: text !== ""; text: item.modelData.sub || ""; color: Theme.muted; font.pixelSize: 12 }
                    }
                }
                // one highlight only: moving the pointer onto a row selects it (a row that merely appears under a still pointer does not)
                HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor; onPointChanged: if (hovered) list.currentIndex = item.index }
                TapHandler { onTapped: { list.currentIndex = item.index; root.launch(); } }
                Arrive { when: root.shown; order: item.index }
                Elastic { radius: item.height / 2 }
            }
        }
    }
}
