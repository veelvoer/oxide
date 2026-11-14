// Empty space. options: width (px)
import QtQuick

Item {
    property var opts: ({})
    property var shell
    implicitWidth: opts.width || 12
    implicitHeight: 1
}
