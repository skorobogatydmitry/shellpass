//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::sync::{LazyLock, Mutex};

use egui::{InnerResponse, Layout, Popup, Response, ScrollArea, Ui};
use pgp::{composed::SignedSecretKey, types::KeyDetails};

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
mod linux;

pub(crate) trait OsUi {
    fn top_padding(&mut self);
    fn pass_root_setting(&mut self);
    /// represet platform-specific part of settings
    fn gnupg_secret_key_settings(&mut self) -> Response;
    fn load_secret_key(passphrase: Option<&str>) -> anyhow::Result<SignedSecretKey>;
}

use crate::{
    finder::FINDER,
    pass::{PassRepository, REPOSITORY},
    settings::SETTINGS,
};

static UI_STATE: LazyLock<Mutex<UiState>> = LazyLock::new(|| {
    Mutex::new(UiState {
        partial_gnupg_secret_key: String::new(),
        partial_gnupg_passphrase: String::new(),
    })
});

struct UiState {
    // that's a UI temporary data storage
    partial_gnupg_secret_key: String,
    partial_gnupg_passphrase: String,
}

/// menu with all the settings
fn settings_menu(button_resp: &Response) -> Option<InnerResponse<()>> {
    Popup::menu(button_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.pass_root_setting();
                    gnupg_settings(ui);
                });
            });
        })
}

/// part of the menu with GnuPG settings
fn gnupg_settings(ui: &mut Ui) {
    let mut settings = SETTINGS.lock().expect("settings are poisoned!");
    let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");

    let settings_key_digest = settings
        .gnupg_secret_key
        .as_ref()
        .map(|k| k.primary_key.fingerprint().to_string());

    ui.label(if settings.gnupg_passphrase_set() {
        "key passphrase is set"
    } else {
        "no passphrase set"
    });

    let passphrase_ui_buf = &mut ui_state.partial_gnupg_passphrase;
    let passphrase_edit = ui.add(
        egui::TextEdit::singleline(passphrase_ui_buf)
            .hint_text("passphrase for secret key")
            .password(true),
    );
    if passphrase_edit.lost_focus() {
        let mut pp = String::new();
        std::mem::swap(passphrase_ui_buf, &mut pp);
        settings.set_gnupg_passphrase(pp);
    }
    drop(ui_state);

    ui.label(match settings_key_digest {
        None => "no secret key loaded".to_string(),
        Some(settings_digest) => format!(
            "current key digest\n{}",
            settings_digest.to_ascii_uppercase()
        ),
    });

    let secret_key_resp = ui.gnupg_secret_key_settings();

    // try to initialize the key using digest and passphrase
    // digest in the settings can't be used here, as it can only be set if the previous load succeeded
    // so, even for passphrase change we rely on that the buffer has digest to load
    if secret_key_resp.lost_focus() || passphrase_edit.lost_focus() {
        match Ui::load_secret_key(settings.gnupg_passphrase()) {
            Ok(secret_key) => {
                settings.gnupg_secret_key = Some(secret_key);
            }
            Err(e) => {
                // TODO: show to the end-user
                log::error!("could not load secret key: {e}");
            }
        }
    }
}

pub(crate) fn main(ui: &mut Ui) {
    ui.set_zoom_factor(1.5);
    ui.vertical_centered_justified(|ui| {
        ui.top_padding();
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let image = egui::include_image!("../assets/cog.png");
                let settings_button_resp = ui.button(image);
                let settings_menu_resp = settings_menu(&settings_button_resp);
                ui.centered_and_justified(|ui| {
                    let mut finder = FINDER.lock().expect("finder is poisoned!");
                    let resp = ui.text_edit_singleline(&mut finder.pattern).highlight();
                    if resp.changed() {
                        finder.change_fence.notify_one();
                    }
                    drop(finder);
                    // keep focus on the main input unless there's a settings menu opened
                    // TODO: take gnupg settings into account
                    // if settings_menu_resp.is_none() {
                    //     resp.request_focus();
                    // }
                });
            });
        });

        let repository = REPOSITORY.lock().expect("repository is poisoned!");
        ui.label(match repository.entries_count() {
            0 => "no entries found, check settings".to_string(),
            count => format!("{} entries in your pass, start typing to search", count),
        });
    });

    let finder = FINDER.lock().expect("finder is poisoned!");
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        finder.last_match.iter().for_each(|entry| {
            // TODO: notification on click
            let entry_button = ui.selectable_label(false, entry.to_string());
            // TODO: avoid re-locking settings
            let settings = SETTINGS.lock().expect("settings are poinsoned!");
            let settings_has_gnupg_config = settings.has_gnupg_config();
            drop(settings);
            let gnupg_configuration_finished = if !settings_has_gnupg_config {
                let popup_resp = egui::Popup::from_toggle_button_response(&entry_button)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.vertical_centered_justified(|ui| {
                            gnupg_settings(ui);
                        });
                    });
                popup_resp
                    .map(|resp| resp.response.should_close())
                    .unwrap_or(false)
            } else {
                false
            };
            if entry_button.clicked() || gnupg_configuration_finished {
                let settings = SETTINGS.lock().expect("settings are poinsoned!");
                let gnupg_secret = settings.get_gnupg_secret();
                if let Some(gnupg_secret) = gnupg_secret {
                    let repository = REPOSITORY.lock().expect("repository is poisoned!");
                    match repository.retrieve(entry, gnupg_secret) {
                        Ok(data) => {
                            ui.copy_text(format!("{}:{}", data.0, data.1));
                        }
                        // TODO: notify the end-user
                        Err(e) => log::error!("unable to retrieve an entry: {e:?}"),
                    }
                }
            }
        });
    });
}
