// AIClient: talks to the `oxide-ai` assistant. This file has no visuals, so you can build your own UI on it.
//
// HOW TO ADD THE ASSISTANT TO YOUR OWN QUICKSHELL
//   1. Copy this file (and AIPanel.qml if you like the chat UI, plus Theme.qml/Icon.qml/Pop.qml/icons/ it uses).
//   2. Create one client in your ShellRoot:      AIClient { id: ai }
//   3. Show the messages anywhere:               ListView { model: ai.messages ... }   (see AIPanel.qml)
//   4. Send:  ai.send("turn the volume down")    Answer a permission question: ai.confirm(id, true|false)
//   5. Put your Anthropic API key in Oxide Settings > Assistant (or ANTHROPIC_API_KEY in the environment).
//
// Under the hood it runs the `oxide-ai` program and talks JSON lines over stdin/stdout. Messages in
// `ai.messages` have a `kind`: "user", "assistant", "tool" or "error". A "tool" entry has a `status`:
// "running", "confirm" (waiting for you: show Allow/Deny), "ok", "failed" or "denied".
import Quickshell.Io
import QtQuick

Item {
    id: root

    property bool ready: false          // oxide-ai is running
    property bool hasKey: true          // an API key is configured
    property bool busy: false           // the assistant is working on a request
    property bool thinking: false       // waiting for the first words
    property string model: ""
    property string name: ""
    property string permission: "safe"  // "ask" | "safe" | "full"
    readonly property ListModel messages: ListModel {}
    property int streaming: -1          // index of the assistant message being written

    function send(text) {
        const t = text.trim();
        if (t === "" || busy) return;
        messages.append(blank({ "kind": "user", "text": t }));
        busy = true;
        thinking = true;
        proc.write(JSON.stringify({ "type": "user", "text": t }) + "\n");
    }
    function confirm(id, allow) {
        const i = indexOfTool(id);
        if (i >= 0) messages.setProperty(i, "status", allow ? "running" : "denied");
        proc.write(JSON.stringify({ "type": "confirm_reply", "id": id, "allow": allow }) + "\n");
    }
    function cancel() { proc.write(JSON.stringify({ "type": "cancel" }) + "\n"); }
    function reset() {
        proc.write(JSON.stringify({ "type": "reset" }) + "\n");
        messages.clear();
        busy = false;
        thinking = false;
        streaming = -1;
    }
    function reload() { proc.write(JSON.stringify({ "type": "reload" }) + "\n"); }

    function blank(o) {
        return Object.assign({ "kind": "", "text": "", "tid": 0, "name": "", "input": "", "status": "", "output": "", "summary": "", "risk": "low" }, o);
    }
    function indexOfTool(id) {
        for (let i = messages.count - 1; i >= 0; i--) if (messages.get(i).kind === "tool" && messages.get(i).tid === id) return i;
        return -1;
    }

    function handle(ev) {
        switch (ev.type) {
        case "ready":
            ready = true; hasKey = ev.has_key; model = ev.model; permission = ev.permission; name = ev.name || "";
            break;
        case "restore":
            // the conversation from last time
            messages.clear();
            for (const it of ev.items) messages.append(blank({ "kind": it.role === "user" ? "user" : "assistant", "text": it.text }));
            break;
        case "turn_start":
            busy = true; thinking = true;
            break;
        case "thinking":
            thinking = true;
            break;
        case "assistant_start":
            thinking = false;
            messages.append(blank({ "kind": "assistant" }));
            streaming = messages.count - 1;
            break;
        case "text_delta":
            if (streaming >= 0) messages.setProperty(streaming, "text", messages.get(streaming).text + ev.text);
            break;
        case "assistant_end":
            streaming = -1;
            break;
        case "tool_start":
            thinking = false;
            messages.append(blank({ "kind": "tool", "tid": ev.id, "name": ev.name, "input": JSON.stringify(ev.input), "status": "running" }));
            break;
        case "confirm": {
            const i = indexOfTool(ev.id);
            if (i >= 0) { messages.setProperty(i, "status", "confirm"); messages.setProperty(i, "summary", ev.summary); messages.setProperty(i, "risk", ev.risk); }
            break;
        }
        case "tool_result": {
            const i = indexOfTool(ev.id);
            if (i >= 0) {
                if (messages.get(i).status !== "denied") messages.setProperty(i, "status", ev.ok ? "ok" : (ev.output === "Denied" ? "denied" : "failed"));
                messages.setProperty(i, "output", ev.output);
            }
            thinking = true;
            break;
        }
        case "error":
            thinking = false;
            messages.append(blank({ "kind": "error", "text": ev.message }));
            break;
        case "done":
            busy = false; thinking = false; streaming = -1;
            break;
        }
    }

    Process {
        id: proc
        command: ["oxide-ai"]
        running: true
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => {
                try { root.handle(JSON.parse(line)); } catch (e) { console.log("oxide-ai: bad line", line); }
            }
        }
        onExited: {
            root.ready = false; root.busy = false; root.thinking = false;
            restart.start();
        }
    }
    Timer { id: restart; interval: 2000; onTriggered: proc.running = true }
}
