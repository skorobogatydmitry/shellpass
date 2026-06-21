use std::time::Duration;

use anyhow::{Context, anyhow};
use jni::{JValue, jni_sig, jni_str, objects::JObject};
use ndk_context::android_context;
use pgp::composed::{Deserializable, SignedSecretKey};

use crate::{
    android_interface::{ActivityClass, get_class},
    ui,
};

impl super::OsSettings for super::Settings {
    fn read_secret_key(_unused_digest: String) -> anyhow::Result<pgp::composed::SignedSecretKey> {
        // expect UI to populate the RX
        let file_picker_rx = ui::FILE_PICKER_RX
            .get()
            .ok_or(anyhow!("receiver for activity data is not ready"))?;
        let new_secret_key_uri = file_picker_rx
            .lock()
            .expect("file picker RX is poisoned!")
            .recv_timeout(Duration::from_secs(90)) // let's assume that's enough for the users
            .context("cannot receive the picked file URI")?;
        log::debug!("new secret key URI from activity: {:?}", new_secret_key_uri);
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
            let byte_array =
                unsafe { jni::objects::JByteArray::from_raw(env, bytes_jobj.as_raw()) };
            env.convert_byte_array(&byte_array)
        })
        .context("unable to read secret key file")?;

        SignedSecretKey::from_bytes(file_content.as_slice())
            .context("error on loading secret key bytes")
    }
}
