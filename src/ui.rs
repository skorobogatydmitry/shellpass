//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::ops::DerefMut;
use std::path::PathBuf;

use egui::{InnerResponse, Layout, Popup, Response, ScrollArea, Ui};

mod android;
mod linux;

trait OsUi {
    fn top_padding(ui: &mut Ui);
}

#[cfg(target_os = "android")]
use android::Ui as OsUiImpl;

#[cfg(target_os = "linux")]
use linux::Ui as OsUiImpl;

use crate::{App, settings::SETTINGS};

fn settings_menu(app: &mut App, button_resp: &Response) -> Option<InnerResponse<()>> {
    Popup::menu(button_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.vertical_centered_justified(|ui| {
                ui.label("pass repository root");
                let mut settings = SETTINGS.write().expect("settings are poisoned");
                let pass_root = settings.pass_root.get_or_insert(String::new());

                if ui.text_edit_singleline(pass_root).changed() {
                    settings.applied = false;
                }
                if ui.button("apply").highlight().clicked() {
                    // TODO: do in own routine
                    let mut repo = app.repository.write().expect("repository is poisoned!");
                    repo.refresh_entries(settings.pass_root.as_ref().map(PathBuf::from));
                }
            });
        })
}

pub(crate) fn main(app: &mut App, ui: &mut Ui) {
    ui.set_zoom_factor(1.5);
    ui.vertical_centered_justified(|ui| {
        OsUiImpl::top_padding(ui);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let image = egui::include_image!("../assets/cog.png");
                let settings_button_resp = ui.button(image);
                let settings_menu_resp = settings_menu(app, &settings_button_resp);
                ui.centered_and_justified(|ui| {
                    let mut pattern = app.pattern.lock().expect("pattern is poisoned!");
                    let resp = ui.text_edit_singleline(pattern.deref_mut()).highlight();
                    drop(pattern);
                    if resp.changed() {
                        app.pattern_change_fence.notify_one();
                    }
                    // keep focus on the main input unless there's a settings menu opened
                    if settings_menu_resp.is_none() {
                        resp.request_focus();
                    }
                });
            });
        });

        let repository = app.repository.read().expect("repository is poisoned!");
        ui.label(match repository.entries_count() {
            0 => "no entries found, check settings".to_string(),
            count => format!("{} entries in your pass, start typing to search", count),
        });
    });

    let last_match = app.last_match.read().expect("last match is poisoned!");
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        last_match.iter().for_each(|entry| {
            // TODO: notification on click
            if ui.selectable_label(false, entry.to_string()).clicked() {
                let repository = app.repository.read().expect("repository is poisoned!");
                if let Ok(data) = repository.retrieve(entry) {
                    ui.copy_text(format!("{}:{}", data.0, data.1));
                } else {
                    todo!("show notification on error")
                }
            }
        });
    });
}
