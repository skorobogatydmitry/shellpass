//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::sync::{LazyLock, Mutex};

use egui::{InnerResponse, Layout, Popup, Response, ScrollArea, Ui};

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "linux")]
mod linux;

pub(crate) trait OsUi {
    fn top_padding(&mut self);
    fn pass_root_setting(&mut self);
    fn gnupg_settings(&mut self);
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

fn settings_menu(button_resp: &Response) -> Option<InnerResponse<()>> {
    Popup::menu(button_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.vertical_centered_justified(|ui| {
                ui.pass_root_setting();
            });
        })
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
                            ui.gnupg_settings();
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
