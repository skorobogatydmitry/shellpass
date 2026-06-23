//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::sync::{LazyLock, Mutex};

use egui::{CentralPanel, Color32, InnerResponse, Layout, Panel, Popup, Response, ScrollArea, Ui};

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "android")]
pub use android::FILE_PICKER_RX;
use egui_extras::{Size, StripBuilder};

#[cfg(target_os = "linux")]
mod linux;

pub(crate) trait OsUi {
    fn top_padding(&mut self);
    fn bottom_padding(&mut self);
    fn pass_root_setting(&mut self);
    /// represet platform-specific part of settings
    /// must return whether the setting is finalized (ready to read the key)
    fn gnupg_secret_key_settings(&mut self, passphrase_setting: Response) -> bool;
    /// send String to clipboard
    fn to_clipboard(&self, s: String);
}

use crate::{
    finder::FINDER,
    notifications::{self, Kind, Message},
    pass::{PassRepository, REPOSITORY},
    settings::{self, SETTINGS, SettingsUpdateReq},
};

static UI_STATE: LazyLock<Mutex<UiState>> = LazyLock::new(|| {
    Mutex::new(UiState {
        partial_gnupg_secret_key: String::new(),
        partial_gnupg_passphrase: String::new(),
    })
});

// UI temporary data storage
struct UiState {
    #[allow(dead_code)] // only for android
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
    let settings = SETTINGS.lock().expect("settings are poisoned!");
    let settings_key_digest = settings.gnupg_secret_key_digest();
    ui.label(if settings.gnupg_passphrase_set() {
        "key passphrase is set"
    } else {
        "no passphrase set"
    });
    drop(settings);

    let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
    let passphrase_ui_buf = &mut ui_state.partial_gnupg_passphrase;
    let passphrase_edit = ui.add(
        egui::TextEdit::singleline(passphrase_ui_buf)
            .hint_text("passphrase for secret key")
            .password(true),
    );
    if passphrase_edit.lost_focus() {
        let mut pp = String::new();
        std::mem::swap(passphrase_ui_buf, &mut pp);
        settings::send_update_request(SettingsUpdateReq::GnuPGPassphrase(pp));
    }
    drop(ui_state);

    ui.label(match settings_key_digest {
        None => "no secret key loaded".to_string(),
        Some(settings_digest) => format!(
            "current key digest\n{}",
            settings_digest.to_ascii_uppercase()
        ),
    });

    let secret_key_ready = ui.gnupg_secret_key_settings(passphrase_edit);

    // try to initialize the key using digest (and passphrase on Linux)
    // digest in the settings can't be used here, as it can only be set if the previous load succeeded
    // so, even for passphrase change we rely on that the buffer has digest to load
    if secret_key_ready {
        let ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let digest = ui_state.partial_gnupg_secret_key.clone(); //"AF0E12DF50A47F57522FDB5346B290E986B754D8"
        settings::send_update_request(SettingsUpdateReq::GnuPGSecretKey(digest));
    }
}

fn notifications_bar(ui: &mut Ui) {
    ui.horizontal(|ui| {
        let row_height = ui.spacing().interact_size.y; // standard widget height
        ui.spacing_mut().item_spacing.x = 0.0;

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(row_height)) // width == height -> square
            .horizontal(|mut strip| {
                let notification = notifications::current_notification();
                let show_close = notification.closable;
                strip.cell(|ui| {
                    ui.add(
                        egui::ProgressBar::new(notification.remained())
                            .animate(true)
                            .text(notification.message)
                            .fill(Color32::DARK_GRAY)
                            .corner_radius(1.5),
                    );
                });
                if show_close {
                    strip.cell(|ui| {
                        if ui
                            .add_sized(
                                [row_height, row_height],
                                egui::Button::new("✖").fill(egui::Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            notifications::expire_current();
                        }
                    });
                }
            });
    });
}

pub(crate) fn main(ui: &mut Ui) {
    ui.set_zoom_factor(1.5);
    Panel::top("notifications")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(3)))
        .show_inside(ui, |ui| {
            ui.top_padding();
            notifications_bar(ui);
        });

    Panel::bottom("search and settings")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(3)))
        .show_inside(ui, |ui| {
            // search bar + settings button
            let responses = ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let image = egui::include_image!("../assets/cog.png");
                    let settings_button_resp = ui.button(image);
                    let settings_menu_resp = settings_menu(&settings_button_resp);
                    ui.centered_and_justified(|ui| {
                        let mut finder = FINDER.lock().expect("finder is poisoned!");
                        let search_bar_resp = ui
                            .add(
                                egui::TextEdit::singleline(&mut finder.pattern)
                                    .hint_text("start typing to search"),
                            )
                            .highlight();
                        if search_bar_resp.changed() {
                            finder.change_fence.notify_one();
                        }
                        drop(finder);

                        // keep focus on the main input unless the settings menu is opened
                        if settings_menu_resp.is_none() {
                            search_bar_resp.request_focus();
                        }
                    })
                })
            });
            ui.bottom_padding();
            responses
        });

    // list of matching entries
    CentralPanel::no_frame().show_inside(ui, |ui| {
        let repository = REPOSITORY.lock().expect("repository is poisoned!");
        let entries_count = repository.entries_count();
        drop(repository);
        if entries_count > 0 {
            let finder = FINDER.lock().expect("finder is poisoned!");
            match finder.last_match.len() {
                0 => {
                    ui.label("no matching entries");
                }
                _ => {
                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        finder.last_match.iter().for_each(|entry| {
                            let entry_button = ui.selectable_label(false, entry.to_string());
                            // TODO: show popup with GnuPG settings if they're missing
                            if entry_button.clicked() {
                                let settings = SETTINGS.lock().expect("settings are poisoned!");
                                let gnupg_secret = settings.get_gnupg_secret();
                                match gnupg_secret {
                                    Some(gnupg_secret) => {
                                        let repository =
                                            REPOSITORY.lock().expect("repository is poisoned!");
                                        match repository.retrieve(entry, gnupg_secret) {
                                            Ok(data) => {
                                                ui.to_clipboard(format!("{}:{}", data.0, data.1));
                                                notifications::push_message(Message::new(
                                                    "copied".to_string(),
                                                    Kind::Success,
                                                ));
                                            }
                                            Err(e) => {
                                                notifications::push_message(Message::new(
                                                    format!("cannot copy: {e:#}"),
                                                    Kind::Error,
                                                ));
                                            }
                                        }
                                    }
                                    None => notifications::push_message(Message::new(
                                        "check settings: GnuPG is not fully configured".to_string(),
                                        Kind::Error,
                                    )),
                                }
                            }
                        });
                    });
                }
            }
        }
    });
}
