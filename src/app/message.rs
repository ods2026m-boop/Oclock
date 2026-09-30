//! Everything the application can be asked to do.
//!
//! One enum, so the whole surface of the application is readable in one place
//! and a new feature cannot smuggle in a second, parallel way of talking to the
//! UI. The variants are grouped by the thing they change, not by the widget
//! that raised them.

use crate::core::time::Discontinuity;
use crate::design::theme::ThemeMode;
use crate::domain::alarm::Repeat;
use crate::domain::clock::HourFormat;
use crate::domain::sound::Sound;
use crate::domain::world::SortMode;
use crate::services::i18n::Language;
use crate::ui::components::navigation::Destination;

/// A message the application handles.
///
/// `PartialEq` so a test can say which message a control sends and be believed:
/// the alternative is comparing `describe` output, which drops the payload and
/// would call pinning city 3 and pinning city 4 the same message.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    // --- the clock, and time passing ---
    /// The periodic tick. Carries nothing: the delta is measured against the
    /// monotonic clock when the message is handled, so a late tick reports how
    /// late it was rather than what it was supposed to be.
    Tick,
    /// The window changed size, and the layout may need to follow.
    Resized(f32, f32),
    /// The system clock or timezone moved under us.
    Discontinuity(Discontinuity),
    /// The desktop's light/dark preference changed.
    SystemAppearance(bool),
    /// Speak a different language, from now and next launch.
    ChooseLanguage(Language),

    // --- the shell ---
    /// Go to a destination by index.
    Navigate(usize),
    /// Cycle the appearance.
    ToggleAppearance,
    /// Choose an appearance explicitly.
    ChooseAppearance(ThemeMode),
    /// Move the keyboard focus through the current page.
    FocusNext,
    /// Move the keyboard focus backwards.
    FocusPrevious,
    /// Activate whatever currently holds the focus.
    ActivateFocus,
    /// A key was pressed, for the shortcuts the shell owns.
    Key(Key),
    /// Show or hide the keyboard shortcut list.
    ToggleShortcuts,
    /// Dismiss whatever is on top: a dialog, a ringing alarm, a toast.
    Dismiss,

    // --- the clock page's preferences ---
    /// Switch between 12- and 24-hour time.
    ToggleHourFormat,
    /// Show or hide the seconds column.
    ToggleSeconds,
    /// Show or hide the analog dial.
    ToggleAnalog,

    // --- the world clock ---
    /// The search box changed.
    SearchCities(String),
    /// Open or close the city picker.
    ToggleCityPicker,
    /// Add a city from the picker.
    AddCity(String),
    /// Remove a city.
    RemoveCity(u64),
    /// Pin or unpin a city.
    TogglePin(u64),
    /// Change the ordering of the world clock.
    CycleSort,
    /// Choose an ordering.
    ChooseSort(SortMode),

    // --- alarms ---
    /// Open the editor for a new alarm.
    NewAlarm,
    /// Open the editor for an existing alarm.
    EditAlarm(u64),
    /// Close the alarm editor without saving.
    ///
    /// Only ever touches the alarm editor: each dialog's cancel belongs to the
    /// dialog it closes, so withdrawing a timer's draft cannot be spelled the
    /// same way as withdrawing an alarm's.
    CancelAlarmEdit,
    /// Create or update the alarm being edited.
    SaveAlarm,
    /// Delete an alarm.
    DeleteAlarm(u64),
    /// Withdraw the deletion, keeping the alarm.
    CancelDeleteAlarm,
    /// Confirm a deletion.
    ConfirmDeleteAlarm,
    /// Arm or disarm an alarm.
    ToggleAlarm(u64),
    /// Change the edited alarm's name.
    SetAlarmName(String),
    /// Change the edited alarm's hour.
    AlarmHourStep(i32),
    /// Change the edited alarm's minute.
    AlarmMinuteStep(i32),
    /// Set the edited alarm's repeat rule.
    SetRepeat(Repeat),
    /// Toggle one weekday in the edited alarm.
    ToggleWeekday(u8),
    /// Set the edited alarm's sound.
    SetAlarmSound(Sound),
    /// Set the edited alarm's snooze length.
    SetSnoozeMinutes(u8),
    /// Silence a ringing alarm without snoozing it.
    DismissAlarm(u64),
    /// Postpone a ringing alarm.
    SnoozeAlarm(u64),
    /// Postpone a ringing alarm by a specific number of minutes.
    SnoozeAlarmFor(u64, u8),
    /// Acknowledge a missed alarm.
    AcknowledgeMissed(u64),
    /// Acknowledge every missed alarm.
    AcknowledgeAllMissed,

    // --- timers ---
    /// Start a timer from a preset.
    StartPreset(u64),
    /// Open the custom timer editor.
    NewTimer,
    /// Close the timer editor without starting anything.
    ///
    /// The timer editor's own cancel: it drops the draft being written, which is
    /// a different piece of state from an alarm's, and nothing else.
    CancelTimerEdit,
    /// Set the edited timer's name.
    SetTimerName(String),
    /// Adjust the edited timer's duration.
    TimerDurationStep(i32),
    /// Create the timer being edited.
    ConfirmTimer,
    /// Start or pause a timer.
    ToggleTimer(u64),
    /// Restart a timer from the beginning.
    ResetTimer(u64),
    /// Remove a timer.
    RemoveTimer(u64),
    /// Save the edited timer as a preset.
    ///
    /// There is no control that sends this: the timer editor has no save button,
    /// so it is deliberately absent from the focus list rather than given an
    /// index of its own.
    SaveTimerPreset,
    /// Remove a preset.
    RemovePreset(u64),
    /// Set the sound a finished timer plays.
    SetTimerSound(Sound),

    // --- the stopwatch ---
    /// Start, pause or resume.
    ToggleStopwatch,
    /// Return to zero.
    ResetStopwatch,
    /// Record a lap.
    LapStopwatch,
    /// Clear the lap history without resetting the elapsed time.
    ClearLaps,
    /// Drop a lap from the history.
    RemoveLap(usize),

    // --- sound settings ---
    /// Turn sound on or off.
    ToggleSound,
    /// Turn desktop notifications on or off.
    ToggleNotifications,

    // --- persistence and reporting ---
    /// Persist the configuration.
    Save,
    /// Discard a reported problem.
    DismissNotice,
    /// Re-read the configuration from disk.
    Reload,
    /// The window was closed.
    Exit,
}

