// Month calendar popup.
import QtQuick

Pop {
    id: root
    closeOnLeave: true
    place: "top-center"
    contentWidth: 330
    contentHeight: 378

    property date shownMonth: new Date()
    onShownChanged: if (shown) shownMonth = new Date()

    readonly property var monthStart: new Date(shownMonth.getFullYear(), shownMonth.getMonth(), 1)
    // Monday-first offset of the 1st
    readonly property int offset: (monthStart.getDay() + 6) % 7
    readonly property int days: new Date(shownMonth.getFullYear(), shownMonth.getMonth() + 1, 0).getDate()

    Rectangle {
        anchors.fill: parent
        radius: Theme.rPop
        color: Theme.surface
        border.color: Theme.line

        Item {
            id: head
            x: 18; y: 14; width: parent.width - 36; height: 40
            Txt {
                anchors.verticalCenter: parent.verticalCenter
                text: Qt.formatDate(root.shownMonth, "MMMM yyyy")
                color: Theme.text
                font { pixelSize: 17; weight: Font.DemiBold }
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                IconButton { icon: "chevron-left"; onClicked: root.shownMonth = new Date(root.shownMonth.getFullYear(), root.shownMonth.getMonth() - 1, 1) }
                IconButton { icon: "chevron-right"; onClicked: root.shownMonth = new Date(root.shownMonth.getFullYear(), root.shownMonth.getMonth() + 1, 1) }
            }
        }
        Row {
            x: 18; y: 62; width: parent.width - 36
            Repeater {
                model: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
                Txt {
                    required property string modelData
                    width: parent.width / 7
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData
                    color: Theme.muted
                    font { pixelSize: 12; weight: Font.Medium }
                }
            }
        }
        Grid {
            x: 18; y: 90; width: parent.width - 36
            columns: 7
            Repeater {
                model: 42
                Item {
                    required property int index
                    readonly property int day: index - root.offset + 1
                    readonly property bool inMonth: day >= 1 && day <= root.days
                    readonly property bool today: inMonth && new Date().getDate() === day && new Date().getMonth() === root.shownMonth.getMonth() && new Date().getFullYear() === root.shownMonth.getFullYear()
                    width: parent.width / 7
                    height: 40
                    Rectangle {
                        anchors.centerIn: parent
                        width: 34; height: 34; radius: Theme.rBox
                        color: parent.today ? Theme.accent : Theme.clear(Theme.accent)
                    }
                    Txt {
                        anchors.centerIn: parent
                        visible: parent.inMonth
                        text: parent.day
                        color: parent.today ? Theme.ink : Theme.text
                        font { pixelSize: 14; weight: parent.today ? Font.Bold : Font.Normal }
                    }
                }
            }
        }
    }
}
