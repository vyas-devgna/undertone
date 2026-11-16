#include "tuner.h"

#include <QAudioDevice>
#include <QAudioSink>
#include <QAudioSource>
#include <QLocale>
#include <QVariantMap>
#include <algorithm>

namespace {
constexpr int updateMs = 25;
constexpr int bufferUs = 120000;
constexpr int inputRetryMs = 1500;

QString noteName(int midi) {
    static const QStringList names {"C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "G♯", "A", "B♭", "B"};
    return names[midi % 12] + QString::number(midi / 12 - 1);
}

undertone::AudioLayout audioLayout(const QAudioFormat &format) {
    using undertone::SampleFormat;
    auto sample = SampleFormat::Invalid;
    switch (format.sampleFormat()) {
    case QAudioFormat::Float: sample = SampleFormat::Float; break;
    case QAudioFormat::Int32: sample = SampleFormat::Int32; break;
    case QAudioFormat::Int16: sample = SampleFormat::Int16; break;
    case QAudioFormat::UInt8: sample = SampleFormat::UInt8; break;
    default: break;
    }
    return {format.sampleRate(), format.channelCount(), sample};
}

QAudioFormat streamFormat(const QAudioDevice &device) {
    QAudioFormat mono;
    mono.setSampleRate(48000);
    mono.setChannelCount(1);
    mono.setSampleFormat(QAudioFormat::Float);
    return device.isFormatSupported(mono) ? mono : device.preferredFormat();
}
}

Tuner::Tuner(QObject *parent)
    : QObject(parent),
      m_engine(undertone::new_engine(m_settings.value("preset", 0).toInt(),
                                    m_settings.value("reference", 440).toInt())),
      m_view(m_engine->view()),
      m_inputId(m_settings.value("input").toByteArray()),
      m_soundFeedback(m_settings.value("soundFeedback", true).toBool()) {
    connect(&m_devices, &QMediaDevices::audioInputsChanged, this, &Tuner::openInput);
    connect(&m_timer, &QTimer::timeout, this, &Tuner::poll);
    m_clock.start();
    m_engine->set_feedback_enabled(m_soundFeedback);
    openInput();
    m_timer.start(updateMs);
}

Tuner::~Tuner() = default;

QStringList Tuner::presets() const {
    return {tr("Standard"), tr("Drop D"), tr("DADGAD"), tr("Open D"), tr("Open G"), tr("E♭ standard")};
}

QVariantList Tuner::strings() const {
    QVariantList list;
    for (std::size_t i = 0; i < 6; ++i) {
        list.append(QVariantMap {{"note", noteName(m_engine->string_midi(i))},
                                 {"hz", m_engine->string_hz(i)}});
    }
    return list;
}

QStringList Tuner::inputs() const {
    QStringList list {tr("System default")};
    for (const auto &device : QMediaDevices::audioInputs())
        list.append(device.description());
    return list;
}

int Tuner::input() const {
    const auto devices = QMediaDevices::audioInputs();
    for (int i = 0; i < devices.size(); ++i)
        if (devices[i].id() == m_inputId)
            return i + 1;
    return 0;
}

QString Tuner::note() const {
    return m_view.hz > 0 && m_view.note >= 0 ? noteName(m_view.note) : QString();
}

void Tuner::setPreset(int value) {
    const auto before = preset();
    m_engine->set_preset(value);
    sync();
    if (before != preset())
        m_settings.setValue("preset", preset());
}

void Tuner::setReference(int value) {
    const auto before = reference();
    m_engine->set_reference(value);
    sync();
    if (before != reference())
        m_settings.setValue("reference", reference());
}

void Tuner::setTarget(int value) {
    m_engine->set_target(value);
    sync();
}

void Tuner::setInput(int value) {
    const auto devices = QMediaDevices::audioInputs();
    if (value < 0 || value > devices.size())
        return;
    m_inputId = value == 0 ? QByteArray() : devices[value - 1].id();
    m_settings.setValue("input", m_inputId);
    openInput();
}

void Tuner::toggleListening() {
    m_paused = !m_paused;
    openInput();
}

void Tuner::openInput() {
    m_nextInputAttempt = m_clock.elapsed() + inputRetryMs;
    const auto devices = QMediaDevices::audioInputs();
    QAudioDevice selected;
    if (!m_paused) {
        selected = QMediaDevices::defaultAudioInput();
        for (const auto &device : devices)
            if (device.id() == m_inputId)
                selected = device;
    }
    if (selected.isNull() || !m_inputStream || selected.id() != m_openInputId) {
        const bool lost = !m_paused && m_inputStream && std::none_of(devices.begin(), devices.end(),
            [this](const QAudioDevice &device) { return device.id() == m_openInputId; });
        m_engine->drop_input(lost);
        m_inputStream = nullptr;
        m_source.reset();
        if (!selected.isNull()) {
            const auto format = streamFormat(selected);
            if (m_engine->configure_input(audioLayout(format))) {
                m_capture.resize(format.bytesForDuration(bufferUs));
                m_source = std::make_unique<QAudioSource>(selected, format);
                m_source->setBufferSize(m_capture.size());
                m_inputStream = m_source->start();
                m_openInputId = selected.id();
                if (!m_inputStream)
                    m_engine->drop_input(true);
            }
        }
    }
    sync();
    emit devicesChanged();
}

void Tuner::clearProgress() {
    m_engine->clear_progress();
    sync();
}

void Tuner::setSoundFeedback(bool enabled) {
    if (m_soundFeedback == enabled)
        return;
    m_soundFeedback = enabled;
    m_settings.setValue("soundFeedback", enabled);
    m_engine->set_feedback_enabled(enabled);
    sync();
    emit soundFeedbackChanged();
}

void Tuner::playReference() {
    m_engine->play_reference();
    sync();
}

void Tuner::checkAudio() {
    m_engine->check_audio();
    sync();
}

void Tuner::stopSound() {
    m_engine->stop_sound();
    sync();
}

void Tuner::openOutput(const undertone::Playback &request) {
    m_output.reset();
    m_toneBuffer.close();
    if (request.cue == undertone::Cue::Off)
        return;
    const auto device = QMediaDevices::defaultAudioOutput();
    if (device.isNull()) {
        m_engine->output_failed(undertone::Notice::OutputMissing);
        return;
    }
    const auto format = streamFormat(device);
    const auto samples = m_engine->render_audio(audioLayout(format));
    if (samples.empty()) {
        m_engine->output_failed(undertone::Notice::OutputUnsupported);
        return;
    }
    m_toneBuffer.setData(QByteArray(reinterpret_cast<const char *>(samples.data()), samples.size()));
    m_toneBuffer.open(QIODevice::ReadOnly);
    m_output = std::make_unique<QAudioSink>(device, format);
    m_output->start(&m_toneBuffer);
    m_engine->output_started(m_clock.elapsed());
}

QString Tuner::formatNotice(const undertone::View &view) const {
    using undertone::Notice;
    if (view.check_index >= 0)
        return tr("Speaker check %1 of %2 · %3").arg(view.check_index + 1).arg(3)
            .arg(noteName(undertone::check_midi(view.check_index)));
    switch (view.notice) {
    case Notice::Clear: return {};
    case Notice::CheckNoInput: return tr("Turn on a microphone before running the speaker check.");
    case Notice::CheckInputLost: return tr("Microphone disconnected during the speaker check.");
    case Notice::OutputMissing: return tr("No speaker output is available.");
    case Notice::OutputUnsupported: return tr("The speaker audio format is unsupported.");
    case Notice::OutputFailed: return tr("Could not play through the system speaker.");
    case Notice::OutputTimeout: return tr("Speaker playback timed out.");
    case Notice::CheckPassed:
        return tr("Audio check passed · max %1¢ error").arg(QLocale().toString(view.max_error, 'f', 1));
    case Notice::CheckPitchMismatch:
        return tr("Pitch check outside tolerance · max %1¢ error").arg(QLocale().toString(view.max_error, 'f', 1));
    case Notice::CheckMissing: {
        QStringList missing;
        for (int i = 0; i < 3; ++i)
            if (view.missing_mask & (1 << i))
                missing.append(noteName(undertone::check_midi(i)));
        return tr("Didn’t hear %1 clearly. Check speaker volume and microphone.").arg(missing.join(", "));
    }
    }
    return {};
}

void Tuner::sync() {
    const auto old = m_view;
    const bool wasPlaying = playing();
    const auto request = m_engine->playback();
    if (m_playbackSerial != request.serial) {
        openOutput(request);
        const auto actual = m_engine->playback();
        m_playbackSerial = actual.serial;
        m_currentCue = actual.cue;
    }
    m_view = m_engine->view();
    // Keep localization and Qt binding notifications at the UI boundary.
    if (old.preset != m_view.preset || old.reference != m_view.reference ||
        old.target != m_view.target)
        emit settingsChanged();
    if (old.hz != m_view.hz || old.cents != m_view.cents || old.level != m_view.level ||
        old.note != m_view.note || old.active_string != m_view.active_string ||
        old.tuned_mask != m_view.tuned_mask || old.in_tune != m_view.in_tune ||
        old.off_target != m_view.off_target || old.held != m_view.held)
        emit readingChanged();
    if (old.notice != m_view.notice || old.check_index != m_view.check_index ||
        old.check_serial != m_view.check_serial || wasPlaying != playing()) {
        m_message = formatNotice(m_view);
        emit playbackChanged();
    }
    if (old.check_serial != m_view.check_serial)
        emit checkFinished(m_view.notice == undertone::Notice::CheckPassed);
}

void Tuner::poll() {
    const auto now = m_clock.elapsed();
    if (m_inputStream && m_source->state() == QtAudio::StoppedState) {
        m_inputStream = nullptr;
        m_source.reset();
        m_engine->drop_input(true);
        m_nextInputAttempt = now + inputRetryMs;
        emit devicesChanged();
    }
    if (!m_inputStream && !m_paused && now >= m_nextInputAttempt) {
        m_nextInputAttempt = now + inputRetryMs;
        if (!QMediaDevices::defaultAudioInput().isNull())
            openInput();
    }
    qsizetype captured = 0;
    if (m_inputStream)
        captured = qMax<qsizetype>(0, m_inputStream->read(m_capture.data(), m_capture.size()));
    auto output = undertone::OutputState::Active;
    if (m_output) {
        if (m_output->state() == QtAudio::StoppedState && m_output->error() != QtAudio::NoError)
            output = undertone::OutputState::Failed;
        else if (m_toneBuffer.atEnd() && m_output->state() == QtAudio::IdleState)
            output = undertone::OutputState::Drained;
    }
    m_engine->tick(now, {reinterpret_cast<const quint8 *>(m_capture.constData()),
                                     static_cast<std::size_t>(captured)}, output);
    sync();
}
