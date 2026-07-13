//! Housekeeper - various periodic tasks

use std::{
    sync::{LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};

use log::info;

use crate::{
    Singleton,
    notifications::{self, Message},
    settings::Settings,
};

/// housekeeper's running period
static PERIOD: Duration = Duration::from_secs(30);
/// for how much time allow to store passphrase in memory
static PASSPHRASE_MAX_STORE_DURATION: Duration = Duration::from_mins(10);

pub struct Housekeeper {
    /// the moment in time passphrase was seen set
    passphrase_seen_set_at: Option<Instant>,
}

impl Singleton for Housekeeper {
    fn storage() -> &'static std::sync::LazyLock<std::sync::Mutex<Self>> {
        static HOUSEKEEPER: LazyLock<Mutex<Housekeeper>> =
            LazyLock::new(|| Mutex::new(Housekeeper::new()));
        &HOUSEKEEPER
    }
}

impl Housekeeper {
    pub fn initialize() {
        thread::spawn(move || {
            loop {
                info!("housekeeper wakes up");
                let mut housekeeper = Self::get();
                match housekeeper.passphrase_seen_set_at {
                    Some(passphrase_seen_set_at) => {
                        if (Instant::now() - passphrase_seen_set_at) > PASSPHRASE_MAX_STORE_DURATION
                        {
                            Settings::get().reset_passphrase();
                            housekeeper.passphrase_seen_set_at = None;
                            notifications::push_message(
                                Message::new(
                                    "passphrase is erased by housekeeper".to_string(),
                                    notifications::Kind::Success,
                                )
                                .with_duration(Duration::from_mins(2)),
                            );
                        }
                    }
                    None => {
                        if Settings::get().gnupg_passphrase_set() {
                            housekeeper.passphrase_seen_set_at = Some(Instant::now());
                            info!("passphrase is registered by housekeeper");
                        }
                    }
                }

                info!("housekeeper finished");
                thread::sleep(PERIOD);
            }
        });
    }

    fn new() -> Self {
        Housekeeper {
            passphrase_seen_set_at: None,
        }
    }
}
