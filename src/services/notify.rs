//! Desktop notifications.
//!
//! The application talks to a [`Notifier`], never to a notification backend
//! directly. Today's implementation speaks the freedesktop specification over
//! D-Bus; an ODS-os-native backend only has to be written behind the same
//! trait. The UI knows nothing about D-Bus, and [`compose`] builds the
//! messages as plain data so their content can be tested without a session bus
//! — which is exactly the situation in a headless test run.
//!
//! Failures are never fatal. A machine with no notification daemon, or no
//! session bus at all, still gets the in-window presentation; only the system
//! notification is skipped, and the reason is reported.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::services::i18n::{Arg, Catalog, Text};

/// How insistent a notification is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    /// The freedesktop urgency name.
    pub fn as_str(self) -> &'static str {
        match self {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }
}

/// A button the user can press on the notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    /// The identifier reported back when the action is chosen.
    pub id: String,
    /// The label shown on the button.
    pub label: String,
}

impl Action {
    /// A new action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Action {
        Action {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A notification, as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    /// The bold first line.
    pub title: String,
    /// The body text.
    pub body: String,
    /// How insistent it is.
    pub urgency: Urgency,
    /// Whether the notification should stay until dismissed.
    pub persistent: bool,
    /// Whether the sound engine should also play [`crate::domain::sound::Sound`].
    pub play_sound: bool,
    /// Actions offered on the notification.
    pub actions: Vec<Action>,
}

impl Notification {
    /// A notification with just a title and a body.
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Notification {
        Notification {
            title: title.into(),
            body: body.into(),
            urgency: Urgency::Normal,
            persistent: false,
            play_sound: true,
            actions: Vec::new(),
        }
    }

    /// Marks the notification as insistent and staying on screen.
    pub fn critical(mut self) -> Notification {
        self.urgency = Urgency::Critical;
        self.persistent = true;
        self
    }

    /// Adds an action.
    pub fn with_action(mut self, id: impl Into<String>, label: impl Into<String>) -> Notification {
        self.actions.push(Action::new(id, label));
        self
    }

    /// Turns the sound off.
    pub fn silent(mut self) -> Notification {
        self.play_sound = false;
        self
    }
}

/// A choice the user made on a notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    /// The notification this answers.
    pub key: String,
    /// The action identifier, or `dismissed` when the notification went away
    /// without a choice.
    pub action: String,
}

impl Chosen {
    /// True when the user picked `id`.
    pub fn is(&self, id: &str) -> bool {
        self.action == id
    }
}

/// The action identifier used when a notification is dismissed rather than
/// acted on.
pub const DISMISSED: &str = "dismissed";

/// The action identifier of the "dismiss" button offered on a notification.
///
/// Distinct from [`DISMISSED`]: this is the id of the button the user pressed,
/// while `DISMISSED` is what a notification that was closed *without* a choice
/// reports. Both mean the user is done with the notification, so both dismiss.
pub const DISMISS: &str = "dismiss";

/// The identifier the notification daemon reports when a notification closes
/// without a choice.
const CLOSED: &str = "__closed";

/// Something that can tell the user something.
pub trait Notifier: Send {
    /// Shows a notification, tagged with `key` so its actions can be matched
    /// back to whatever raised it.
    fn notify(&self, key: &str, notification: &Notification) -> Result<(), NotifyError>;
}

/// The channel action choices travel on.
///
/// Kept beside the notifier rather than inside it: the application needs the
/// receiving end for the lifetime of the process, and a receiver cannot be
/// cloned, so it is created once and lent to whichever backend is in use.
pub struct Actions {
    sender: Sender<Chosen>,
    receiver: Receiver<Chosen>,
}

impl Default for Actions {
    fn default() -> Self {
        Self::new()
    }
}

impl Actions {
    /// Creates an empty channel.
    pub fn new() -> Actions {
        let (sender, receiver) = channel();
        Actions { sender, receiver }
    }

    /// A sender to hand to a backend.
    pub fn sender(&self) -> Sender<Chosen> {
        self.sender.clone()
    }

    /// The receiving end, which the application subscribes to once.
    pub fn receiver(&self) -> &Receiver<Chosen> {
        &self.receiver
    }

