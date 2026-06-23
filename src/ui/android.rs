use egui::{Response, Ui};
use jni::{
    EnvUnowned, jni_sig, jni_str,
    objects::{JObject, JString, JValue},
};
use jni_min_helper::jni_with_env;
use log::debug;
use ndk_context::android_context;
use std::{
    sync::{
        Mutex, OnceLock,
        mpsc::{Receiver, SyncSender},
    },
    thread::{self},
    time::Duration,
};

use crate::{
    android_interface::{ActivityClass, get_class, uri_path},
    notifications::{self, Message},
    settings::{self, SETTINGS, SettingsUpdateReq},
};

static DIR_PICKER_TX: OnceLock<SyncSender<Option<String>>> = OnceLock::new();
pub static DIR_PICKER_RX: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

static FILE_PICKER_TX: OnceLock<SyncSender<Option<String>>> = OnceLock::new();
pub static FILE_PICKER_RX: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

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
            if let Err(e) = run_activity(ActivityClass::DocTreePickerActivity) {
                notifications::push_message(Message::new(
                    format!("cannot start directory picker: {e:#}"),
                    notifications::Kind::Error,
                ));
            }
            // TODO: move to settings ?
            thread::spawn(|| {
                let new_pass_root = DIR_PICKER_RX
                    .get()
                    .and_then(|m| {
                        m.lock()
                            .expect("dir picker RX is poisoned!")
                            .recv_timeout(Duration::from_secs(90)) // let's assume that's enough to pick a folder
                            .ok()
                    })
                    .flatten();
                debug!("new pass root from activity: {:?}", new_pass_root);
                if let Some(new_pass_root) = new_pass_root {
                    settings::send_update_request(SettingsUpdateReq::PassRoot(new_pass_root));
                }
            });
        }
    }

    fn gnupg_secret_key_settings(&mut self, _passphrase_setting: Response) -> bool {
        let button = self.button("pick a new file").highlight();
        if button.clicked() {
            match run_activity(ActivityClass::FilePickerActivity) {
                Ok(()) => true, // one activity - one request to process its results
                Err(e) => {
                    notifications::push_message(Message::new(
                        format!("cannot start file picker: {e:#}"),
                        notifications::Kind::Error,
                    ));
                    false
                }
            }
        } else {
            false
        }
    }

    fn to_clipboard(&self, s: String) {
        let result = jni_with_env(|env| -> jni::errors::Result<()> {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let service_name = env.new_string("clipboard")?;
            let clipboard_service = env
                .call_method(
                    ctx,
                    jni_str!("getSystemService"),
                    jni_sig!((java.lang.String) -> java.lang.Object),
                    &[JValue::Object(&service_name)],
                )?
                .l()?;

            let label = env.new_string("username:password from pass")?;
            let text_jstr = env.new_string(s.as_str())?;

            let clip_data = env.call_static_method(
                jni_str!("android/content/ClipData"),
                jni_str!("newPlainText"),
                jni_sig!((java.lang.CharSequence, java.lang.CharSequence) -> android.content.ClipData),
                &[JValue::Object(&label), JValue::Object(&text_jstr)],
            )?.l()?;

            let description = env
                .call_method(
                    &clip_data,
                    jni_str!("getDescription"),
                    jni_sig!(() -> android.content.ClipDescription),
                    &[],
                )?
                .l()?;

            let extras = env.new_object(
                jni_str!("android/os/PersistableBundle"),
                jni_sig!(() -> ()),
                &[],
            )?;

            let key = env.new_string("android.content.extra.IS_SENSITIVE")?;
            env.call_method(
                &extras,
                jni_str!("putBoolean"),
                jni_sig!((java.lang.String, boolean) -> ()),
                &[JValue::Object(&key), JValue::Bool(true)],
            )?;

            env.call_method(
                &description,
                jni_str!("setExtras"),
                jni_sig!((android.os.PersistableBundle) -> ()),
                &[JValue::Object(&extras)],
            )?;

            env.call_method(
                &clipboard_service,
                jni_str!("setPrimaryClip"),
                jni_sig!((android.content.ClipData) -> ()),
                &[JValue::Object(&clip_data)],
            )?;

            Ok(())
        });

        if let Err(e) = result {
            let masked_desc = format!("{e:#}").replace(s.as_str(), "****");
            crate::pass::clear_string(s);
            notifications::push_message(Message::new(masked_desc, notifications::Kind::Error));
        } else {
            crate::pass::clear_string(s);
        }
    }
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
pub fn init_picker_activities() -> jni::errors::Result<()> {
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