/// A key the shell reacts to, rather than every key on the keyboard.
///
/// Only the keys OClock itself owns are named here. Typing into a field is the
/// field's business, and Iced delivers it to the field first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Return.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Shift-tab.
    BackTab,
    /// The space bar.
    Space,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// A digit, 1 to 9.
    Digit(u8),
    /// `?` with shift held.
    Question,
    /// `S` — the page's primary action: start or pause.
    Start,
    /// `L` — record a lap.
    Lap,
    /// `R` — reset the page's measurement.
    Reset,
    /// `N` — start something new.
    New,
}

impl Message {
    /// The navigation destinations, in shell order.
    pub fn destinations() -> &'static [Destination] {
        static DESTINATIONS: std::sync::OnceLock<Vec<Destination>> = std::sync::OnceLock::new();
        DESTINATIONS.get_or_init(crate::ui::components::navigation::destinations)
    }

    /// A short, human description, for the log and for tests.
    pub fn describe(&self) -> &'static str {
        match self {
            Message::Tick => "tick",
            Message::Resized(..) => "resized",
            Message::Discontinuity(_) => "clock moved",
            Message::SystemAppearance(_) => "system appearance",
            Message::ChooseLanguage(_) => "language",
            Message::Navigate(_) => "navigate",
            Message::ToggleAppearance => "toggle appearance",
            Message::ChooseAppearance(_) => "choose appearance",
            Message::FocusNext => "focus next",
            Message::FocusPrevious => "focus previous",
            Message::ActivateFocus => "activate",
            Message::Key(_) => "key",
            Message::ToggleShortcuts => "shortcuts",
            Message::Dismiss => "dismiss",
            Message::ToggleHourFormat => "hour format",
            Message::ToggleSeconds => "seconds",
            Message::ToggleAnalog => "analog",
            Message::SearchCities(_) => "search",
            Message::ToggleCityPicker => "city picker",
            Message::AddCity(_) => "add city",
            Message::RemoveCity(_) => "remove city",
            Message::TogglePin(_) => "pin",
            Message::CycleSort => "sort",
            Message::ChooseSort(_) => "sort",
            Message::NewAlarm => "new alarm",
            Message::EditAlarm(_) => "edit alarm",
            Message::CancelAlarmEdit => "cancel alarm",
            Message::CancelDeleteAlarm => "cancel delete",
            Message::SaveAlarm => "save alarm",
            Message::DeleteAlarm(_) => "delete alarm",
            Message::ConfirmDeleteAlarm => "confirm delete",
            Message::ToggleAlarm(_) => "toggle alarm",
            Message::SetAlarmName(_) => "alarm name",
            Message::AlarmHourStep(_) => "alarm hour",
            Message::AlarmMinuteStep(_) => "alarm minute",
            Message::SetRepeat(_) => "repeat",
            Message::ToggleWeekday(_) => "weekday",
            Message::SetAlarmSound(_) => "alarm sound",
            Message::SetSnoozeMinutes(_) => "snooze",
            Message::DismissAlarm(_) => "dismiss alarm",
            Message::SnoozeAlarm(_) => "snooze alarm",
            Message::SnoozeAlarmFor(..) => "snooze alarm for",
            Message::AcknowledgeMissed(_) => "acknowledge missed",
            Message::AcknowledgeAllMissed => "acknowledge all",
            Message::StartPreset(_) => "start preset",
            Message::NewTimer => "new timer",
            Message::CancelTimerEdit => "cancel timer",
            Message::SetTimerName(_) => "timer name",
            Message::TimerDurationStep(_) => "timer duration",
            Message::ConfirmTimer => "confirm timer",
            Message::ToggleTimer(_) => "toggle timer",
            Message::ResetTimer(_) => "reset timer",
            Message::RemoveTimer(_) => "remove timer",
            Message::SaveTimerPreset => "save preset",
            Message::RemovePreset(_) => "remove preset",
            Message::SetTimerSound(_) => "timer sound",
            Message::ToggleStopwatch => "stopwatch",
            Message::ResetStopwatch => "reset stopwatch",
            Message::LapStopwatch => "lap",
            Message::ClearLaps => "clear laps",
            Message::RemoveLap(_) => "remove lap",
            Message::ToggleSound => "sound",
            Message::ToggleNotifications => "notifications",
            Message::Save => "save",
            Message::DismissNotice => "dismiss notice",
            Message::Reload => "reload",
            Message::Exit => "exit",
        }
    }
}

