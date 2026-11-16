import QtQuick
import QtQuick.Layouts

Item {
    id: meter
    property bool motion: true
    readonly property bool hasPitch: tuner.hz > 0 && !tuner.offTarget
    property color signal: tuner.inTune ? window.good : window.readout
    readonly property string displayedNote: tuner.note
    property real settled: tuner.inTune ? 1 : 0
    Behavior on signal {
        enabled: meter.motion
        ColorAnimation { duration: 140 }
    }
    Behavior on settled {
        enabled: meter.motion
        NumberAnimation { duration: 215; easing.type: Easing.OutCubic }
    }
    property real amount: hasPitch ? Math.max(-50, Math.min(50, tuner.cents)) : 0
    property real phase: 0
    // While a reading is held, the needle breathes so it reads as the last pluck, not a live one.
    property real pulse: 1
    SequentialAnimation on pulse {
        running: meter.motion && tuner.held
        loops: Animation.Infinite
        onStopped: meter.pulse = 1
        NumberAnimation { to: .35; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
    }

    Behavior on amount {
        enabled: meter.motion && meter.hasPitch
        NumberAnimation { duration: 85; easing.type: Easing.OutCubic }
    }

    onAmountChanged: scale.requestPaint()
    onHasPitchChanged: scale.requestPaint()
    onSignalChanged: scale.requestPaint()
    onPhaseChanged: scale.requestPaint()
    onPulseChanged: scale.requestPaint()
    onSettledChanged: scale.requestPaint()
    onDisplayedNoteChanged: if (meter.motion && displayedNote.length) noteEntrance.restart()

    Connections {
        target: window
        function onScreenLineChanged() { scale.requestPaint() }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Item { Layout.fillHeight: true; Layout.minimumHeight: 4 }
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: window.compact ? 112 : Math.min(238, meter.height * .50)

            Item {
                anchors.fill: parent
                opacity: tuner.held ? .55 : 1
                Behavior on opacity {
                    enabled: meter.motion
                    NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
                }
            Row {
                id: noteReadout
                anchors.centerIn: parent
                visible: tuner.hz > 0
                spacing: 5

                OpacityAnimator on opacity {
                    id: noteEntrance
                    running: false
                    from: 0.68
                    to: 1
                    duration: 145
                    easing.type: Easing.OutCubic
                }
                Text {
                    id: note
                    text: tuner.note.slice(0, -1)
                    textFormat: Text.PlainText
                    color: meter.hasPitch ? meter.signal : window.screenInk
                    font.family: monoFont
                    font.weight: Font.DemiBold
                    font.pixelSize: window.compact ? 92
                        : Math.round(Math.max(112, Math.min(202, meter.height * .34)))
                    font.letterSpacing: -4
                }
                Text {
                    text: tuner.note.slice(-1)
                    textFormat: Text.PlainText
                    color: meter.hasPitch ? meter.signal : window.screenInk
                    font.family: monoFont
                    font.pixelSize: window.compact ? 32 : 52
                    anchors.baseline: note.baseline
                }
            }
            }
            Column {
                anchors.centerIn: parent
                width: parent.width
                visible: tuner.hz <= 0
                spacing: 9
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: tuner.checking ? qsTr("Speaker check")
                          : tuner.playing ? qsTr("Reference tone")
                          : tuner.paused ? qsTr("Mic paused") : !tuner.listening ? qsTr("No microphone")
                          : tuner.target === -1 && window.stringsTuned === 6 ? qsTr("All six strings in tune")
                          : tuner.target >= 0 ? qsTr("Pluck string %1").arg(6 - tuner.target)
                          : tuner.target === -1 ? qsTr("Pluck any open string")
                          : qsTr("Pluck one string")
                    textFormat: Text.PlainText
                    color: window.screenInk
                    font.pixelSize: window.compact ? 26 : 40
                    font.weight: Font.Medium
                    font.letterSpacing: -.65
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: tuner.checking ? tuner.message
                        : tuner.playing ? qsTr("Listen, then match the pitch")
                        : tuner.paused ? qsTr("Resume the microphone to tune")
                        : !tuner.listening ? qsTr("Listening resumes when a microphone is available")
                        : tuner.target === -1 && window.stringsTuned === 6 ? qsTr("Pluck any open string to recheck it")
                        : tuner.target === -1 && tuner.soundFeedback ? qsTr("Rising cue → tighten   ·   Falling cue → loosen")
                        : qsTr("Let it ring before adjusting the peg")
                    textFormat: Text.PlainText
                    color: window.screenMuted
                    font.pixelSize: 15
                }
            }
        }
        Item { Layout.preferredHeight: window.compact ? 18 : 29 }

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.preferredHeight: window.compact ? 76 : 104

            Repeater {
                model: ["−50", "−25", "0", "+25", "+50"]
                Text {
                    required property int index
                    required property string modelData
                    x: Math.max(0, Math.min(parent.width - width, parent.width * index / 4 - width / 2))
                    text: modelData
                    textFormat: Text.PlainText
                    color: index === 2 && meter.hasPitch ? meter.signal : window.screenMuted
                    font.family: monoFont
                    font.pixelSize: 12
                }
            }

            Canvas {
                id: scale
                anchors.fill: parent
                anchors.topMargin: 21
                anchors.bottomMargin: 20
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    const ctx = getContext("2d")
                    ctx.reset()
                    const count = 45
                    const step = width / count
                    const center = (count - 1) / 2
                    const active = center + center * meter.amount / 50
                    for (let i = 0; i < count; ++i) {
                        const delta = i - center
                        const emphasized = i === 0 || i === count - 1 || delta % 11 === 0
                        const inRange = meter.hasPitch && i >= Math.min(center, active) && i <= Math.max(center, active)
                        const stripe = .48 + .32 * Math.cos(delta * .94 - meter.phase)
                        ctx.fillStyle = inRange ? window.alpha(meter.signal, Math.abs(delta) <= 1 ? .84 : stripe)
                                                : emphasized ? window.screenLineStrong : window.screenLine
                        const y = emphasized ? 2 : height * .19
                        if (meter.hasPitch && meter.settled > 0 && Math.abs(delta) <= 2) {
                            ctx.fillStyle = window.alpha(meter.signal, meter.settled * (delta === 0 ? .92 : .31))
                        }
                        ctx.fillRect(Math.round(i * step), Math.round(y), Math.max(2, Math.floor(step - 3)), Math.floor(height - y * 2))
                    }
                    if (meter.hasPitch) {
                        const x = Math.min(width - 3, (active + .5) * step - 1)
                        ctx.fillStyle = window.alpha(meter.signal, tuner.held ? meter.pulse : 1)
                        ctx.fillRect(Math.round(x), 0, 2, height)
                    }
                }
            }

            Text {
                anchors.bottom: parent.bottom
                text: qsTr("FLAT")
                color: window.screenMuted
                font.pixelSize: 11
                font.letterSpacing: .7
            }
            Text {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                text: qsTr("SHARP")
                color: window.screenMuted
                font.pixelSize: 11
                font.letterSpacing: .7
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredHeight: window.compact ? 29 : 39
            spacing: 10
            Text {
                visible: meter.hasPitch
                text: window.signed(tuner.cents, 1) + " ¢"
                textFormat: Text.PlainText
                color: meter.signal
                font.family: monoFont
                font.pixelSize: window.compact ? 15 : 19
            }
            Text {
                text: tuner.hz > 0 ? window.status
                      : tuner.message && !tuner.checking && !tuner.playing ? tuner.message : ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                Layout.maximumWidth: meter.width * (meter.hasPitch ? .73 : .98)
                color: tuner.inTune ? window.good : window.screenMuted
                font.pixelSize: window.compact ? 13 : 15
            }
        }
        Item { Layout.fillHeight: true; Layout.minimumHeight: 4 }
    }

    FrameAnimation {
        running: meter.visible && meter.motion && meter.hasPitch && !tuner.inTune && Math.abs(meter.amount) > .2
        onTriggered: meter.phase = (meter.phase + Math.min(.05, frameTime) * meter.amount * .54) % (2 * Math.PI)
    }
}
