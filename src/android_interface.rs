//! Various methods to simplify working with Java classes and data types

use anyhow::Context;
use jni::{
    Env, jni_sig, jni_str,
    objects::{JClass, JObject, JValue},
    refs::Global,
};
use ndk_context::android_context;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

static CLASSES: LazyLock<Mutex<HashMap<ActivityClass, Global<JClass>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// enum to store available activity classes
#[derive(PartialEq, Eq, Hash)]
pub(crate) enum ActivityClass {
    DocTreePickerActivity,
    FilePickerActivity,
    FSAdapter,
}

impl ActivityClass {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::DocTreePickerActivity => "java.DocTreePickerActivity",
            Self::FilePickerActivity => "java.FilePickerActivity",
            Self::FSAdapter => "java.FSAdapter",
        }
    }
}

pub(crate) fn get_class<'a>(
    env: &mut Env<'a>,
    activity_class: ActivityClass,
) -> jni::errors::Result<JClass<'a>> {
    let mut classes = CLASSES.lock().expect("classes cache is poisoned!");
    match classes.entry(activity_class) {
        std::collections::hash_map::Entry::Occupied(entry) => {
            let existing_class = entry.get().as_raw();
            // UNSAFE: casting raw pointer obtained from a global ref just above this line
            Ok(unsafe { JClass::from_raw(env, existing_class) })
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };

            let loader = env
                .call_method(
                    &ctx,
                    jni_str!("getClassLoader"),
                    jni_sig!(() -> java.lang.ClassLoader),
                    &[],
                )?
                .l()?;

            let class_name_jstr = env.new_string(entry.key().as_str())?;
            let class_jobj = env
                .call_method(
                    &loader,
                    jni_str!("loadClass"),
                    jni_sig!((java.lang.String) -> java.lang.Class),
                    &[JValue::Object(&class_name_jstr)],
                )?
                .l()?;

            let local_class = JClass::cast_local(env, class_jobj)?;
            let class = env.new_global_ref(&local_class)?;
            entry.insert(class);

            Ok(local_class)
        }
    }
}

// content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass -> Documents/pass
pub(crate) fn uri_path(uri: &str) -> anyhow::Result<String> {
    let decoded = urlencoding::decode(uri)?;
    decoded
        .split(":")
        .last()
        .context("error finding ':'")
        .map(|s| s.to_string())
}
