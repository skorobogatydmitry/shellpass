use std::path::Path;

use jni::{
    JValue, jni_sig, jni_str,
    objects::{JObject, JObjectArray, JString},
};
use log::warn;
use ndk_context::android_context;

use crate::android_interface::get_class;

pub(crate) struct PassRepository {
    entries: Vec<PassEntry>,
    last_seen_pass: Option<String>,
}

impl super::PassRepository<PassEntry> for PassRepository {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            entries: Vec::new(),
            last_seen_pass: None,
        }
    }

    fn entries_count(&self) -> usize {
        self.entries.len()
    }

    fn get_by_pattern(&self, _pattern: &str) -> Vec<PassEntry> {
        Vec::new()
    }

    fn retrieve(&self, _entry: &PassEntry) -> anyhow::Result<(String, String)> {
        Ok(("dummy username".to_string(), "dummy password".to_string()))
    }

    fn refresh_entries(&mut self, pass_root: &str) {
        match jni_min_helper::jni_with_env(|env| {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let uri_jstr = env.new_string(pass_root)?;

            // Uri.parse(uriStr)
            let fs_adapter = get_class(env, "java/FSAdapter")?;
            let files_jobj = env
                .call_static_method(
                    &fs_adapter,
                    jni_str!("listFilesRecursive"),
                    jni_sig!((android.content.Context, java.lang.String) -> [java.lang.String]),
                    &[JValue::Object(&ctx), JValue::Object(&uri_jstr)],
                )?
                .l()?;
            let files_jarr =
                unsafe { JObjectArray::<'_, JObject>::from_raw(env, files_jobj.as_raw()) };
            let len = files_jarr.len(env)?;
            let mut gpg_files = Vec::with_capacity(len as usize);
            for i in 0..len {
                let file_uri_jobj: JObject = files_jarr.get_element(env, i)?;
                let file_uri = JString::cast_local(env, file_uri_jobj)?.to_string();
                if file_uri.ends_with(".gpg") {
                    gpg_files.push(file_uri);
                }
            }
            Ok(gpg_files)
        }) {
            Ok(gpg_files) => {
                self.entries = gpg_files
                    .into_iter()
                    .map(|file_uri| PassEntry::new(file_uri))
                    .collect();
                self.last_seen_pass = Some(pass_root.to_string());
                log::info!(
                    "{} entries for root {} found",
                    self.entries_count(),
                    pass_root
                );
            }
            // TODO: show to the end-user
            Err(e) => warn!("unable to retrieve list of GPG files of {pass_root}: {e}"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PassEntry {
    uri: String,
}

impl PassEntry {
    fn new(uri: String) -> Self {
        // TODO: validate
        Self { uri }
    }
}

impl super::PassEntry for PassEntry {
    fn contains(&self, pattern: &str) -> bool {
        todo!()
    }

    fn username(&self) -> String {
        todo!()
    }
}

impl ToString for PassEntry {
    fn to_string(&self) -> String {
        todo!()
    }
}

impl From<&Path> for PassEntry {
    fn from(value: &Path) -> Self {
        todo!()
    }
}
