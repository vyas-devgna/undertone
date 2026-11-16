import QtQuick
import QtQuick.Controls

Button {
    id: control
    property bool selected: false
    property bool primary: false
    property bool quiet: false
    implicitHeight: 42
    horizontalPadding: 13
    verticalPadding: 6
    topInset: 0
    bottomInset: 0
    leftInset: 0
    rightInset: 0
    hoverEnabled: true

    background: Rectangle {
        color: control.primary ? Qt.tint(window.accent, control.down ? "#28000000" : control.hovered ? "#1affffff" : "#00000000")
            : control.down ? window.line : control.selected || control.hovered ? window.raised : "transparent"
        border.width: control.quiet && !control.visualFocus && !control.selected ? 0 : 1
        border.color: control.visualFocus ? window.ink : control.primary || control.selected ? window.accent : window.line
    }
    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font.pixelSize: 14
        font.weight: control.primary || control.selected ? Font.DemiBold : Font.Normal
        color: control.primary ? window.onAccent : window.ink
        opacity: control.enabled ? 1 : .46
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
}