    /// Hands the receiving end over, leaving the sending end behind.
    ///
    /// A `Receiver` can be moved into a thread but not shared, so whoever will
    /// read the choices has to be given ownership rather than a borrow.
    pub fn take_receiver(&mut self) -> Receiver<Chosen> {
        std::mem::replace(&mut self.receiver, channel().1)
    }
}

/// Why a notification could not be shown.
#[derive(Debug)]
pub enum NotifyError {
    /// No session bus, or no notification daemon answered.
    Unavailable(String),
    /// The backend rejected the request.
    Rejected(String),
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotifyError::Unavailable(detail) => {
                write!(f, "no notification service is available: {detail}")
            }
            NotifyError::Rejected(detail) => write!(f, "the notification was rejected: {detail}"),
        }
    }
}

impl std::error::Error for NotifyError {}

/// Builds the notification for a ringing alarm.
///
/// Pure data in, pure data out, so the wording and the offered actions are
/// covered by tests on a machine with no notification daemon at all.
pub fn compose_alarm(
    alarm: &crate::domain::alarm::Alarm,
    snooze_minutes: u8,
    catalog: Catalog,
) -> Notification {
    let label = if alarm.label.trim().is_empty() {
        // The user's own name for the alarm, or the application's word for one
        // when they left it blank.
        catalog.text(Text::NotifyAlarmFallback).to_string()
    } else {
        alarm.label.clone()
    };
    let repeat = catalog.repeat(alarm.repeat);
    let time = catalog.alarm_time(alarm, crate::domain::clock::HourFormat::TwentyFour);

    Notification::new(
        &label,
        catalog.format(Text::NotifyAlarmBody, &[Arg::from(time), Arg::from(repeat)]),
    )
    .critical()
    .with_action(
        "snooze",
        catalog.format(Text::NotifySnoozeAction, &[Arg::from(snooze_minutes)]),
    )
    .with_action(DISMISS, catalog.text(Text::NotifyDismissAction))
}

/// Builds the notification for a finished timer.
pub fn compose_timer(label: &str, elapsed: std::time::Duration, catalog: Catalog) -> Notification {
    let name = if label.trim().is_empty() {
        catalog.text(Text::NotifyTimerFallback).to_string()
    } else {
        label.to_string()
    };
    Notification::new(
        &name,
        catalog.format(
            Text::NotifyFinishedAfter,
            &[Arg::from(catalog.duration(elapsed))],
        ),
    )
    .with_action(DISMISS, catalog.text(Text::NotifyDismissAction))
}

/// The freedesktop notification backend.
///
/// Created once and shared. Action callbacks arrive on a channel the
/// application subscribes to; the backend spawns one short-lived thread per
/// notification that has actions, which is the only way the specification's
/// callback model works.
pub struct DesktopNotifier {
    actions: Sender<Chosen>,
    app_name: String,
}

impl DesktopNotifier {
    /// Connects to the session notification service, reporting action choices
    /// on `actions`.
    pub fn connect(app_name: &str, actions: Sender<Chosen>) -> DesktopNotifier {
        DesktopNotifier {
            actions,
            app_name: app_name.to_string(),
        }
    }
}

impl Notifier for DesktopNotifier {
    fn notify(&self, key: &str, notification: &Notification) -> Result<(), NotifyError> {
        use notify_rust::{Notification as Desktop, Urgency as DesktopUrgency};

        let mut desktop = Desktop::new();
        desktop
            .appname(&self.app_name)
            .summary(&notification.title)
            .body(&notification.body)
            .urgency(match notification.urgency {
                Urgency::Low => DesktopUrgency::Low,
                Urgency::Normal => DesktopUrgency::Normal,
                Urgency::Critical => DesktopUrgency::Critical,
            })
            .timeout(if notification.persistent {
                // A ringing alarm must not vanish on its own.
                notify_rust::Timeout::Never
            } else {
                notify_rust::Timeout::Default
            });

        for action in &notification.actions {
            desktop.action(&action.id, &action.label);
        }

        let handle = desktop
            .show()
            .map_err(|error| NotifyError::Unavailable(error.to_string()))?;

        if notification.actions.is_empty() {
            return Ok(());
        }

        // Wait for a choice on a worker thread. The specification's callback
        // model blocks until the user acts or the notification is closed, and
        // the application should not be doing either.
        let sender = self.actions.clone();
        let key = key.to_string();

        std::thread::spawn(move || {
            let chosen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let sink = Arc::clone(&chosen);

            handle.wait_for_action(move |action: &str| {
                *sink.lock().expect("action sink") = Some(action.to_string());
            });

            // A closed notification, or no answer at all, is a dismissal: the
            // safest reading, because acting on it would be inventing intent.
            let action = chosen
                .lock()
                .expect("action sink")
                .take()
                .filter(|action| action != CLOSED && action != "default")
                .unwrap_or_else(|| DISMISSED.to_string());

            // The channel is unbounded, so this never blocks and never fails
            // in a way worth handling.
            let _ = sender.send(Chosen { key, action });
        });

        Ok(())
    }
}

