FROM rust:1-alpine

ENV ANDROID_HOME="/opt/android"
ENV ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
ENV PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"

RUN rustup target add armv7-linux-androideabi aarch64-linux-android

RUN mkdir -p "${ANDROID_HOME}/cmdline-tools"
RUN apk add curl
RUN curl -sLo /tmp/clt.zip https://dl.google.com/android/repository/commandlinetools-linux-14742923_latest.zip
RUN unzip -d "${ANDROID_HOME}" /tmp/clt.zip

RUN apk add openjdk21

RUN yes | sdkmanager --licenses --sdk_root="${ANDROID_HOME}"
RUN sdkmanager --sdk_root="${ANDROID_HOME}" --install "build-tools;36.0.0" "ndk;29.0.14206865" "platforms;android-35"

RUN cargo install cargo-apk2

# android SDK tools don't run otherwise
RUN apk add gcompat bash

ENTRYPOINT [ "/bin/bash", "-c" "/usr/local/cargo/bin/cargo apk2 ${INPUT_APK2_ARGS:-build --lib}" ]
