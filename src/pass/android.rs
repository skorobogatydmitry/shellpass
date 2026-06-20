use std::fmt::Display;

use anyhow::Context;
use jni::{
    JValue, jni_sig, jni_str,
    objects::{JObject, JObjectArray, JString},
};
use log::warn;
use ndk_context::android_context;

use crate::android_interface::{ActivityClass, get_class, uri_path};

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

    fn refresh_entries(&mut self, pass_root: &str) {
        warn!("root is {pass_root}");
        match jni_min_helper::jni_with_env(|env| {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let uri_jstr = env.new_string(pass_root)?;

            // Uri.parse(uriStr)
            let fs_adapter = get_class(env, ActivityClass::FSAdapter)?;
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
                    .map(|url| PassEntry::new(pass_root, url))
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
    root: String,
    suffix: String,
}

impl PassEntry {
    // content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass
    // content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass/document/primary%3ADocuments%2Fpass%2F1.gpg
    fn new(root: &str, uri: String) -> Self {
        // TODO: validate
        // UNWRAP: uri should start from the prefix
        Self {
            root: root.to_string(),
            suffix: uri.strip_prefix(root).unwrap().to_string(),
        }
    }

    /// makes user-digestable relative (from root) path within the repo
    fn entry_path(&self) -> String {
        // TODO: validate
        let real_root_path = uri_path(&self.root).expect("cannot get entry's root path");
        let entry_full_path = uri_path(&self.suffix).expect("cannot get entry's full path");
        entry_full_path
            .strip_prefix(&real_root_path)
            .expect("entry doesn't start from root's path")[1..]
            .to_string()
    }
}

impl super::PassEntry for PassEntry {
    /// search for match only within relative path
    fn contains(&self, pattern: &str) -> bool {
        self.entry_path().contains(pattern)
    }

    fn username(&self) -> String {
        // TODO: store the convention in common place
        self.entry_path()
            .split("/")
            .last()
            .unwrap_or("n/a")
            .to_string()
    }

    fn read(&self) -> anyhow::Result<Vec<u8>> {
        jni_min_helper::jni_with_env(|env| {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let jni_secret_key_uri =
                env.new_string(format!("{}{}", self.root, self.suffix).as_str())?;
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
        .context("unable to read entry file")
    }
}

impl Display for PassEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.entry_path()
                .strip_suffix(".gpg")
                .expect("cannot strip prefix from entry path")
        )
    }
}
