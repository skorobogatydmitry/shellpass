# Simple client for GNU pass

The app is supposed to have a minimalistic interface with just a search bar and a list of matches.

You could copy a match by clicking on it.

It's sketchy at the moment and works for Linux only.

## Android build & run

1. Follow [this guide](https://github.com/skorobogatydmitry/egui/blob/sd/fill-android-pre-reqs/examples/hello_android/README.md#desktop-pre-requisites) to install pre-requisities.
2. Then you can build an apk:
  ```
  export ANDROID_HOME="$HOME/tools/android"
  export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
  export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
  cargo apk build
  ```
3. Running APK on emulator requires some more components: `sdkmanager --sdk_root=${ANDROID_HOME} --install platform-tools emulator "system-images;android-35;google_apis;x86_64"`
4. Create AVD: `avdmanager create avd -n main -k "system-images;android-35;google_apis;x86_64"`
  > I had to fix `~/.android/avd/main.avd/config.ini` - remove android/ prefix from the `image.sysdir.1` option
5. Start emulator: `$ANDROID_HOME/emulator/emulator -avd main -no-snapshot-load`
6. Run the app: `cargo apk run --lib`
  > ... from a different terminal, requires the same environment

### Routine run

Terminal 1:
```
export ANDROID_HOME="$HOME/tools/android"
export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
$ANDROID_HOME/emulator/emulator -avd main -no-snapshot-load
```

Terminal 2:
```
export ANDROID_HOME="$HOME/tools/android"
export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/29.0.14206865"
export PATH="$PATH:${ANDROID_NDK_ROOT}:${ANDROID_HOME}/platform-tools:${ANDROID_HOME}/build-tools/${BUILDTOOLS_VERSION}:${ANDROID_HOME}/cmdline-tools/bin"
cargo apk run --lib
```

# TODO
1. TODOs in the code
2. Accelerate startup by splitting app onto server and client parts
3. Add some sort of notifications
4. Make a button to close application
5. Make scale factor user-defined
