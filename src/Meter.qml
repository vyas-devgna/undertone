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
    // Strobe dial angle in degrees; 360 is a whole number of segments on every ring, so wrapping never jumps.
    property real turn: 0
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

    onDisplayedNoteChanged: if (meter.motion && displayedNote.length) noteEntrance.restart()

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Item { Layout.preferredHeight: window.compact ? 4 : 10 }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: window.compact ? 112 : 220

            // Strobe dial: rings spin toward sharp (clockwise) or flat, faster the further off,
            // and stand still in tune. Static segments; only three rotations change per frame.
            Item {
                id: dial
                anchors.centerIn: parent
                width: Math.min(parent.width, parent.height) * .98
                height: width
                opacity: !meter.hasPitch ? .14 : tuner.held ? .5 : .55 + .45 * tuner.level
                scale: 1 + .03 * meter.settled
                Behavior on opacity {
                    enabled: meter.motion
                    NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
                }
                Repeater {
                    model: [
                        { count: 72, radius: .5, depth: .06, speed: 1, alpha: 1 },
                        { count: 48, radius: .43, depth: .045, speed: .75, alpha: .62 },
                        { count: 36, radius: .375, depth: .032, speed: .5, alpha: .38 }
                    ]
                    Item {
                        id: ring
                        required property var modelData
                        anchors.fill: parent
                        rotation: meter.turn * modelData.speed
                        opacity: modelData.alpha
                        Repeater {
                            model: ring.modelData.count
                            Rectangle {
                                id: segment
                                required property int index
                                readonly property real orbit: dial.width * ring.modelData.radius
                                width: Math.max(2, 2.4 * orbit / ring.modelData.count)
                                height: dial.width * ring.modelData.depth
                                radius: width / 2
                                x: dial.width / 2 - width / 2
                                y: dial.width / 2 - orbit
                                color: meter.hasPitch ? meter.signal : window.screenLineStrong
                                transform: Rotation {
                                    origin.x: segment.width / 2
                                    origin.y: segment.orbit
                                    angle: segment.index * 360 / ring.modelData.count
                                }
                            }
                        }
                    }
                }
            }

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
                    font.pixelSize: Math.round(Math.min(window.compact ? 92 : 202, dial.width * .34))
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
            Layout.preferredHeight: window.compact ? 64 : 84

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

            // Scene-graph segments: only the strobing ones rebind per frame, nothing is rasterized in software.
            Item {
                id: strip
                anchors.fill: parent
                anchors.topMargin: 21
                anchors.bottomMargin: 20
                readonly property real step: width / 45
                readonly property real active: 22 + 22 * meter.amount / 50
                Repeater {
                    model: 45
                    Rectangle {
                        required property int index
                        readonly property int delta: index - 22
                        readonly property bool emphasized: index === 0 || index === 44 || delta % 11 === 0
                        readonly property bool inRange: meter.hasPitch
                            && index >= Math.min(22, strip.active) && index <= Math.max(22, strip.active)
                        readonly property real inset: emphasized ? 2 : strip.height * .19
                        x: Math.round(index * strip.step)
                        y: Math.round(inset)
                        width: Math.max(2, Math.floor(strip.step - 3))
                        height: Math.floor(strip.height - inset * 2)
                        color: meter.hasPitch && meter.settled > 0 && Math.abs(delta) <= 2
                            ? window.alpha(meter.signal, meter.settled * (delta === 0 ? .92 : .31))
                            : inRange ? window.alpha(meter.signal, Math.abs(delta) <= 1 ? .84 : .58)
                            : emphasized ? window.screenLineStrong : window.screenLine
                    }
                }
            }
            // The needle pulses on its own so held readings stay cheap.
            Rectangle {
                visible: meter.hasPitch
                y: strip.y
                width: 2
                height: strip.height
                x: Math.round(Math.min(strip.width - 3, (22 + 22 * meter.amount / 50 + .5) * strip.width / 45 - 1))
                color: meter.signal
                opacity: tuner.held ? meter.pulse : 1
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
        Item { Layout.preferredHeight: window.compact ? 4 : 10 }
    }

    // Spin the dial at ~60 updates a second; high-refresh displays gain nothing visible.
    Timer {
        interval: 16
        repeat: true
        running: meter.visible && meter.motion && meter.hasPitch && !tuner.inTune && Math.abs(meter.amount) > .2
        onTriggered: meter.turn = (meter.turn + .016 * meter.amount * 3.6) % 360
    }
}
