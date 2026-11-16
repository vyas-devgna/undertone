import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import QtCore

ApplicationWindow {
    id: window
    width: 1152
    height: 740
    minimumWidth: 420
    minimumHeight: 360
    visible: true
    visibility: Window.FullScreen
    title: qsTr("Undertone · Acoustic Guitar Tuner")
    Material.theme: Material.Dark
    Material.accent: accent
    Material.roundedScale: Material.NotRounded

    readonly property color base: palette.window
    readonly property color ink: palette.windowText
    readonly property bool lightTheme: luminance(base) > .45
    readonly property color muted: alphaMix(ink, base, lightTheme ? .12 : .21)
    readonly property color panel: alphaMix(base, ink, .04)
    readonly property color raised: alphaMix(base, ink, .10)
    readonly property color line: alphaMix(base, ink, .22)
    readonly property color accent: palette.accent
    readonly property color onAccent: contrastInk(accent)
    readonly property color screen: alphaMix(base, Qt.rgba(.082, .082, .086, 1), lightTheme ? .93 : .74)
    readonly property color screenInk: alphaMix(lightTheme ? base : ink, Qt.rgba(1, .969, .914, 1), .50)
    readonly property color screenMuted: alphaMix(screenInk, screen, .32)
    readonly property color screenLine: alphaMix(screenInk, screen, .84)
    readonly property color screenLineStrong: alphaMix(screenInk, screen, .56)
    readonly property color readout: alphaMix(accent, Qt.rgba(.937, .965, .875, 1), .27)
    readonly property color good: "#c3e8a6"
    readonly property bool compact: height < 680 || width < 870
    readonly property bool stacked: width < 740
    readonly property bool motion: preferences.animateStrobe
    readonly property int stringsTuned: tuner.strings.filter((item, i) => tuner.tunedMask & (1 << i)).length
    readonly property string status: {
        if (tuner.offTarget) return tuner.target >= 0 ? qsTr("Pluck the selected string") : qsTr("Pluck one open string")
        if (tuner.inTune) return qsTr("In tune")
        if (Math.abs(tuner.cents) <= 2) return qsTr("Let the note ring")
        return tuner.cents < 0 ? qsTr("Flat · tune up") : qsTr("Sharp · tune down")
    }
    color: base
    font.family: "Adwaita Sans"

    Settings {
        id: preferences
        category: "Display"
        property bool animateStrobe: true
    }

    function alpha(color, a) { return Qt.rgba(color.r, color.g, color.b, a) }
    function alphaMix(a, b, t) {
        return Qt.rgba(a.r * (1 - t) + b.r * t, a.g * (1 - t) + b.g * t, a.b * (1 - t) + b.b * t, 1)
    }
    function fixed(value, decimals) { return Number(value).toLocaleString(Qt.locale(), "f", decimals) }
    function signed(value, decimals) {
        return (value < -.05 ? "−" : value >= .05 ? "+" : "") + fixed(Math.abs(value), decimals)
    }
    function luminance(color) {
        function linear(v) { return v <= .04045 ? v / 12.92 : Math.pow((v + .055) / 1.055, 2.4) }
        return linear(color.r) * .2126 + linear(color.g) * .7152 + linear(color.b) * .0722
    }
    function contrastInk(color) { return luminance(color) > .183 ? "#121212" : "#ffffff" }

    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; onActivated: window.close() }
    Shortcut { sequence: "F11"; context: Qt.ApplicationShortcut; onActivated: window.visibility === Window.FullScreen ? window.showNormal() : window.showFullScreen() }
    Shortcut { sequence: "Space"; context: Qt.ApplicationShortcut; enabled: !settings.opened && !help.opened; onActivated: tuner.playReference() }
    Shortcut { sequence: "A"; context: Qt.ApplicationShortcut; enabled: !settings.opened && !help.opened; onActivated: tuner.target = -1 }
    Shortcut { sequence: "C"; context: Qt.ApplicationShortcut; enabled: !settings.opened && !help.opened; onActivated: tuner.target = -2 }
    Shortcut { sequence: "M"; context: Qt.ApplicationShortcut; enabled: !settings.opened && !help.opened; onActivated: tuner.toggleListening() }
    Shortcut { sequence: "?"; context: Qt.ApplicationShortcut; onActivated: help.opened ? help.close() : help.open() }
    Shortcut { sequence: "Escape"; context: Qt.ApplicationShortcut; enabled: !settings.opened && !help.opened; onActivated: tuner.playing ? tuner.stopSound() : window.showNormal() }
    Instantiator {
        model: 6
        delegate: Shortcut {
            required property int index
            sequence: String(6 - index)
            enabled: !settings.opened && !help.opened
            context: Qt.ApplicationShortcut
            onActivated: tuner.target = index
        }
    }

    Flickable {
        id: surface
        objectName: "tunerSurface"
        anchors.fill: parent
        readonly property int horizontalMargin: window.stacked ? 12 : window.compact ? 16 : 38
        readonly property int verticalMargin: window.compact ? 14 : 32
        clip: true
        interactive: contentHeight > height
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: width
        contentHeight: controls.height + 2 * verticalMargin
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        ColumnLayout {
            id: controls
            x: (surface.width - width) / 2
            y: surface.verticalMargin
            width: Math.min(surface.width - 2 * surface.horizontalMargin, 1420)
            height: Math.max(implicitHeight, surface.height - 2 * surface.verticalMargin)
            spacing: window.compact ? 12 : 23

            GridLayout {
                Layout.fillWidth: true
                columns: 6
                columnSpacing: window.compact ? 5 : 10
                rowSpacing: 8
                Text {
                    Layout.columnSpan: window.width < 480 ? 6 : 1
                    text: qsTr("Undertone")
                    textFormat: Text.PlainText
                    color: window.ink
                    font.pixelSize: window.compact ? 23 : 28
                    font.weight: Font.DemiBold
                    font.letterSpacing: -.7
                }
                Item { Layout.fillWidth: true; Layout.columnSpan: window.width < 480 ? 2 : 1 }
                ComboBox {
                    id: tuning
                    objectName: "tuningPreset"
                    Layout.preferredWidth: window.stacked ? 104 : window.compact ? 122 : 168
                    Layout.preferredHeight: 42
                    model: tuner.presets
                    currentIndex: tuner.preset
                    onActivated: index => tuner.preset = index
                    font.pixelSize: 14
                    Accessible.name: qsTr("Guitar tuning preset")
                    background: Rectangle {
                        color: tuning.hovered || tuning.down ? window.raised : "transparent"
                        border.color: tuning.visualFocus ? window.accent : window.line
                    }
                    indicator: Canvas {
                        width: 9
                        height: 7
                        x: tuning.width - width - 12
                        y: (tuning.height - height) / 2
                        onPaint: {
                            const ctx = getContext("2d")
                            ctx.reset()
                            ctx.strokeStyle = window.ink
                            ctx.lineWidth = 1.5
                            ctx.beginPath()
                            ctx.moveTo(1, 2)
                            ctx.lineTo(4.5, 5.5)
                            ctx.lineTo(8, 2)
                            ctx.stroke()
                        }
                        Connections { target: window; function onInkChanged() { tuning.indicator.requestPaint() } }
                    }
                    contentItem: Text {
                        leftPadding: 9
                        rightPadding: 5
                        text: tuning.displayText
                        textFormat: Text.PlainText
                        font: tuning.font
                        color: window.ink
                        elide: Text.ElideRight
                        verticalAlignment: Text.AlignVCenter
                    }
                }
                TunerButton {
                    objectName: "openTunerSettings"
                    text: qsTr("A4 %1 Hz").arg(tuner.reference)
                    onClicked: settings.open()
                    Accessible.name: qsTr("Settings and reference pitch")
                }
                TunerButton { text: "?"; quiet: true; Accessible.name: qsTr("Keyboard shortcuts"); onClicked: help.open() }
                TunerButton { text: "×"; quiet: true; Accessible.name: qsTr("Close tuner"); onClicked: window.close() }
            }

            GridLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                columns: window.stacked ? 1 : 2
                columnSpacing: window.compact ? 16 : 32
                rowSpacing: 16

                Rectangle {
                    id: face
                    objectName: "pitchFace"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: window.stacked ? 286 : 275
                    Layout.preferredHeight: window.compact ? 303 : 423
                    color: window.screen
                    border.width: 1
                    border.color: window.alpha(window.screenInk, .14)

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: window.compact ? 15 : 27
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 9
                            Rectangle {
                                Layout.preferredWidth: 7
                                Layout.preferredHeight: 7
                                radius: 4
                                color: tuner.listening ? window.accent : window.screenMuted
                            }
                            Text {
                                text: tuner.listening ? qsTr("Mic on") : tuner.paused ? qsTr("Mic paused") : qsTr("No mic")
                                color: window.screenMuted
                                font.pixelSize: 13
                            }
                            Row {
                                spacing: 2
                                Repeater {
                                    model: 8
                                    Rectangle {
                                        required property int index
                                        width: 4
                                        height: 11
                                        color: tuner.level * 8 > index + .6 ? window.readout : window.screenLine
                                        Behavior on color {
                                            enabled: window.motion
                                            ColorAnimation { duration: 100 }
                                        }
                                    }
                                }
                            }
                            Item { Layout.fillWidth: true }
                            Text {
                                text: tuner.hz > 0 ? qsTr("%1 Hz").arg(window.fixed(tuner.hz, 2)) : qsTr("— Hz")
                                color: window.screenMuted
                                font.family: monoFont
                                font.pixelSize: 13
                            }
                        }
                        Meter {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.leftMargin: window.compact ? 4 : 20
                            Layout.rightMargin: window.compact ? 4 : 20
                            motion: window.motion
                        }
                    }
                }

                ColumnLayout {
                    readonly property int railWidth: window.compact ? 205 : Math.min(298, window.width * .21)
                    objectName: "stringPanel"
                    Layout.fillWidth: window.stacked
                    Layout.minimumWidth: window.stacked ? 0 : railWidth
                    Layout.maximumWidth: window.stacked ? Infinity : railWidth
                    Layout.preferredWidth: railWidth
                    Layout.fillHeight: !window.stacked
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.bottomMargin: window.compact ? 6 : 12
                        Label {
                            text: qsTr("Open strings")
                            color: window.ink
                            font.pixelSize: window.compact ? 17 : 20
                            font.weight: Font.DemiBold
                        }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: qsTr("%1/6").arg(window.stringsTuned)
                            color: window.muted
                            font.family: monoFont
                            font.pixelSize: 13
                            Accessible.name: qsTr("%1 of six strings tuned").arg(window.stringsTuned)
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.bottomMargin: window.compact ? 7 : 13
                        spacing: 6
                        TunerButton {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.preferredHeight: window.compact ? 35 : 42
                            text: qsTr("Auto")
                            selected: tuner.target === -1
                            onClicked: tuner.target = -1
                        }
                        TunerButton {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.preferredHeight: window.compact ? 35 : 42
                            text: qsTr("Chromatic")
                            selected: tuner.target === -2
                            onClicked: tuner.target = -2
                        }
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: !window.stacked
                        columns: !window.stacked ? 1 : window.width < 520 ? 2 : 3
                        columnSpacing: 10
                        rowSpacing: 0
                        Repeater {
                            model: tuner.strings
                            delegate: Button {
                                id: stringRow
                                required property int index
                                required property var modelData
                                readonly property bool complete: (tuner.tunedMask & (1 << index)) !== 0
                                readonly property bool selected: tuner.target === index
                                readonly property bool active: tuner.activeString === index
                                objectName: "string" + (6 - index)
                                Layout.fillWidth: true
                                Layout.fillHeight: !window.stacked
                                Layout.preferredHeight: window.stacked ? 44 : window.compact ? 38 : 60
                                Layout.minimumHeight: 34
                                Layout.minimumWidth: 0
                                horizontalPadding: window.compact ? 8 : 11
                                verticalPadding: 0
                                hoverEnabled: true
                                Accessible.name: qsTr("String %1, %2").arg(6 - index).arg(modelData.note)
                                Accessible.description: complete ? qsTr("Tuned") : qsTr("Select this string")
                                onClicked: tuner.target = selected ? -1 : index
                                onCompleteChanged: if (complete && window.motion) confirmation.restart()

                                background: Rectangle {
                                    color: stringRow.active || stringRow.selected || stringRow.hovered ? window.raised : "transparent"
                                    border.color: stringRow.visualFocus || stringRow.selected ? window.accent : "transparent"
                                    Behavior on color {
                                        enabled: window.motion
                                        ColorAnimation { duration: 145 }
                                    }
                                    Rectangle {
                                        anchors.fill: parent
                                        color: window.accent
                                        opacity: 0
                                        NumberAnimation on opacity {
                                            id: confirmation
                                            running: false
                                            from: 0.20
                                            to: 0
                                            duration: 280
                                            easing.type: Easing.OutCubic
                                        }
                                    }
                                    Rectangle {
                                        width: parent.width
                                        height: 1
                                        anchors.bottom: parent.bottom
                                        color: window.line
                                    }
                                }
                                contentItem: RowLayout {
                                    spacing: window.compact ? 8 : 12
                                    Text {
                                        text: String(6 - index)
                                        color: window.muted
                                        font.family: monoFont
                                        font.pixelSize: 12
                                    }
                                    Text {
                                        text: modelData.note
                                        textFormat: Text.PlainText
                                        color: stringRow.active || stringRow.selected ? window.accent : window.ink
                                        font.pixelSize: window.compact ? 19 : 25
                                        font.weight: Font.DemiBold
                                    }
                                    Item { Layout.fillWidth: true }
                                    Text {
                                        text: qsTr("%1 Hz").arg(window.fixed(modelData.hz, 2))
                                        color: window.muted
                                        font.family: monoFont
                                        font.pixelSize: 12
                                    }
                                    Text {
                                        Layout.preferredWidth: 15
                                        text: stringRow.complete ? "✓" : stringRow.active ? "•" : ""
                                        color: stringRow.complete ? window.ink : window.accent
                                        font.pixelSize: 14
                                    }
                                }
                            }
                        }
                    }

                    TunerButton {
                        Layout.alignment: Qt.AlignRight
                        Layout.topMargin: window.compact ? 3 : 8
                        Layout.preferredHeight: window.compact ? 31 : 38
                        quiet: true
                        text: qsTr("Reset progress")
                        enabled: tuner.tunedMask > 0
                        onClicked: tuner.clearProgress()
                    }
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: window.stacked ? 3 : 5
                columnSpacing: window.compact ? 7 : 11
                rowSpacing: 7
                TunerButton {
                    Layout.fillWidth: window.stacked
                    text: tuner.playing ? qsTr("Stop sound") : qsTr("Play reference")
                    onClicked: tuner.playReference()
                    Accessible.description: qsTr("Space plays the active or selected pitch")
                }
                TunerButton {
                    Layout.fillWidth: window.stacked
                    text: tuner.checking ? qsTr("Cancel check") : qsTr("Speaker check")
                    onClicked: tuner.checkAudio()
                    Accessible.description: qsTr("Play three tones and test if the microphone hears them")
                }
                Item { Layout.fillWidth: true; visible: !window.stacked }
                Label {
                    visible: window.width > 1030
                    text: qsTr("1–6 Select  ·  Space Play")
                    color: window.muted
                    font.pixelSize: 13
                    Layout.rightMargin: 6
                }
                TunerButton {
                    Layout.fillWidth: window.stacked
                    text: tuner.listening ? qsTr("Pause mic") : tuner.paused ? qsTr("Resume mic") : qsTr("Retry mic")
                    onClicked: {
                        if (tuner.listening || tuner.paused) tuner.toggleListening()
                        else tuner.input = tuner.input
                    }
                }
            }
        }
    }

    Popup {
        id: settings
        parent: Overlay.overlay
        x: parent.width - width - (window.compact ? 19 : 34)
        y: Math.max(10, Math.min(window.compact ? 63 : 80, parent.height - height - 10))
        width: Math.min(parent.width - 38, 438)
        padding: 21
        modal: true
        dim: true
        focus: true
        background: Rectangle { color: window.panel; border.color: window.line }
        contentItem: ColumnLayout {
            spacing: window.compact ? 8 : 16
            RowLayout {
                Label { Layout.fillWidth: true; text: qsTr("Settings"); color: window.ink; font.pixelSize: 23 }
                TunerButton { text: "×"; Accessible.name: qsTr("Close settings"); onClicked: settings.close() }
            }
            RowLayout {
                Label { Layout.fillWidth: true; text: qsTr("Reference pitch · A4"); color: window.ink }
                SpinBox {
                    from: 415
                    to: 466
                    value: tuner.reference
                    onValueModified: tuner.reference = value
                    Accessible.name: qsTr("Reference pitch in hertz")
                }
                Label { text: qsTr("Hz"); color: window.muted }
            }
            Label { text: qsTr("Microphone input"); color: window.ink }
            ComboBox {
                Layout.fillWidth: true
                currentIndex: tuner.input
                model: tuner.inputs
                onActivated: index => tuner.input = index
                Accessible.name: qsTr("Microphone input")
                delegate: ItemDelegate {
                    required property int index
                    required property string modelData
                    width: ListView.view.width
                    highlighted: ListView.isCurrentItem
                    contentItem: Text {
                        text: modelData
                        textFormat: Text.PlainText
                        color: window.ink
                        elide: Text.ElideRight
                        verticalAlignment: Text.AlignVCenter
                    }
                }
            }
            Switch {
                objectName: "soundFeedbackSwitch"
                text: qsTr("Play tuning cues in Auto")
                checked: tuner.soundFeedback
                onToggled: tuner.soundFeedback = checked
            }
            Switch {
                objectName: "feedbackMotionSwitch"
                text: qsTr("Animate tuning feedback")
                checked: preferences.animateStrobe
                onToggled: preferences.animateStrobe = checked
            }
        }
    }

    Popup {
        id: help
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(parent.width - 38, 495)
        padding: 23
        modal: true
        dim: true
        focus: true
        background: Rectangle { color: window.panel; border.color: window.line }
        contentItem: ColumnLayout {
            spacing: 12
            RowLayout {
                Label { Layout.fillWidth: true; text: qsTr("Keyboard shortcuts"); color: window.ink; font.pixelSize: 23 }
                TunerButton { text: "×"; Accessible.name: qsTr("Close keyboard shortcuts"); onClicked: help.close() }
            }
            Repeater {
                model: [
                    ["1–6", qsTr("Select a string")], ["A", qsTr("Auto guitar")], ["C", qsTr("Chromatic")],
                    ["Space", qsTr("Play or stop the reference")], ["M", qsTr("Pause or resume the microphone")],
                    ["F11", qsTr("Toggle fullscreen")], ["Esc", qsTr("Stop sound or leave fullscreen")], ["Q", qsTr("Quit")]
                ]
                delegate: RowLayout {
                    required property var modelData
                    Caption { Layout.preferredWidth: 95; text: modelData[0]; color: window.accent }
                    Label { Layout.fillWidth: true; text: modelData[1]; color: window.ink }
                }
            }
        }
    }
}
