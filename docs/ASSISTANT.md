# Assistant

`Super+A` opens a chat panel with an AI assistant (it is named "Jef" by default; change that in *Settings →
Assistant*). It can use the computer for you, but you decide how much it may do on its own.

## Set up

1. Get an API key at the Anthropic console (*Settings → Assistant → Open console*).
2. Paste it in *Settings → Assistant → API key* (the **Paste** button reads your clipboard).
3. Pick a model (Haiku is the fast, cheap default) and a permission level.

The key is stored in `~/.config/oxide/ai.toml` with owner-only permissions and never leaves your machine except
in requests to the API.

## What it can do

Run commands, read and change Oxide settings, control windows and workspaces, open apps, adjust volume and
brightness, control media, read and set the clipboard, take screenshots, use the mouse and keyboard, fetch web
pages and search the web, set reminders, and remember things you tell it (`~/.config/oxide/ai-memory.json`; the chat is `ai-chat.json`).
The chat and its memory persist between sessions; the trash button in the panel clears the chat.

## Permissions

| Level | Behavior |
|---|---|
| **Ask every time** | every action except looking (reading state, screenshots, memory) asks first |
| **Ask for risky** (default) | harmless actions run directly; anything that changes or deletes data, uses sudo, installs software, kills processes, uses the network to download-and-run, or is not clearly read-only asks first |
| **Never ask** | everything runs; a warning shows in the settings. Only for people who trust it fully. |

Confirmations show the exact command in the panel with *Allow* and *Deny*.

## Use it in your own shell

The assistant is two files, `AIClient.qml` (talks to the `oxide-ai` process) and `AIPanel.qml` (a ready chat UI).
Copy them into your Quickshell config, add `AIClient { id: ai }` and `AIPanel { ai: ai; shown: open }`, and call
`ai.send("turn the volume down")` from anywhere. *Settings → Assistant → Use it in your own Quickshell* has the
snippet with a copy button.

## Privacy

Only what you type, tool results the assistant asks for, and screenshots it takes itself are sent. Nothing is
sent until you send a message.
