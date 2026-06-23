use std::{
    fmt::Display,
    sync::{
        LazyLock, Mutex, OnceLock,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use crate::pass::{REPOSITORY, RepositoryAccessor};

/// sink for messages
static TX: OnceLock<Sender<Message>> = OnceLock::new();
static RX: OnceLock<Mutex<Receiver<Message>>> = OnceLock::new();
/// current non-default notification
static CURRENT_NOTIFICATION: LazyLock<Mutex<Option<Notification>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn initialize() {
    let (tx, rx) = mpsc::channel();
    RX.set(Mutex::new(rx))
        .expect("error on initializing notifications");
    TX.set(tx).expect("error on initializing notifications");
}

/// gets up-to-date current notification:
/// - replaces notifications when they expire
/// - polls the queue
/// - resets to default if the queue is empty
pub fn current_notification() -> Notification {
    let mut current_notification = CURRENT_NOTIFICATION
        .lock()
        .expect("current notification is poisoned!");
    // no notification or the current one expired
    if current_notification
        .as_ref()
        .map(|cn| cn.expired())
        .unwrap_or(true)
    {
        *current_notification = try_receive();
    }

    current_notification
        .as_ref()
        .map(Notification::clone)
        .unwrap_or_default()
}

/// displace current notification
pub fn expire_current() {
    let mut cn = CURRENT_NOTIFICATION
        .lock()
        .expect("current notification is poisoned!");
    cn.take();
}

/// publish a new message
pub fn push_message(message: Message) {
    // log all messages
    match message.kind {
        Kind::Error => log::error!("{}", message.message),
        Kind::Warning => log::warn!("{}", message.message),
        Kind::Success => log::debug!("{}", message.message),
    }
    let tx = TX
        .get()
        .expect("notifications delivery channel is not initialized");
    // can't do much here and it's not reasonable to crash => fallback to logs
    if let Err(e) = tx.send(message) {
        log::error!("unable to send user-visible message: {e:#}");
    }
}

fn try_receive() -> Option<Notification> {
    let rx = RX
        .get()
        .expect("notifications delivery channel is not initialized")
        .lock()
        .expect("notifications channel is poisoned!");
    rx.try_recv().ok().map(Notification::from)
}

/// mesasges the parts of the application can send
pub struct Message {
    message: String,
    duration: Duration,
    kind: Kind,
}

impl Message {
    pub fn new(message: String, kind: Kind) -> Self {
        Self {
            message,
            duration: Duration::from_secs(3),
            kind,
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

pub enum Kind {
    Success,
    Warning,
    Error,
}

impl Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Error => "❗",
                Self::Warning => "➡️",
                Self::Success => "√",
            }
        )
    }
}

/// represent a single notification to show to the user
#[derive(Clone)]
pub struct Notification {
    pub message: String,
    duration: Duration,
    started_at: Instant,
    pub closable: bool,
}
impl Notification {
    fn expired(&self) -> bool {
        self.closable && self.started_at.elapsed() >= self.duration
    }

    /// returns current progress in [0,1)
    pub fn remained(&self) -> f32 {
        if !self.closable {
            1.0 // tells UI not to re-draw
        } else if self.expired() {
            0.0 // avoid subtraction with overflow
        } else {
            (self.duration - self.started_at.elapsed()).as_millis() as f32
                / self.duration.as_millis() as f32
        }
    }
}

impl Default for Notification {
    fn default() -> Self {
        let repository = REPOSITORY.lock().expect("repository is poisoned!");
        let message = match repository.entries_count() {
            0 => "no entries found, check settings".to_string(),
            count => format!("{} entries in your pass", count),
        };

        Self {
            message,
            duration: Duration::from_hours(1), // effectively infinite
            started_at: Instant::now(),
            closable: false,
        }
    }
}

impl From<Message> for Notification {
    fn from(value: Message) -> Self {
        Self {
            message: format!("{} {}", value.kind, value.message),
            duration: value.duration,
            started_at: Instant::now(),
            closable: true, // all message-based notifications could be closed
        }
    }
}
