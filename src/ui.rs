//! UI-related salad of methods
//! No functionality expected, just egui-s ladders

use std::ops::DerefMut;

use egui::{InnerResponse, Layout, Popup, Response, ScrollArea, Ui};

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
mod linux;

trait OsUi {
    fn top_padding(&mut self);
    fn pass_root_setting(&mut self);
}

use crate::pass::{PassEntryImpl, PassRepository, REPOSITORY};

fn settings_menu(button_resp: &Response) -> Option<InnerResponse<()>> {
    Popup::menu(button_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.vertical_centered_justified(|ui| {
                ui.pass_root_setting();
            });
        })
}

pub(crate) fn main(app: &mut super::App<PassEntryImpl>, ui: &mut Ui) {
    ui.set_zoom_factor(1.5);
    ui.vertical_centered_justified(|ui| {
        ui.top_padding();
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let image = egui::include_image!("../assets/cog.png");
                let settings_button_resp = ui.button(image);
                let settings_menu_resp = settings_menu(&settings_button_resp);
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

        let repository = REPOSITORY.lock().expect("repository is poisoned!");
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
                let repository = REPOSITORY.lock().expect("repository is poisoned!");
                if let Ok(data) = repository.retrieve(entry) {
                    ui.copy_text(format!("{}:{}", data.0, data.1));
                } else {
                    todo!("show notification on error")
                }
            }
        });
    });
}