/// The hour format to switch to.
pub fn other_format(current: HourFormat) -> HourFormat {
    current.toggled()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_describes_itself() {
        // A variant that forgot its description would show up here, which is
        // the only reason the method exists.
        let samples = [
            Message::Tick,
            Message::Navigate(0),
            Message::ToggleHourFormat,
            Message::AddCity("Tokyo".into()),
            Message::NewAlarm,
            Message::SetAlarmName("x".into()),
            Message::ToggleWeekday(0),
            Message::StartPreset(1),
            Message::LapStopwatch,
            Message::Key(Key::Escape),
            Message::Discontinuity(Discontinuity::Steady),
            Message::ChooseLanguage(Language::English),
        ];
        for message in samples {
            assert!(!message.describe().is_empty());
        }
    }

    #[test]
    fn the_shell_owns_five_destinations() {
        assert_eq!(Message::destinations().len(), 5);
        for (index, destination) in Message::destinations().iter().enumerate() {
            assert_eq!(destination.shortcut() as usize, index + 1);
        }
    }

    #[test]
    fn the_hour_format_toggles_both_ways() {
        assert_eq!(other_format(HourFormat::Twelve), HourFormat::TwentyFour);
        assert_eq!(other_format(HourFormat::TwentyFour), HourFormat::Twelve);
    }

    #[test]
    fn keys_are_comparable() {
        assert_eq!(Key::Escape, Key::Escape);
        assert_ne!(Key::Enter, Key::Space);
        assert_ne!(Key::Digit(1), Key::Digit(2));
    }
}
