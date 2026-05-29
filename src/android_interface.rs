//! Various methods to simplify working with Java classes

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

static CLASSES: LazyLock<Mutex<HashMap<&'static str, Global<JClass>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn get_class<'a>(
    env: &mut Env<'a>,
    class_name: &'static str,
) -> jni::errors::Result<JClass<'a>> {
    let mut classes = CLASSES.lock().expect("classes cache is poisoned");
    if classes.contains_key(class_name) {
        let existing_class = classes.get(class_name).unwrap().as_raw();
        // UNSAFE: casting raw pointer obtained from a global ref just above this line
        return Ok(unsafe { JClass::from_raw(env, existing_class) });
    } else {
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

        let class_name_jstr = env.new_string(class_name)?;
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
        classes.insert(class_name, class);

        Ok(local_class)
    }
}
