// Now playing (any MPRIS player). Click toggles play/pause. Hidden when nothing plays. options: maxWidth
import QtQuick
import Oxide

Row {
    id: root
    property var opts: ({})
    property var shell
    readonly property var p: Sys.player
    // false = nothing to show: the bar gives this module no room at all
    readonly property bool wanted: !!p && (p.trackTitle || "") !== ""
    visible: wanted
    spacing: 8
    Icon { anchors.verticalCenter: parent.verticalCenter; name: root.p && root.p.isPlaying ? "music-note-outline" : "pause"; size: 15; color: Theme.muted }
    Txt {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, root.opts.maxWidth || 220)
        elide: Text.ElideRight
        text: root.p ? ((root.p.trackArtist ? root.p.trackArtist + " - " : "") + root.p.trackTitle) : ""
        color: Theme.muted
        font.pixelSize: 14
    }
    TapHandler { onTapped: if (root.p) root.p.togglePlaying() }
    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
