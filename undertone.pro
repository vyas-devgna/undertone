QT += quick quickcontrols2 multimedia
CONFIG += c++17
TARGET = undertone
SOURCES += src/main.cpp src/tuner.cpp
HEADERS += src/tuner.h
RESOURCES += src/ui.qrc

RUST_LIB = rust/release/libundertone_core.a
INCLUDEPATH += $$quote($$OUT_PWD/rust/cxxbridge)
PRE_TARGETDEPS += $$RUST_LIB
LIBS += $$RUST_LIB -ldl -lpthread -lm
QMAKE_LFLAGS_RELEASE += -Wl,--gc-sections,--strip-debug
