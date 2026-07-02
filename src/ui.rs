//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::{
    sync::{LazyLock, Mutex},
    thread,
    time::Duration,
};

use egui::{CentralPanel, Color32, InnerResponse, Layout, Panel, Popup, Response, ScrollArea, Ui};
use egui_extras::{Size, StripBuilder};

use crate::{
    finder::FINDER,
    notifications::{self, Kind, Message},
    pass::{PassEntryImpl, REPOSITORY, RepositoryAccessor, clear_string},
    settings::{self, GnuPGSecretKeyProvider, SETTINGS, SettingsUpdateReq},
};

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::init_picker_activities;

#[cfg(target_os = "linux")]
mod linux;

/// OS-specific functionality of the UI
pub(crate) trait OsUi {
    fn top_padding(&mut self);
    fn bottom_padding(&mut self);
    fn pass_root_setting(&mut self);
    /// represet platform-specific part of settings
    /// returns a key provider if it's ready, None otherwise
    fn gnupg_secret_key_settings(
        &mut self,
        passphrase_update_issued: bool,
    ) -> Option<GnuPGSecretKeyProvider>;
    /// send String to clipboard
    fn to_clipboard(&self, s: String);
}

static UI_STATE: LazyLock<Mutex<UiState>> = LazyLock::new(|| {
    Mutex::new(UiState {
        partial_gnupg_secret_key: String::new(),
        partial_gnupg_passphrase: String::new(),
        partial_pass_root: String::new(),
    })
});

// UI temporary data storage
struct UiState {
    #[allow(dead_code)] // the buffer is only used on Linux
    partial_gnupg_secret_key: String,
    #[allow(dead_code)] // the buffer is only used on Linux
    partial_pass_root: String,
    partial_gnupg_passphrase: String,
}

/// menu with all the settings
fn settings_menu(button_resp: &Response) -> Option<InnerResponse<()>> {
    Popup::menu(button_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ScrollArea::vertical().auto_shrink(true).show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.pass_root_setting();
                    gnupg_settings(ui);
                    ui.add(egui::Separator::default());
                    if ui.button("reset settings").highlight().clicked() {
                        settings::send_update_request(SettingsUpdateReq::Reset);
                    }
                });
            });
        })
}

/// part of the menu with GnuPG settings
fn gnupg_settings(ui: &mut Ui) {
    let passphrase_update_issued = gnupg_passphrase_setting(ui);

    // secret key state
    {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        match settings.gnupg_secret_key_digest() {
            None => {
                ui.label("no secret key loaded");
            }
            Some(settings_digest) => {
                ui.label("current key digest");
                ui.label(settings_digest.to_ascii_uppercase());
            }
        }
    }

    let secret_key_provider = ui.gnupg_secret_key_settings(passphrase_update_issued);

    // try to initialize the key using digest (and passphrase on Linux)
    // digest from the settings can't be used, as it can only be set by a the previous update request
    // so, even for passphrase change we rely on that the buffer has a digest to load
    if let Some(secret_key_provider) = secret_key_provider {
        settings::send_update_request(SettingsUpdateReq::GnuPGSecretKey(secret_key_provider));
    }
}

/// passphrase status & edit field
/// returns whether an update request was issued
fn gnupg_passphrase_setting(ui: &mut Ui) -> bool {
    let settings = SETTINGS.lock().expect("settings are poisoned!");
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
        true
    } else {
        false
    }
}

fn notifications_bar(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        match notifications::current_notification() {
            Some(notification) => {
                let row_height = ui.spacing().interact_size.y;
                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(row_height)) // width == height -> square
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.add(
                                egui::ProgressBar::new(notification.remained())
                                    .animate(true)
                                    .text(notification.message)
                                    .fill(Color32::DARK_GRAY)
                                    .corner_radius(1.5),
                            );
                        });
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
                    });
            }
            None => {
                let repository = REPOSITORY.lock().expect("repository is poisoned!");
                ui.label(match repository.entries_count() {
                    0 => "no entries found, check settings".to_string(),
                    count => format!("{} entries in your pass", count),
                });
            }
        }
    });
}

pub(crate) fn main(ui: &mut Ui) {
    ui.set_zoom_factor(1.5);
    let (search_bar, settings_opened) = Panel::top("search and notifications")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(3)))
        .show_inside(ui, |ui| {
            ui.top_padding();
            // search bar + settings button
            let search_bar_and_settins = ui.horizontal(|ui| {
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

                        (search_bar_resp, settings_menu_resp.is_some())
                    })
                })
            });
            notifications_bar(ui);
            search_bar_and_settins.inner.inner.inner
        })
        .inner;

    // list of matching entries
    let mut passphrase_popup_present = false;
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
                            passphrase_popup_present =
                                passphrase_popup_present || retrieve_entry(ui, entry);
                        });
                    });
                }
            }
        }
        ui.bottom_padding();
    });

    if !passphrase_popup_present && !settings_opened {
        search_bar.request_focus();
    }
}

/// returns whether this retrieval process has passphrase prompt opened
fn retrieve_entry(ui: &mut Ui, entry: &PassEntryImpl) -> bool {
    let entry_button = ui.selectable_label(false, entry.to_string());
    let mut passphrase_popup_present = false;
    let passphrase_updated = {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        if !settings.gnupg_passphrase_set() {
            drop(settings);
            let passphrase_updated = Popup::menu(&entry_button)
                .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                .show(gnupg_passphrase_setting);
            passphrase_popup_present = passphrase_updated.is_some();
            let passphrase_updated = passphrase_updated.is_some_and(|r| r.inner);
            if passphrase_updated {
                // settings update may happen slower
                // TODO: support sync calls
                thread::sleep(Duration::from_millis(50));
            }
            passphrase_updated
        } else {
            false
        }
    };
    // 2 cases: everything is configured and the popup's edit lost the focus (the user pressed Enter or so)
    if entry_button.clicked() || passphrase_updated {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        let gnupg_secret = settings.get_gnupg_secret();
        match gnupg_secret {
            Some(gnupg_secret) => {
                let repository = REPOSITORY.lock().expect("repository is poisoned!");
                match repository.retrieve(entry, gnupg_secret) {
                    Ok(data) => {
                        let data = std::hint::black_box(data);
                        ui.to_clipboard(format!("{}:{}", data.0, data.1));
                        // UNSAFE: we drain the content just after the loop => no need to be valid seq
                        clear_string(data.1);
                        notifications::push_message(Message::new(
                            "copied".to_string(),
                            Kind::Success,
                        ));
                    }
                    Err(_e) => {
                        // the error can possibly contain sensitive data from the message or TheRing
                        notifications::push_message(Message::new(
                            "cannot retrieve the entry, check settings".to_string(),
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
    passphrase_popup_present
}
