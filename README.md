# Simple client for GNU pass

The app is supposed to have a minimalistic interface with just a search bar and a list of matches.

You could copy a matched entry by clicking at it.

It's sketchy at the moment and works for Linux only.

## Linux

No gotchas: `cargo run`.

## Android

### Steps to prepare Android build environment & emulator

> First steps are from [this guide](https://github.com/skorobogatydmitry/egui/blob/sd/fill-android-pre-reqs/examples/hello_android/README.md#desktop-pre-requisites).

1. `rustup target add armv7-linux-androideabi aarch64-linux-android` - install targets for android
2. Set environment variables (these variables are required each time for `cargo apk2`):
  ```sh
  export ANDROID_HOME="$HOME/tools/android"
  export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
  export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
  ```
3. Install command line tools:
  ```sh
  mkdir -p "${ANDROID_HOME}/cmdline-tools"
  curl -sLo /tmp/clt.zip https://dl.google.com/android/repository/commandlinetools-linux-14742923_latest.zip
  unzip -d "${ANDROID_HOME}" /tmp/clt.zip
  ```
4. Install SDK components: `sdkmanager --sdk_root="${ANDROID_HOME}" --install "build-tools;36.0.0" "ndk;29.0.14206865" "platforms;android-35"`
5. Install cargo-apk: `cargo install cargo-apk2`
6. Install any JDK <= 21 using your OS package manager (e.g. `sudo pacman -S jdk21-openjdk ; sudo archlinux-java set java-21-openjdk`)

Now it's possible to build an apk:
  ```
  export ANDROID_HOME="$HOME/tools/android"
  export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
  export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
  cargo apk2 build --lib
  ```

7. Install components for emulator: `sdkmanager --sdk_root=${ANDROID_HOME} --install platform-tools emulator "system-images;android-35;google_apis;x86_64"`
8. Create AVD: `avdmanager create avd -n main -k "system-images;android-35;google_apis;x86_64"`
  > I had to fix `~/.android/avd/main.avd/config.ini` - remove android/ prefix from the `image.sysdir.1` option

The env to run app on android is ready.

### Start emulator and run app on it
1. Emulator:
```
export ANDROID_HOME="$HOME/tools/android"
export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
$ANDROID_HOME/emulator/emulator -avd main -no-snapshot-load
```
2. Build & run application:
```
export ANDROID_HOME="$HOME/tools/android"
export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
cargo apk2 run --lib
```

# TODO
- TODOs in the code
- Add user-visible notifications
- Make a button to close application
- Make interface scale factor user-defined
- Make a housekeeper to crash program if any thread crashes
- Persist settings, excluding password
- Show entries as a tree
- mention `gpg --export-secret-keys "AF0E12DF50A47F57522FDB5346B290E986B754D8" > my.key`
- make sure decrypted data is handled safely
