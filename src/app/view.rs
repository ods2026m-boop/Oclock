//! The view state: what the interface is showing, independent of what the
//! application is *doing*.
//!
//! Kept apart from the feature state on purpose. The alarms themselves belong
//! to the domain; which alarm is being edited, whether the city picker is open
//! and where the keyboard focus sits are the interface's own business, and
//! keeping them here means a change to the interface cannot disturb a feature's
//! state and vice versa.

use std::time::Duration;

use crate::design::motion::Presence;
use crate::design::tokens::layout;
use crate::domain::alarm::Alarm;
use crate::domain::clock::HourFormat;

/// The window's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Sidebar and content side by side.
    Wide,
    /// Marks only in the sidebar, to save width.
    Compact,
    /// A top bar, because the window is too narrow for a sidebar.
    Stacked,
}

impl Layout {
    /// The layout for a window of this size.
    ///
    /// The narrowest test comes first, because the two breakpoints are
    /// ordered: a window too narrow for any sidebar is also too narrow for a
    /// compact one.
    pub fn for_size(width: f32, _height: f32) -> Layout {
        use crate::design::tokens::layout as tokens;

        if width < tokens::NARROW_BREAKPOINT {
            Layout::Stacked
        } else if width < tokens::COMPACT_BREAKPOINT {
            Layout::Compact
        } else {
            Layout::Wide
        }
    }

    /// Whether the navigation shows its labels.
    pub fn is_expanded(self) -> bool {
        matches!(self, Layout::Wide)
    }

    /// Whether the shell puts the navigation beside the content.
    pub fn has_sidebar(self) -> bool {
        !matches!(self, Layout::Stacked)
    }
}

/// A draft of an alarm, being edited.
#[derive(Clone, Debug, PartialEq)]
pub struct AlarmDraft {
    /// The alarm being edited, or a new one.
    pub alarm: Alarm,
    /// True when this draft is for an alarm that does not exist yet.
    pub is_new: bool,
}

impl AlarmDraft {
    /// A draft for a new alarm at the next sensible time.
    pub fn new(id: u64, hour: u32, minute: u32) -> AlarmDraft {
        AlarmDraft {
            alarm: Alarm::new(id, hour, minute),
            is_new: true,
        }
    }

    /// A draft for an existing alarm.
    pub fn existing(alarm: Alarm) -> AlarmDraft {
        AlarmDraft {
            alarm,
            is_new: false,
        }
    }

    /// Steps the hour, wrapping at both ends.
    pub fn step_hour(&mut self, delta: i32) {
        self.alarm.hour = wrap(self.alarm.hour as i32 + delta, 24) as u32;
    }

    /// Steps the minute, wrapping at both ends.
    pub fn step_minute(&mut self, delta: i32) {
        self.alarm.minute = wrap(self.alarm.minute as i32 + delta, 60) as u32;
    }

    /// The draft's wall time, in the reader's chosen clock format.
    ///
    /// The 12-hour form carries a meridiem word, so this needs the catalogue
    /// as well as the format.
    pub fn time(&self, format: HourFormat, catalog: crate::services::i18n::Catalog) -> String {
        catalog.alarm_time(&self.alarm, format)
    }

    /// The draft's wall time as its digits and its meridiem, for a reading that
    /// sets the two at different sizes.
    pub fn time_parts(
        &self,
        format: HourFormat,
        catalog: crate::services::i18n::Catalog,
    ) -> (String, Option<&'static str>) {
        catalog.alarm_time_parts(&self.alarm, format)
    }
}

fn wrap(value: i32, limit: i32) -> i32 {
    let value = value % limit;
    if value < 0 {
        value + limit
    } else {
        value
    }
}

/// A draft of a timer, being created.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerDraft {
    /// The name the user gave it.
    pub name: String,
    /// How long it will run for.
    pub duration: Duration,
}

impl TimerDraft {
    /// A draft for a new timer.
    pub fn new(name: impl Into<String>, duration: Duration) -> TimerDraft {
        TimerDraft {
            name: name.into(),
            duration,
        }
    }

    /// Steps the duration by whole minutes, never below one minute.
    pub fn step(&mut self, minutes: i32) {
        let total = self.duration.as_secs() as i64 + i64::from(minutes) * 60;
        self.duration = Duration::from_secs(total.clamp(60, 24 * 3600) as u64);
    }

    /// The name to show when the user has not chosen one.
    ///
    /// The fallback is the timer's own length, described in the language being
    /// read rather than in English, so an unnamed timer is called `5m 05s` in
    /// English and `5 دقیقه و 05 ثانیه` in Persian.
    pub fn name_or_default(&self, catalog: crate::services::i18n::Catalog) -> String {
        let trimmed = self.name.trim();
        if trimmed.is_empty() {
            catalog.duration(self.duration)
        } else {
            trimmed.to_string()
        }
    }
}

