use egui::Ui;
use jni::{
    Env, EnvUnowned, JNIEnv, jni_sig, jni_str,
    objects::{JClass, JObject, JString, JValue},
    refs::Global,
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

use crate::{App, settings::SETTINGS};

static BOOTSTRAP_ACTIVITY_CLASS: OnceLock<Global<JClass<'_>>> = OnceLock::new();
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

    fn pass_root_setting(&mut self, _app: &mut App) {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        let pass_root_hint = match settings.pass_root() {
            Some(pass_root) => format!("current folder is {pass_root}"),
            None => "pass root is not set".to_string(),
        };
        drop(settings);
        let is_clicked = self
            .button("pick a new folder")
            .highlight()
            .on_hover_text(pass_root_hint) // TODO: find out why this doesn't work
            .clicked();
        if is_clicked {
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

/// launch prepared file picker activity
/// relies on that load_file_picker_activity ran successfully beforehand
fn run_picker() -> jni::errors::Result<()> {
    jni_with_env(|env| -> jni::errors::Result<()> {
        let ctx =
            unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
        let class_ref = BOOTSTRAP_ACTIVITY_CLASS
            .get()
            .expect("unable to fetch class ref")
            .as_obj();

        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!((android.content.Context, java.lang.Class) -> ()),
            &[JValue::Object(&ctx), JValue::Object(class_ref)],
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
    jni_with_env(|env| -> jni::errors::Result<()> {
        let activity_class = load_bootstrap_class(env)?;
        BOOTSTRAP_ACTIVITY_CLASS
            .set(
                env.new_global_ref(activity_class)
                    .expect("cannot store global ref to the loaded activity"),
            )
            .expect("unable to save bootstrap class");
        Ok(())
    })?;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    DIR_PICKER_TX
        .set(tx)
        .expect("can't set directory picker pipe tx");
    DIR_PICKER_RX
        .set(Mutex::new(rx))
        .expect("can't set directory picker pipe rx");
    Ok(())
}

/// load the pre-compiled java.BootstrapActivity class by the default class loader
fn load_bootstrap_class<'a>(env: &mut Env<'a>) -> jni::errors::Result<JClass<'a>> {
    let ctx = unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };

    let loader = env
        .call_method(
            &ctx,
            jni_str!("getClassLoader"),
            jni_sig!(() -> java.lang.ClassLoader),
            &[],
        )?
        .l()?;

    let class_name = env.new_string("java.BootstrapActivity")?;
    let class = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!((java.lang.String) -> java.lang.Class),
            &[JValue::Object(&class_name)],
        )?
        .l()?;

    JClass::cast_local(env, class)
}

/// impls the respective Java function, see java/BootstrapActivity.java
#[unsafe(no_mangle)]
extern "C" fn Java_java_BootstrapActivity_nativeOnActivityResult(
    mut env: EnvUnowned,
    _this: JObject,
    _request_code: i32,
    result_code: i32,
    data: JObject,
) {
    DIR_PICKER_TX
        .get()
        .expect("dir picker channel is closed")
        .send((result_code == -1).then(|| {
            let uri = env.with_env(|env| {
                let uri_obj = env
                    .call_method(
                        &data,
                        jni_str!("getData"),
                        jni_sig!(() -> android.net.Uri),
                        &[],
                    )?
                    .l()?;
                let uri_str = env
                    .call_method(
                        &uri_obj,
                        jni_str!("toString"),
                        jni_sig!(() -> java.lang.String),
                        &[],
                    )?
                    .l()?;
                JString::cast_local(env, uri_str).map(|js| js.to_string())
            });
            // TODO: bubble-up errors / process correctly here
            uri.resolve::<jni::errors::LogErrorAndDefault>()
        }))
        .expect("unable to send picked path");
}
