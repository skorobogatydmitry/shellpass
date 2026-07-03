use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use jni::{
    JValue, jni_sig, jni_str,
    objects::{JObject, JObjectArray, JString},
};
use ndk_context::android_context;

use crate::{
    android_interface::{ActivityClass, get_class, uri_path},
    notifications::{self, Message},
};

use super::PassEntry as _PassEntry;

impl super::RepositoryAccessor<PassEntry> for super::PassRepository<PassEntry> {
    fn entries_count(&self) -> usize {
        self.entries.len()
    }

    fn get_by_pattern(&self, pattern: &str) -> Vec<&PassEntry> {
        self.entries
            .iter()
            .filter(|e| e.contains(pattern))
            .collect()
    }

    fn fetch_entries_for(&mut self, pass_root: &str) {
        match jni_min_helper::jni_with_env(|env| {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let uri_jstr = env.new_string(pass_root)?;

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
                // TODO: catch and show all such errors to the user
                self.entries = gpg_files
                    .into_iter()
                    .map(|url| PassEntry::try_from((pass_root, url)).expect("unable to make entry"))
                    .collect();
                self.last_used_root = Some(pass_root.to_string());
                log::info!(
                    "{} entries for root {} found",
                    self.entries_count(),
                    pass_root
                );
            }
            Err(e) => {
                notifications::push_message(Message::new(
                    format!("unable to retrieve list of GPG files of {pass_root}: {e:#}"),
                    notifications::Kind::Error,
                ));
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PassEntry {
    // full path for storage API, e.g. content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass/document/primary%3ADocuments%2Fpass%2F1.gpg
    android_path: String,
    // user-visible path, e.g. dir1/dir2/some.gpg
    entry_relpath: PathBuf,
}

impl super::PassEntry for PassEntry {
    fn entry_relpath(&self) -> &PathBuf {
        &self.entry_relpath
    }

    fn read(&self) -> anyhow::Result<Vec<u8>> {
        jni_min_helper::jni_with_env(|env| {
            let ctx =
                unsafe { JObject::from_raw(env, android_context().context() as jni::sys::jobject) };
            let jni_secret_key_uri = env.new_string(self.android_path.as_str())?;
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

impl TryFrom<(&str, String)> for PassEntry {
    type Error = anyhow::Error;

    // content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass
    // content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fpass/document/primary%3ADocuments%2Fpass%2F1.gpg
    fn try_from(value: (&str, String)) -> Result<Self, Self::Error> {
        let (root, uri) = value;
        // Documents/pass
        let real_root_path = uri_path(root).context("cannot get entry's root path")?;
        // Documents/pass/some.gpg
        let entry_full_path = uri_path(&uri).context("cannot get entry's full path")?;
        // some.gpg
        let entry_relpath = &entry_full_path
            .strip_prefix(&real_root_path)
            .context("entry doesn't start from root's path")?[1..];

        Ok(Self {
            android_path: uri,
            entry_relpath: PathBuf::from_str(entry_relpath)
                .expect("entry relative path can't be interpreted"),
        })
    }
}