/// An alarm that rang and is waiting to be dealt with.
#[derive(Clone, Debug, PartialEq)]
pub struct Ringing {
    /// The alarm that rang.
    pub alarm: Alarm,
    /// When it was due.
    pub due: crate::core::tz::Zoned,
    /// How late it was.
    pub late_by: Duration,
    /// Whether the user can still snooze it. False once it is too stale.
    pub snoozable: bool,
}

/// A dismissed toast, waiting to disappear.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// What it says.
    pub text: String,
    /// Its tone: positive by default, danger for a failure.
    pub tone: crate::design::palette::Tone,
    /// Its animated presence.
    pub presence: Presence,
    /// True once the toast has been asked to leave.
    pub leaving: bool,
    /// True once its exit has actually been started.
    ///
    /// Kept apart from `leaving` because the exit must be started exactly once:
    /// re-starting it on every tick would keep the presence at the beginning of
    /// its animation, and the toast would never finish leaving.
    pub exiting: bool,
    /// True once its entrance has actually been started, for the same reason.
    pub entering: bool,
}

/// The view state.
#[derive(Debug)]
pub struct View {
    /// The window's shape.
    pub layout: Layout,
    /// The window's size, which a dialog measures itself against.
    pub window: (f32, f32),
    /// The previous page, while one is sliding out.
    pub leaving: Option<usize>,
    /// How far the leaving page has got, `0.0..=1.0`.
    pub leaving_progress: f32,
    /// How far the arriving page has got.
    pub entering_progress: f32,
    /// The page currently on screen, as an index into the shell's destinations.
    pub page: usize,

    /// The alarm being edited.
    pub alarm_draft: Option<AlarmDraft>,
    /// The alarm whose deletion is being confirmed.
    pub pending_delete: Option<u64>,
    /// Alarms that rang and have not been dealt with.
    pub ringing: Vec<Ringing>,
    /// Alarms whose moment passed while the application was not watching.
    pub missed: Vec<(Alarm, Duration)>,

    /// The timer being created.
    pub timer_draft: Option<TimerDraft>,
    /// The city picker.
    pub city_picker: bool,
    /// The picker's search text.
    pub query: String,

    /// The keyboard focus, as an index into the current page's action list.
    pub focus: Option<usize>,
    /// True while the user is typing into a field, so the shell's shortcuts
    /// stay out of the way.
    pub typing: bool,
    /// Whether the shortcut list is showing.
    pub shortcuts: bool,

    /// Toasts currently on screen.
    pub toasts: Vec<Toast>,
    /// A one-off banner about the configuration.
    pub notice: Option<Notice>,

    /// How long a toast stays up, in milliseconds.
    pub toast_duration: Duration,
    /// When the current toast should leave.
    pub toast_expires: Duration,
    /// Monotonic time of the last tick.
    pub now: Duration,
}

/// A banner about something the user should know about.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// The headline.
    pub headline: String,
    /// The detail.
    pub detail: String,
    /// Its tone: caution for a recovery, danger for a failure.
    pub tone: crate::design::palette::Palette,
    /// Whether it is about damage that was repaired.
    pub recoverable: bool,
}

impl View {
    /// The view state for a fresh window.
    pub fn new(now: Duration) -> View {
        View {
            layout: Layout::Wide,
            window: (layout::WINDOW.0 as f32, layout::WINDOW.1 as f32),
            leaving: None,
            leaving_progress: 1.0,
            entering_progress: 1.0,
            page: 0,
            alarm_draft: None,
            pending_delete: None,
            ringing: Vec::new(),
            missed: Vec::new(),
            timer_draft: None,
            city_picker: false,
            query: String::new(),
            focus: None,
            typing: false,
            shortcuts: false,
            toasts: Vec::new(),
            notice: None,
            toast_duration: Duration::from_millis(2600),
            toast_expires: Duration::ZERO,
            now,
        }
    }

    /// True when something is layered over the page and should absorb Escape.
    pub fn has_overlay(&self) -> bool {
        self.shortcuts
            || self.alarm_draft.is_some()
            || self.timer_draft.is_some()
            || self.city_picker
            || self.pending_delete.is_some()
            || !self.ringing.is_empty()
    }

    /// True when the topmost overlay is a dialog that should block page
    /// switching.
    pub fn has_modal(&self) -> bool {
        self.alarm_draft.is_some()
            || self.timer_draft.is_some()
            || self.city_picker
            || self.pending_delete.is_some()
    }

    /// Records a transition to `page`, starting the cross-fade.
    pub fn go_to(&mut self, page: usize) {
        if page == self.page {
            return;
        }
        self.leaving = Some(self.page);
        self.leaving_progress = 0.0;
        self.entering_progress = 0.0;
        self.page = page;
        // The focus belongs to a page; carrying it across would highlight
        // whatever happens to sit at the same index on the new one.
        self.focus = None;
    }

