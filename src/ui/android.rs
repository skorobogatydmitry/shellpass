use anyhow::{Context, anyhow};
use egui::Ui;
use jni::{
    EnvUnowned, jni_sig, jni_str,
    objects::{JObject, JString, JValue},
};
use jni_min_helper::jni_with_env;
use log::debug;
use ndk_context::android_context;
use pgp::composed::{Deserializable, SignedSecretKey};
use std::{
    sync::{
        Mutex, OnceLock,
        mpsc::{Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    android_interface::{ActivityClass, get_class, uri_path},
    settings::SETTINGS,
};

static DIR_PICKER_TX: OnceLock<SyncSender<Option<String>>> = OnceLock::new();
static DIR_PICKER_RX: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

static FILE_PICKER_TX: OnceLock<SyncSender<Option<String>>> = OnceLock::new();
static FILE_PICKER_RX: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

impl super::OsUi for Ui {
    /// there's an area in android screen which is actually occupied by status bar
    /// let's keep it clean
    fn top_padding(&mut self) {
        self.add_space(32.0);
    }
    /// some models have round bottom corners...
    fn bottom_padding(&mut self) {
        self.add_space(10.0);
    }

    fn pass_root_setting(&mut self) {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        let pass_root_hint = match settings.pass_root() {
            Some(pass_root) => format!(
                "current folder\n{}",
                uri_path(&pass_root).expect("unable to decode root's path")
            ),
            None => "pass root is not set".to_string(),
        };
        drop(settings);
        self.label(pass_root_hint);

        if self.button("pick a new folder").highlight().clicked() {
            // TODO: show error to the user
            run_activity(ActivityClass::DocTreePickerActivity).expect("can't fire dir picker");
            thread::spawn(|| {
                let new_pass_root = DIR_PICKER_RX
                    .get()
                    .and_then(|m| {
                        m.lock()
                            .expect("dir picker RX is poisoned!")
                            .recv_timeout(Duration::from_secs(90)) // let's assume that's enough for the users
                            .ok()
                    })
                    .flatten();
                debug!("new pass root from activity: {:?}", new_pass_root);
                if let Some(new_pass_root) = new_pass_root {
                    let mut settings = SETTINGS.lock().expect("settings are poisoned!");
                    settings.set_pass_root(new_pass_root);
                }
            });
        }
    }

    fn gnupg_secret_key_settings(&mut self) -> bool {
        let button = self.button("pick a new file").highlight();
        if button.clicked() {
            // TODO: show error to the user
            run_activity(ActivityClass::FilePickerActivity).expect("can't fire file picker");
        }
        // TODO: synchronize initialization and loading properly
        true
    }

    fn load_secret_key() -> JoinHandle<()> {
        thread::spawn(|| {
            match read_secret_key() {
                Ok(key) => {
                    let mut settings = SETTINGS.lock().expect("settings are poisoned!");
                    settings.gnupg_secret_key = Some(key);
                }
                Err(e) => {
                    // TODO: show to the user
                    log::error!("unable to read secret key file: {e:#}");
                }
            }
        })
    }
}

/// just obtain, read and parse the secret key
fn read_secret_key() -> anyhow::Result<SignedSecretKey> {
    let file_picker_rx = FILE_PICKER_RX
        .get()
        .ok_or(anyhow!("receiver for activity data is not ready"))?;
    let new_secret_key_uri = file_picker_rx
        .lock()
        .expect("file picker RX is poisoned!")
        .recv_timeout(Duration::from_secs(90)) // let's assume that's enough for the users
        .context("cannot receive the picked file URI")?;
    debug!("new secret key URI from activity: {:?}", new_secret_key_uri);
    let new_secret_key_uri =
        new_secret_key_uri.ok_or(anyhow!("no file uri received from picker"))?;

    let file_content = jni_min_helper::jni_with_env(|env| {
        let ctx =
            unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
        let jni_secret_key_uri = env.new_string(new_secret_key_uri)?;
        let fs_adapter = get_class(env, ActivityClass::FSAdapter)?;
        let bytes_jobj = env
            .call_static_method(
                &fs_adapter,
                jni_str!("readFile"),
                jni_sig!((android.content.Context, java.lang.String) -> [byte]),
                &[JValue::Object(&ctx), JValue::Object(&jni_secret_key_uri)],
            )?
            .l()?;
        let byte_array = unsafe { jni::objects::JByteArray::from_raw(env, bytes_jobj.as_raw()) };
        env.convert_byte_array(&byte_array)
    })
    .context("unable to read secret key file")?;

    SignedSecretKey::from_bytes(file_content.as_slice())
        .context("error on loading secret key bytes")
}

/// launch the prepared activity
fn run_activity(activity_class: ActivityClass) -> jni::errors::Result<()> {
    jni_with_env(|env| -> jni::errors::Result<()> {
        let ctx =
            unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
        let class_raw = get_class(env, activity_class)?.as_raw();
        // UNSAFE: cast the pointer obtained above
        let class_ref = unsafe { JObject::from_raw(env, class_raw) };

        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!((android.content.Context, java.lang.Class) -> ()),
            &[JValue::Object(&ctx), JValue::Object(&class_ref)],
        )?;

        // prevents `Calling startActivity() from outside of an Activity context requires the FLAG_ACTIVITY_NEW_TASK flag`
        env.call_method(
            &intent,
            jni_str!("addFlags"),
            jni_sig!((int) -> android.content.Intent),
            &[JValue::Int(0x10000000)],
        )?;

        env.call_method(
            &ctx,
            jni_str!("startActivity"),
            jni_sig!((android.content.Intent) -> ()),
            &[JValue::Object(&intent)],
        )?;

        Ok(())
    })
}

/// initiazile classes and variables to be able to launch file picker
pub fn load_file_picker_activity() -> jni::errors::Result<()> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    DIR_PICKER_TX
        .set(tx)
        .expect("can't set directory picker pipe tx");
    DIR_PICKER_RX
        .set(Mutex::new(rx))
        .expect("can't set directory picker pipe rx");
    // TODO: dedup
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    FILE_PICKER_TX
        .set(tx)
        .expect("can't set file picker pipe tx");
    FILE_PICKER_RX
        .set(Mutex::new(rx))
        .expect("can't set file picker pipe rx");
    Ok(())
}

/// impls the respective Java function, see java/DocTreePickerActivity.java
#[unsafe(no_mangle)]
extern "C" fn Java_java_DocTreePickerActivity_nativeOnActivityResult(
    mut env: EnvUnowned,
    _this: JObject,
    _request_code: i32,
    result_code: i32,
    uri: JObject,
) {
    DIR_PICKER_TX
        .get()
        .expect("dir picker channel is closed")
        .send((result_code == -1).then(|| {
            let uri = env.with_env(|env| JString::cast_local(env, uri).map(|js| js.to_string()));
            // TODO: bubble-up errors / process correctly here
            uri.resolve::<jni::errors::LogErrorAndDefault>()
        }))
        .expect("unable to send picked path");
}

#[unsafe(no_mangle)]
extern "C" fn Java_java_FilePickerActivity_nativeOnActivityResult(
    mut env: EnvUnowned,
    _this: JObject,
    _request_code: i32,
    result_code: i32,
    uri: JObject,
) {
    FILE_PICKER_TX
        .get()
        .expect("file picker channel is closed")
        .send((result_code == -1).then(|| {
            let uri = env.with_env(|env| JString::cast_local(env, uri).map(|js| js.to_string()));
            // TODO: bubble-up errors / process correctly here
            uri.resolve::<jni::errors::LogErrorAndDefault>()
        }))
        .expect("unable to send picked file");
}
