#pragma once

#include "undertone-core/core/lib.rs.h"

#include <QAudioFormat>
#include <QBuffer>
#include <QElapsedTimer>
#include <QMediaDevices>
#include <QObject>
#include <QSettings>
#include <QTimer>
#include <QVariantList>
#include <memory>

class QAudioSource;
class QAudioSink;

class Tuner : public QObject {
    Q_OBJECT
    Q_PROPERTY(QStringList presets READ presets CONSTANT)
    Q_PROPERTY(int preset READ preset WRITE setPreset NOTIFY settingsChanged)
    Q_PROPERTY(int reference READ reference WRITE setReference NOTIFY settingsChanged)
    Q_PROPERTY(int target READ target WRITE setTarget NOTIFY settingsChanged)
    Q_PROPERTY(QVariantList strings READ strings NOTIFY settingsChanged)
    Q_PROPERTY(QStringList inputs READ inputs NOTIFY devicesChanged)
    Q_PROPERTY(int input READ input WRITE setInput NOTIFY devicesChanged)
    Q_PROPERTY(bool listening READ listening NOTIFY devicesChanged)
    Q_PROPERTY(bool paused READ paused NOTIFY devicesChanged)
    Q_PROPERTY(double hz READ hz NOTIFY readingChanged)
    Q_PROPERTY(double cents READ cents NOTIFY readingChanged)
    Q_PROPERTY(double level READ level NOTIFY readingChanged)
    Q_PROPERTY(QString note READ note NOTIFY readingChanged)
    Q_PROPERTY(int activeString READ activeString NOTIFY readingChanged)
    Q_PROPERTY(int tunedMask READ tunedMask NOTIFY readingChanged)
    Q_PROPERTY(bool inTune READ inTune NOTIFY readingChanged)
    Q_PROPERTY(bool offTarget READ offTarget NOTIFY readingChanged)
    Q_PROPERTY(bool held READ held NOTIFY readingChanged)
    Q_PROPERTY(bool soundFeedback READ soundFeedback WRITE setSoundFeedback NOTIFY soundFeedbackChanged)
    Q_PROPERTY(bool playing READ playing NOTIFY playbackChanged)
    Q_PROPERTY(bool checking READ checking NOTIFY playbackChanged)
    Q_PROPERTY(QString message READ message NOTIFY playbackChanged)

public:
    explicit Tuner(QObject *parent = nullptr);
    ~Tuner() override;
    QStringList presets() const;
    int preset() const { return m_view.preset; }
    int reference() const { return m_view.reference; }
    int target() const { return m_view.target; }
    QVariantList strings() const;
    QStringList inputs() const;
    int input() const;
    bool listening() const { return m_inputStream != nullptr; }
    bool paused() const { return m_paused; }
    double hz() const { return m_view.hz; }
    double cents() const { return m_view.cents; }
    double level() const { return m_view.level; }
    QString note() const;
    int activeString() const { return m_view.active_string; }
    int tunedMask() const { return m_view.tuned_mask; }
    bool inTune() const { return m_view.in_tune; }
    bool offTarget() const { return m_view.off_target; }
    bool held() const { return m_view.held; }
    bool soundFeedback() const { return m_soundFeedback; }
    bool playing() const { return m_currentCue == undertone::Cue::Reference || m_currentCue == undertone::Cue::Check; }
    bool checking() const { return m_view.check_index >= 0; }
    QString message() const { return m_message; }

    void setPreset(int value);
    void setReference(int value);
    void setTarget(int value);
    void setInput(int value);
    void setSoundFeedback(bool enabled);

    Q_INVOKABLE void toggleListening();
    Q_INVOKABLE void playReference();
    Q_INVOKABLE void checkAudio();
    Q_INVOKABLE void stopSound();
    Q_INVOKABLE void clearProgress();

signals:
    void settingsChanged();
    void devicesChanged();
    void readingChanged();
    void soundFeedbackChanged();
    void playbackChanged();
    void checkFinished(bool ok);

private:
    void openInput();
    void poll();
    void sync();
    void openOutput(const undertone::Playback &request);
    QString formatNotice(const undertone::View &view) const;

    QSettings m_settings;
    rust::Box<undertone::Engine> m_engine;
    undertone::View m_view;
    QMediaDevices m_devices;
    QTimer m_timer;
    QElapsedTimer m_clock;
    std::unique_ptr<QAudioSource> m_source;
    QIODevice *m_inputStream = nullptr;
    QByteArray m_inputId, m_openInputId, m_capture;
    bool m_paused = false;
    bool m_soundFeedback;
    qint64 m_nextInputAttempt = 0;
    QBuffer m_toneBuffer;
    std::unique_ptr<QAudioSink> m_output;
    undertone::Cue m_currentCue = undertone::Cue::Off;
    quint64 m_playbackSerial = 0;
    QString m_message;
};