/// A backend that shows nothing and reports why.
///
/// Used when there is no notification service, so the rest of the application
/// can carry on with a single code path.
pub struct SilentNotifier {
    reason: String,
}

impl SilentNotifier {
    /// Creates a notifier that reports `reason` for every request.
    pub fn new(reason: impl Into<String>) -> SilentNotifier {
        SilentNotifier {
            reason: reason.into(),
        }
    }
}

impl Notifier for SilentNotifier {
    fn notify(&self, _key: &str, _notification: &Notification) -> Result<(), NotifyError> {
        Err(NotifyError::Unavailable(self.reason.clone()))
    }
}

/// A notifier that records what it was asked to show, for tests.
pub struct RecordingNotifier {
    /// Everything that was shown, in order.
    pub shown: std::sync::Mutex<Vec<(String, Notification)>>,
    /// Whether to report a failure, and the message to give.
    pub failure: std::sync::Mutex<Option<String>>,
    actions: Sender<Chosen>,
}

impl RecordingNotifier {
    /// A notifier that always succeeds, using `actions` to report choices.
    pub fn new(actions: &Actions) -> RecordingNotifier {
        RecordingNotifier {
            shown: std::sync::Mutex::new(Vec::new()),
            failure: std::sync::Mutex::new(None),
            actions: actions.sender(),
        }
    }

    /// A notifier that always fails with `reason`.
    pub fn failing(reason: impl Into<String>) -> RecordingNotifier {
        RecordingNotifier {
            shown: std::sync::Mutex::new(Vec::new()),
            failure: std::sync::Mutex::new(Some(reason.into())),
            actions: Actions::new().sender(),
        }
    }

    /// Pretends the user chose `action` on the notification keyed `key`.
    pub fn choose(&self, key: &str, action: &str) {
        let _ = self.actions.send(Chosen {
            key: key.to_string(),
            action: action.to_string(),
        });
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, key: &str, notification: &Notification) -> Result<(), NotifyError> {
        if let Some(reason) = self.failure.lock().expect("lock").as_ref() {
            return Err(NotifyError::Unavailable(reason.clone()));
        }
        self.shown
            .lock()
            .expect("lock")
            .push((key.to_string(), notification.clone()));
        Ok(())
    }
}