    /// The progress of the page transition, `0.0..=1.0`.
    pub fn transition(&self) -> f32 {
        self.entering_progress
    }

    /// A toast to show.
    pub fn toast(&mut self, text: impl Into<String>, tone: crate::design::palette::Tone) {
        self.toasts.clear();
        self.toasts.push(Toast {
            text: text.into(),
            tone,
            // Hidden, so the first `advance` starts the entrance from nothing
            // rather than from wherever the last toast left its presence.
            presence: Presence::hidden(),
            leaving: false,
            exiting: false,
            entering: false,
        });
        self.toast_expires = self.now + self.toast_duration;
    }

    /// Advances every animation in the view.
    pub fn advance(&mut self, now: Duration) {
        self.now = now;

        // Each of a toast's two animations is started exactly once. Re-stamping
        // the entrance on every tick would look like it worked — the value
        // creeps towards 1 — but it would be an exponential approach towards 1
        // rather than the 180ms curve the code asks for, so the toast would
        // settle in a fraction of the time it is meant to take and would never
        // be seen to have finished arriving.
        for toast in &mut self.toasts {
            if !toast.leaving {
                if !toast.entering {
                    toast.presence.show(now, crate::design::motion::Spec::ENTER);
                    toast.entering = true;
                }
            } else if !toast.exiting {
                toast.presence.hide(now, crate::design::motion::Spec::EXIT);
                toast.exiting = true;
            }
        }
        self.toasts
            .retain(|toast| !(toast.leaving && toast.presence.is_settled(now)));
    }

    /// Marks the current toast for removal.
    pub fn dismiss_toast(&mut self) {
        for toast in &mut self.toasts {
            toast.leaving = true;
        }
    }

    /// Whether a toast's time is up.
    pub fn toast_expired(&self) -> bool {
        !self.toasts.is_empty() && self.now >= self.toast_expires
    }

