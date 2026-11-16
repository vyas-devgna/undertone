import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

BarWidget {
    id: root
    moduleName: "local.undertone"
    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    Translations { id: strings; directory: Qt.resolvedUrl("i18n") }

    // Focus a running tuner instead of launching a second one that exits on the instance lock.
    function open() { Quickshell.execDetached(["omarchy-launch-or-focus", "undertone", "gtk-launch undertone"]) }

    ShellIpc {
        target: root.moduleName
        function open(): void { root.open() }
    }

    BarIconButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        text: "♪"
        tooltipText: strings.t("launch")
        onPressed: function(key) {
            if (key === Qt.LeftButton)
                root.open()
        }
    }
}
