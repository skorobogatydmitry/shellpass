use egui::Ui;
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
    thread,
    time::Duration,
};

use crate::{
    android_interface::{get_class, uri_path},
    settings::SETTINGS,
};

static DIR_PICKER_TX: OnceLock<SyncSender<Option<String>>> = OnceLock::new();
static DIR_PICKER_RX: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

impl super::OsUi for Ui {
    /// there's an area in adnroid screen which is actually occupied by status bar
    /// let's keep it clean
    fn top_padding(&mut self) {
        egui::Panel::top("status_bar_space").show_inside(self, |ui| {
            ui.set_height(32.0);
        });
    }

    fn pass_root_setting(&mut self) {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        let pass_root_hint = match settings.pass_root() {
            Some(pass_root) => format!(
                "current folder is {}",
                uri_path(&pass_root).expect("unable to decode root's path")
            ),
            None => "pass root is not set".to_string(),
        };
        drop(settings);
        self.label(pass_root_hint);

        if self.button("pick a new folder").highlight().clicked() {
            // TODO: show error to the user
            run_picker().expect("can't fire dir picker");
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
}

/// launch the prepared file picker activity
/// relies on that load_file_picker_activity ran successfully beforehand
fn run_picker() -> jni::errors::Result<()> {
    jni_with_env(|env| -> jni::errors::Result<()> {
        let ctx =
            unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
        let class_raw = get_class(env, "java.DocTreePickerActivity")?.as_raw();
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