/// Builds the best available notifier for this session.
///
/// The desktop backend is always chosen; it reports its own failures per
/// notification, because a daemon that is missing *now* may appear later.
///
/// There is no up-front check for a session with no bus at all, and there cannot
/// be one that is worth the name: `notify-rust` does not connect until it has
/// something to send, so the only thing a probe could establish is that sending
/// a message fails — which is what the per-notification error already says, and
/// says with the reason attached.
pub fn connect(app_name: &str, actions: &Actions) -> Box<dyn Notifier> {
    Box::new(DesktopNotifier::connect(app_name, actions.sender()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alarm::{Alarm, Repeat, Weekdays};
    use chrono::Weekday;
    use std::time::Duration;

    fn alarm() -> Alarm {
        Alarm {
            id: 1,
            label: "Gym".into(),
            hour: 6,
            minute: 30,
            enabled: true,
            repeat: Repeat::Weekly(Weekdays::from_days([Weekday::Mon, Weekday::Wed])),
            sound: crate::domain::sound::Sound::Chime,
            snooze_minutes: 9,
            last_fired: None,
            spent: false,
        }
    }

    #[test]
    fn an_alarm_notification_names_the_alarm_and_its_time() {
        let notification = compose_alarm(&alarm(), 9, Catalog::english());
        assert_eq!(notification.title, "Gym");
        assert!(notification.body.contains("06:30"), "{}", notification.body);
        assert!(
            notification.body.contains("Mon, Wed"),
            "{}",
            notification.body
        );
    }

    #[test]
    fn an_alarm_notification_is_critical_and_persistent() {
        let notification = compose_alarm(&alarm(), 9, Catalog::english());
        assert_eq!(notification.urgency, Urgency::Critical);
        assert!(notification.persistent, "a ringing alarm must not time out");
        assert!(notification.play_sound);
    }

    #[test]
    fn an_alarm_notification_offers_snooze_and_dismiss() {
        let notification = compose_alarm(&alarm(), 9, Catalog::english());
        let ids: Vec<&str> = notification.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["snooze", "dismiss"]);
        assert_eq!(notification.actions[0].label, "Snooze 9 min");
    }

    #[test]
    fn an_alarm_notification_uses_the_configured_snooze_length() {
        let notification = compose_alarm(&alarm(), 3, Catalog::english());
        assert_eq!(notification.actions[0].label, "Snooze 3 min");
    }

    #[test]
    fn an_unnamed_alarm_still_reads_well() {
        let mut unnamed = alarm();
        unnamed.label = "   ".into();
        let notification = compose_alarm(&unnamed, 9, Catalog::english());
        assert_eq!(notification.title, "Alarm");
    }

    #[test]
    fn a_timer_notification_reports_how_long_ran() {
        let notification = compose_timer("Tea", Duration::from_secs(305), Catalog::english());
        assert_eq!(notification.title, "Tea");
        assert_eq!(notification.body, "Finished after 5m 05s");
        assert_eq!(notification.urgency, Urgency::Normal);
        assert!(!notification.persistent);
    }

    #[test]
    fn an_unnamed_timer_still_reads_well() {
        let notification = compose_timer("  ", Duration::ZERO, Catalog::english());
        assert_eq!(notification.title, "Timer");
        assert_eq!(notification.body, "Finished after 0s");
    }

    #[test]
    fn urgency_names_match_the_specification() {
        assert_eq!(Urgency::Low.as_str(), "low");
        assert_eq!(Urgency::Normal.as_str(), "normal");
        assert_eq!(Urgency::Critical.as_str(), "critical");
    }

    #[test]
    fn the_recording_notifier_captures_what_it_was_asked_to_show() {
        let notifier = RecordingNotifier::new(&Actions::new());
        notifier
            .notify("alarm:1", &compose_alarm(&alarm(), 9, Catalog::english()))
            .expect("accepted");

        let shown = notifier.shown.lock().expect("lock");
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].0, "alarm:1");
        assert_eq!(shown[0].1.title, "Gym");
    }

    #[test]
    fn a_failing_notifier_reports_a_reason() {
        let notifier = RecordingNotifier::failing("no session bus");
        let error = notifier
            .notify("alarm:1", &compose_alarm(&alarm(), 9, Catalog::english()))
            .expect_err("should fail");
        assert!(error.to_string().contains("no session bus"));
        assert!(notifier.shown.lock().expect("lock").is_empty());
    }

    #[test]
    fn the_silent_notifier_explains_itself() {
        let notifier = SilentNotifier::new("no daemon");
        let error = notifier
            .notify("timer:1", &Notification::new("Done", "x"))
            .expect_err("should fail");
        assert!(matches!(error, NotifyError::Unavailable(_)));
        assert!(error.to_string().contains("no daemon"));
    }

    #[test]
    fn action_choices_arrive_on_the_shared_channel() {
        let actions = Actions::new();
        let notifier = RecordingNotifier::new(&actions);
        notifier.choose("alarm:1", "snooze");

        let chosen = actions.receiver().try_recv().expect("a choice");
        assert_eq!(chosen.key, "alarm:1");
        assert!(chosen.is("snooze"));
        assert!(!chosen.is(DISMISSED));
    }

    #[test]
    fn connecting_always_yields_a_usable_notifier() {
        // Headless test environments have no session bus; the application must
        // still start.
        let actions = Actions::new();
        let notifier = connect("OClock", &actions);
        let _ = notifier.notify("probe", &Notification::new("Probe", "checking"));
    }

    #[test]
    fn many_choices_do_not_block_the_producer() {
        // The worker thread that reports an action must never be able to wedge,
        // so the channel carries far more than the app will ever queue.
        let actions = Actions::new();
        let notifier = RecordingNotifier::new(&actions);
        for _ in 0..64 {
            notifier.choose("alarm:1", "dismiss");
        }
        let mut received = 0;
        while actions.receiver().try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 64);
    }
}