    /// The page currently on screen, as a destination name.
    pub fn page_name(&self) -> &'static str {
        crate::app::message::Message::destinations()
            .get(self.page)
            .map_or("oclock", |destination| destination.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alarm::Repeat;
    use chrono::Weekday;
    use std::time::Duration;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_fresh_view_starts_on_the_clock_with_nothing_over_it() {
        let view = View::new(Duration::ZERO);
        assert_eq!(view.page, 0);
        assert!(!view.has_overlay());
        assert!(!view.has_modal());
        assert!(view.toasts.is_empty());
        assert!(view.alarm_draft.is_none());
    }

    #[test]
    fn the_layout_follows_the_window() {
        assert_eq!(Layout::for_size(1400.0, 900.0), Layout::Wide);
        assert_eq!(Layout::for_size(900.0, 700.0), Layout::Compact);
        assert_eq!(Layout::for_size(600.0, 700.0), Layout::Stacked);

        assert!(Layout::Wide.is_expanded());
        assert!(!Layout::Compact.is_expanded());
        assert!(Layout::Wide.has_sidebar());
        assert!(!Layout::Stacked.has_sidebar());
    }

    #[test]
    fn the_breakpoints_are_where_they_say() {
        use crate::design::tokens::layout as tokens;
        // A middle sample between the two breakpoints, so the table above
        // cannot describe three layouts if the middle one became unreachable.
        assert_eq!(
            Layout::for_size(
                (tokens::NARROW_BREAKPOINT + tokens::COMPACT_BREAKPOINT) / 2.0,
                800.0
            ),
            Layout::Compact
        );
        assert_eq!(
            Layout::for_size(tokens::NARROW_BREAKPOINT - 1.0, 800.0),
            Layout::Stacked
        );
        assert_eq!(
            Layout::for_size(tokens::NARROW_BREAKPOINT + 1.0, 800.0),
            Layout::Compact
        );
        assert_eq!(
            Layout::for_size(tokens::COMPACT_BREAKPOINT - 1.0, 800.0),
            Layout::Compact
        );
        assert_eq!(
            Layout::for_size(tokens::COMPACT_BREAKPOINT + 1.0, 800.0),
            Layout::Wide
        );
    }

    #[test]
    fn changing_page_starts_a_transition_and_forgets_the_focus() {
        let mut view = View::new(Duration::ZERO);
        view.focus = Some(3);
        view.go_to(2);

        assert_eq!(view.page, 2);
        assert_eq!(view.leaving, Some(0));
        assert_eq!(view.entering_progress, 0.0);
        assert_eq!(view.focus, None, "focus does not carry across pages");
    }

    #[test]
    fn going_to_the_page_you_are_on_does_nothing() {
        let mut view = View::new(Duration::ZERO);
        view.go_to(0);
        assert_eq!(view.leaving, None);
        assert_eq!(view.entering_progress, 1.0);
    }

    #[test]
    fn every_overlay_is_reported() {
        let mut view = View::new(Duration::ZERO);

        view.shortcuts = true;
        assert!(view.has_overlay() && !view.has_modal());

        view.shortcuts = false;
        view.city_picker = true;
        assert!(view.has_overlay() && view.has_modal());

        view.city_picker = false;
        view.alarm_draft = Some(AlarmDraft::new(1, 7, 0));
        assert!(view.has_modal());

        view.alarm_draft = None;
        view.ringing.push(Ringing {
            alarm: Alarm::new(1, 7, 0),
            due: crate::core::tz::now_in(chrono_tz::UTC),
            late_by: Duration::ZERO,
            snoozable: true,
        });
        assert!(view.has_overlay() && !view.has_modal());
    }

    #[test]
    fn an_alarm_draft_steps_its_time_both_ways() {
        let mut draft = AlarmDraft::new(1, 7, 0);
        draft.step_hour(1);
        assert_eq!(draft.alarm.hour, 8);
        draft.step_hour(23);
        assert_eq!(draft.alarm.hour, 7, "stepping past midnight wraps");

        draft.step_minute(-1);
        assert_eq!(draft.alarm.minute, 59, "stepping below zero wraps");
        draft.step_minute(1);
        assert_eq!(draft.alarm.minute, 0);
    }

    #[test]
    fn a_draft_shows_its_time_in_the_chosen_format() {
        let draft = AlarmDraft::new(1, 21, 5);
        let english = crate::services::i18n::Catalog::english();
        assert_eq!(draft.time(HourFormat::TwentyFour, english), "21:05");
        assert_eq!(draft.time(HourFormat::Twelve, english), "09:05 PM");
    }

    #[test]
    fn an_existing_draft_is_not_new() {
        let draft = AlarmDraft::existing(Alarm::new(4, 6, 0));
        assert!(!draft.is_new);
    }

    #[test]
    fn a_timer_draft_stays_within_sensible_bounds() {
        let mut draft = TimerDraft::new("Tea", Duration::from_secs(300));
        draft.step(1);
        assert_eq!(draft.duration, Duration::from_secs(360));

        // Never below a minute, however many decrements arrive.
        for _ in 0..20 {
            draft.step(-1);
        }
        assert_eq!(draft.duration, Duration::from_secs(60));

        // And never past a day, which is a whole number of hours.
        for _ in 0..2_000 {
            draft.step(1);
        }
        assert_eq!(draft.duration, Duration::from_secs(86_400));
    }

    #[test]
    fn an_unnamed_timer_falls_back_to_its_length() {
        let english = crate::services::i18n::Catalog::english();
        let draft = TimerDraft::new("   ", Duration::from_secs(305));
        assert_eq!(draft.name_or_default(english), "5m 05s");

        let named = TimerDraft::new("Tea", Duration::from_secs(305));
        assert_eq!(named.name_or_default(english), "Tea");
    }

    #[test]
    fn a_toast_appears_and_then_leaves() {
        let mut view = View::new(Duration::ZERO);
        view.toast("Timer started", crate::design::palette::Tone::Positive);
        assert_eq!(view.toasts.len(), 1);
        assert!(!view.toast_expired());

        view.advance(view.toast_expires + ms(10));
        assert!(view.toast_expired());

        // Dismissing starts the exit at the current moment, so the toast lives
        // on for the length of that exit and is then dropped.
        let dismissed_at = view.now;
        view.dismiss_toast();
        view.advance(dismissed_at + ms(10));
        assert_eq!(view.toasts.len(), 1, "an exit is not instantaneous");
        view.advance(dismissed_at + ms(400));
        assert!(
            view.toasts.is_empty(),
            "a dismissed toast must be dropped once its exit has run"
        );
    }

    #[test]
    fn a_second_toast_replaces_the_first() {
        let mut view = View::new(Duration::ZERO);
        view.toast("First", crate::design::palette::Tone::Positive);
        view.advance(ms(100));
        view.toast("Second", crate::design::palette::Tone::Danger);
        assert_eq!(view.toasts.len(), 1);
        assert_eq!(view.toasts[0].text, "Second");
    }

    #[test]
    fn an_alarm_draft_carries_a_repeat_rule_untouched() {
        let draft = AlarmDraft::existing(Alarm {
            repeat: Repeat::Weekly(crate::domain::alarm::Weekdays::from_days([Weekday::Fri])),
            ..Alarm::new(1, 6, 0)
        });
        // The rule itself is data and survives untouched; only the *wording* of
        // a label moved to the catalogue, which has its own tests for this.
        assert_eq!(
            draft.alarm.repeat,
            Repeat::Weekly(crate::domain::alarm::Weekdays::from_days([Weekday::Fri]))
        );
    }
}
