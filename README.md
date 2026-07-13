# Simple read-only client for [pass](https://www.passwordstore.org/)

Graphical application for Android and Linux to retrieve entries from pass repository.

The idea for the app is to have a minimalistic interface with just a search bar and a list of matches.
Any entry could be copied to clipboard by clicking on it.

The app is designed to be read-only and needs to:
- be able to access folder with pass repository (e.g. `~/.password-store` on Linux)
- have your secret key imported to settings
- have your password for the key entered

Linux version imports your key from the local ring by its digest. The digest could be obtained with `gpg --list-secret-keys`.

Android version imports secret key as file, which should be exported by the following command and copied over to Android device: `gpg --export-secret-keys DIGEST > my.key`. There's no need to keep the key file, as it's stored in the settings.

The app persists settings by default, **excluding your password**, which should be entered on each start. The password is stored in memory for 10 minutes, so you can leave the application running.

## Development

### Linux

`cargo run` works just fine in case you have a working `pass`.

The only gotcha ATM is long decrytion time in debug mode caused by rpgp. It could take up to 10 seconds for an entry, so it's sometimes better to run the app in release mode.

## Android

Android support is set up with
- egui's native capabilities
- cargo-apk2 to build the project
- several Java files to allow picking folders, files and reading them 

Build environment requires some severe preparations.

### Steps to prepare Android build environment & emulator

> First steps are from [this guide](https://github.com/skorobogatydmitry/egui/blob/sd/fill-android-pre-reqs/examples/hello_android/README.md#desktop-pre-requisites).

1. Install targets for android - `rustup target add armv7-linux-androideabi aarch64-linux-android`
2. Set environment variables (have to be set for `cargo apk2`):
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
5. Install cargo-apk2: `cargo install cargo-apk2`
6. Install any JDK <= 21 using your OS package manager (e.g. `sudo pacman -S jdk21-openjdk ; sudo archlinux-java set java-21-openjdk`)

Now it's possible to build the apk: `cargo apk2 build --lib`

7. Install components for emulator: `sdkmanager --sdk_root=${ANDROID_HOME} --install platform-tools emulator "system-images;android-35;google_apis;x86_64"`
8. Create AVD: `avdmanager create avd -n main -k "system-images;android-35;google_apis;x86_64"`
  > I had to remove android/ prefix from the `image.sysdir.1` option from `~/.android/avd/main.avd/config.ini` manually.

The env to run app on Android is ready.

### Start the emulator and run the app
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

It's also possible to avoid emulator if you have a device with debugging enabled.

### Release & keystore

This app is not (and possibly won't ever be) Google Play -ready by any means, so password for Android release builds is in Cargo.toml to allow build apk's in release mode without frictions.

This command was used to generate the keystore:
```
keytool -genkeypair -v \
  -keystore release.keystore \
  -alias ShellPassKey \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000 \
  -storepass "@-:8e3zWE;z-zQmC._as\Ei" \
  -keypass "@-:8e3zWE;z-zQmC._as\Ei" \
  -dname "CN=LazyDeveloper,O=Personal,C=WE"
```

## Next

> Opportunities to contribute and enhance the app 

- TODOs in the code
- Make a safe PassEntry prefix strip
- Show pass entries as a tree
- Resize interface when keyboard appears/disappears on android
