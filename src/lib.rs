use eframe::CreationContext;
use std::{
    error::Error,
    panic, process,
    sync::{LazyLock, Mutex, MutexGuard},
    thread,
    time::Duration,
};

#[cfg(target_os = "android")]
use crate::ui::init_picker_activities;
#[cfg(target_os = "android")]
pub(crate) mod android_interface;

use crate::{finder::Finder, notifications::Message, settings::Settings};

pub const NAME_VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

pub(crate) mod finder;
pub(crate) mod notifications;
pub(crate) mod pass;
pub(crate) mod settings;
pub(crate) mod ui;

pub struct App;

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(cc: &CreationContext) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        notifications::initialize();
        {
            Finder::search_routine();
        }

        Settings::initialize();

        cc.egui_ctx.set_zoom_factor(Settings::get().zoom_factor);
        Ok(Box::new(Self {}))
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::main(ui);
    }
}

// Android's entrypoint
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Log to android output
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    set_panic_handler();

    init_picker_activities().expect("error on initializing activities");

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    eframe::run_native(
        NAME_VERSION,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            App::new(cc)
        }),
    )
    .expect("cannot run application")
}

trait Singleton: Sized + 'static {
    fn storage() -> &'static LazyLock<Mutex<Self>>;

    fn get() -> MutexGuard<'static, Self> {
        Self::storage().lock().unwrap()
    }
}

/// sets panic handler to (1) exit the app if settigns or finder or any other non-main thread panics
/// and (2) send a UI notification unless it's the notification subsystem panicked
pub fn set_panic_handler() {
    let default_handler = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        default_handler(info);

        // it's not guaranteed that the mesaging or UI works, but it's better to try
        let notification_duration = Duration::from_secs(5);
        notifications::push_message(
            Message::new(
                info.payload_as_str()
                    .unwrap_or("application panicked for unknown reason")
                    .to_string(),
                notifications::Kind::Error,
            )
            .with_duration(notification_duration),
        );
        thread::sleep(notification_duration);

        process::exit(1);
    }));
}
