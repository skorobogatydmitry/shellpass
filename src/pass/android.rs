use std::{fmt::Display, path::Path};

use jni::{
    JValue, jni_sig, jni_str,
    objects::{JObject, JObjectArray, JString},
};
use log::warn;
use ndk_context::android_context;
use url::Url;

use crate::android_interface::get_class;

use super::PassEntry as _PassEntry;

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

    fn get_by_pattern(&self, pattern: &str) -> Vec<&PassEntry> {
        self.entries
            .iter()
            .filter(|e| e.contains(pattern))
            .collect()
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
                    .filter_map(|uri_str| match Url::parse(uri_str.as_str()) {
                        Ok(url) => Some(url),
                        Err(e) => {
                            // TODO: show to the end-user
                            warn!("unable to parse file URI as URL: {e}");
                            None
                        }
                    })
                    .map(|url| PassEntry::new(url))
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
    url: Url,
}

impl PassEntry {
    fn new(url: Url) -> Self {
        // TODO: validate
        Self { url }
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

impl From<&Path> for PassEntry {
    fn from(value: &Path) -> Self {
        todo!()
    }
}

impl Display for PassEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.url
                .path_segments()
                .unwrap()
                .last()
                .map(|full_path| full_path)
                .unwrap()
        )
    }
}
