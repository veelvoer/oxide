// A Material Design icon (SVG in ./icons) tinted with any color.
import QtQuick
import QtQuick.Effects

Item {
    id: root
    property string name: ""
    property color color: Theme.text
    property real size: 18
    implicitWidth: size
    implicitHeight: size

    Image {
        id: img
        anchors.fill: parent
        source: root.name !== "" ? Qt.resolvedUrl("icons/" + root.name + ".svg") : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        visible: false
        smooth: true
        mipmap: true
    }
    MultiEffect {
        anchors.fill: img
        source: img
        colorization: 1.0
        colorizationColor: root.color
        Behavior on colorizationColor { ColorAnimation { duration: Theme.fast } }
    }
}
