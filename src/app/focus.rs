//! Which control the keyboard focus is on, and what pressing it does.
//!
//! Iced 0.13 has no focus traversal for buttons, so OClock has its own: the
//! window's controls are listed once, in reading order, and both the focus ring
//! and the key that activates it read that same list. That is the whole point
//! of the arrangement — a second, hand-maintained copy of the order cannot
//! disagree with the first without anything noticing, which is how the ring ends
//! up on a control that is not there and the key that does nothing.
//!
//! The order is
//!
//! 1. the shell's own controls, which sit before the content in reading order:
//!    the five destinations, then the appearance control, then the language
//!    control when there is a rail to put it in;
//! 2. the current page's header controls;
//! 3. the current page's own controls, in the order they are laid out.
//!
//! A modal dialog takes the list over entirely, for as long as it is up. It is
//! the only thing the keyboard is talking to while it is there, and a page
//! behind a scrim is not something a key can be aimed at.
//!
//! # Indices are derived, never counted
//!
//! [`OClock::page_focus`] adds the shell's length to a page-local index, and
//! each page asks for its own row bases through the same functions the lists are
//! built from. A list that grows a row therefore moves every later row with it,
//! and a page cannot draw a ring on an index the activation path does not have.
//!
//! Nothing is listed that is not drawn. A control the page does not draw — a
//! card's pin button on a zone this build cannot resolve, a search field that
//! only exists inside a dialog — is not an entry here, because a focus target
//! that activates something is worse than one that cannot be reached.

use iced::Task;

use crate::app::message::Message;
use crate::app::OClock;

/// The focus indices the dialogs draw their own controls at.
///
/// A dialog's controls start at zero, because a dialog *is* the whole list while
/// it is up: nothing behind it is reachable, so nothing behind it is numbered.
pub mod dialog {
    /// The alarm editor's hour stepper.
    pub const ALARM_HOUR: usize = 0;
    /// The alarm editor's minute stepper.
    pub const ALARM_MINUTE: usize = 1;
    /// The alarm editor's save button.
    pub const ALARM_CONFIRM: usize = 2;

    /// The timer editor's hour stepper.
    pub const TIMER_HOURS: usize = 0;
    /// The timer editor's minute stepper.
    pub const TIMER_MINUTES: usize = 1;
    /// The timer editor's start button.
    pub const TIMER_CONFIRM: usize = 2;

    /// The deletion question's delete button.
    pub const DELETE_CONFIRM: usize = 0;
    /// The deletion question's cancel button.
    pub const DELETE_CANCEL: usize = 1;

    /// The city picker's done button.
    pub const PICKER_CLOSE: usize = 0;
}

/// The shell's own controls, which come before the page's in reading order.
///
/// The destinations are first because they are first: a rail runs down the
/// leading edge, and a top bar runs across it above the content.
fn shell_targets(app: &OClock) -> Vec<Message> {
    let mut targets: Vec<Message> = (0..Message::destinations().len())
        .map(Message::Navigate)
        .collect();

    // The appearance control is drawn in every layout — down the rail, or at
    // the end of the top bar — so it keeps the same index in both. It asks the
    // shell what pressing it sends, rather than deciding for itself, so the
    // keyboard and the mouse cannot end up meaning different things by it.
    targets.push(crate::ui::shell::appearance_message(app));

    // The language control only exists beside a rail: a top bar has room for
    // the destinations and the appearance control, and no more. Listing it in
    // the narrow layout would be a focus target with nothing behind it.
    if app.view.layout.has_sidebar() {
        targets.push(crate::ui::shell::language_message(app));
    }

    targets
}

impl OClock {
    /// Every control the window can put the focus on, in reading order.
    pub fn focus_targets(&self) -> Vec<Message> {
        if self.view.has_modal() {
            return self.dialog_targets();
        }

        let mut targets = shell_targets(self);
        targets.extend(self.page_targets());
        targets
    }

    /// How many controls the focus can move through.
    pub fn focus_count(&self) -> usize {
        self.focus_targets().len()
    }

    /// The window index of the shell's own control number `index`.
    ///
    /// The shell's controls are the first ones in the list, so its own
    /// numbering is the window's.
    pub fn shell_focused(&self, index: usize) -> bool {
        self.view.focus == Some(index) && index < shell_targets(self).len()
    }

    /// The window index of the current page's own control number `index`.
    ///
    /// `None` past the end of the page's controls, so a ring cannot be drawn on
    /// an index the activation path does not have.
    pub fn page_focus(&self, index: usize) -> Option<usize> {
        if index >= self.page_targets().len() {
            return None;
        }
        shell_targets(self).len().checked_add(index)
    }

    /// The page's own controls, in reading order.
    pub fn page_targets(&self) -> Vec<Message> {
        match self.view.page {
            0 => self.clock_page_actions(),
            1 => self.world_page_actions(),
            2 => self.alarm_page_actions(),
            3 => self.timer_page_actions(),
            4 => self.stopwatch_page_actions(),
            _ => Vec::new(),
        }
    }

    /// The controls a dialog puts the whole focus list to use while it is up.
    ///
    /// A stepper is one target, not two: the keyboard has no way to say which
    /// of a stepper's two buttons was meant, so it raises the step. Both
    /// directions are still a click away, which is what a stepper is for.
    pub fn dialog_targets(&self) -> Vec<Message> {
        if self.view.alarm_draft.is_some() {
            vec![
                Message::AlarmHourStep(1),
                Message::AlarmMinuteStep(1),
                Message::SaveAlarm,
            ]
        } else if self.view.timer_draft.is_some() {
            vec![
                Message::TimerDurationStep(60),
                Message::TimerDurationStep(5),
                Message::ConfirmTimer,
            ]
        } else if self.view.city_picker {
            vec![Message::ToggleCityPicker]
        } else if self.view.pending_delete.is_some() {
            vec![Message::ConfirmDeleteAlarm, Message::CancelDeleteAlarm]
        } else {
            Vec::new()
        }
    }

    /// The clock page's controls.
    ///
    /// The three preference chips, in the order they are laid out. The analog
    /// toggle in the header is the same action as the dial chip and shares its
    /// index, so the two never disagree about what the third control does.
    pub fn clock_page_actions(&self) -> Vec<Message> {
        vec![
            Message::ToggleHourFormat,
            Message::ToggleSeconds,
            Message::ToggleAnalog,
        ]
    }

    /// The world page's controls: the two header buttons, then each card's.
    pub fn world_page_actions(&self) -> Vec<Message> {
        let mut actions = vec![Message::ToggleCityPicker, Message::CycleSort];

        for row in &self.snapshot.world {
            if row.at.is_none() {
                // A card for a zone this build cannot resolve draws one button,
                // to take the city away. It has no pin, because there is no
                // local time on it to be ahead or behind.
                actions.push(Message::RemoveCity(row.location.id));
            } else {
                actions.push(Message::TogglePin(row.location.id));
                actions.push(Message::RemoveCity(row.location.id));
            }
        }

        actions
    }

    /// The page-local index of the first control on world card `index`.
    ///
    /// The two the header takes come first, then each card before this one, at
    /// the length [`WorldClocks::ordered`](crate::domain::world::WorldClocks::ordered)
    /// gave it: one control for a card whose zone does not resolve, two for
    /// every other.
    pub fn world_card_base(&self, index: usize) -> usize {
        self.snapshot
            .world
            .iter()
            .take(index)
            .fold(2, |next, row| next + if row.at.is_none() { 1 } else { 2 })
    }

    /// The alarms page's controls.
    pub fn alarm_page_actions(&self) -> Vec<Message> {
        let mut actions = vec![Message::NewAlarm];

        if self.alarms.is_empty() {
            // The empty state draws its own button in place of the list. The
            // missed-alarm notice is not drawn while the empty state is up, so
            // it is not listed either.
            actions.push(Message::NewAlarm);
            return actions;
        }

        if !self.view.missed.is_empty() {
            actions.push(Message::AcknowledgeAllMissed);
        }

        for alarm in self.alarms.alarms() {
            actions.push(Message::ToggleAlarm(alarm.id));
            actions.push(Message::EditAlarm(alarm.id));
            actions.push(Message::DeleteAlarm(alarm.id));
        }

        actions
    }

    /// The page-local index of the first alarm row's first control.
    pub fn alarm_row_base(&self) -> usize {
        if self.alarms.is_empty() {
            return 2;
        }
        1 + usize::from(!self.view.missed.is_empty())
    }

    /// The timers page's controls.
    pub fn timer_page_actions(&self) -> Vec<Message> {
        let mut actions = vec![Message::NewTimer];

        for preset in self.timers.presets() {
            actions.push(Message::StartPreset(preset.id));
        }

        for timer in self.timers.timers() {
            actions.push(Message::ToggleTimer(timer.id));
            actions.push(Message::ResetTimer(timer.id));
            actions.push(Message::RemoveTimer(timer.id));
        }

        actions
    }

    /// The page-local index of the first timer card's first control.
    pub fn timer_row_base(&self) -> usize {
        1 + self.timers.presets().len()
    }

    /// The stopwatch page's controls.
    pub fn stopwatch_page_actions(&self) -> Vec<Message> {
        let mut actions = vec![
            Message::ToggleStopwatch,
            Message::LapStopwatch,
            Message::ResetStopwatch,
        ];

        // Laps are drawn newest first, and the list is in that same order: the
        // target at position `k` has to be the lap the row drawn there is
        // showing, which is the `count - 1 - k`th recorded one. Listing them in
        // the order they were recorded would put the ring on the newest lap and
        // delete the oldest.
        let count = self.stopwatch.lap_count();
        for position in 0..count {
            actions.push(Message::RemoveLap(count - 1 - position));
        }

        actions
    }

    /// The page-local index of the first lap row's remove button.
    pub fn lap_row_base(&self) -> usize {
        3
    }

    /// The message a focus index activates, if there is one.
    pub fn focus_at(&self, index: usize) -> Option<Message> {
        self.focus_targets().get(index).cloned()
    }

    /// Runs whatever the control at `index` does, as pressing it would.
    pub fn activate(&mut self, index: usize) -> Task<Message> {
        match self.focus_at(index) {
            Some(message) => self.update(message),
            None => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{app_with, app_with_alarms, app_with_timers};
    use std::time::Duration;

    fn on_page(app: &mut OClock, page: usize) {
        app.view.page = page;
        app.view.focus = None;
    }

    #[test]
    fn the_count_and_the_list_agree_on_every_page() {
        let mut app = app_with(vec![], vec![], vec![]);

        for page in 0..5 {
            on_page(&mut app, page);
            let shell = shell_targets(&app).len();
            assert_eq!(
                app.focus_count(),
                app.page_targets().len() + shell,
                "page {page}: the shell's controls and the page's own are one list"
            );
            for index in 0..app.focus_count() {
                assert!(
                    app.focus_at(index).is_some(),
                    "page {page}: index {index} is counted but cannot be reached"
                );
            }
            assert_eq!(
                app.focus_at(app.focus_count()),
                None,
                "page {page}: nothing is counted past the end"
            );
        }
    }

    #[test]
    fn the_shell_comes_first_and_keeps_the_same_number_in_every_layout() {
        let mut app = app_with(vec![], vec![], vec![]);

        for layout in [
            crate::app::view::Layout::Wide,
            crate::app::view::Layout::Compact,
            crate::app::view::Layout::Stacked,
        ] {
            app.view.layout = layout;
            let shell = shell_targets(&app);

            for (index, message) in shell.iter().enumerate() {
                assert!(
                    matches!(message, Message::Navigate(_)) || index >= 5,
                    "the destinations lead the list, in {layout:?}"
                );
            }
            assert!(
                matches!(shell[5], Message::ChooseAppearance(_)),
                "the appearance control follows the five destinations, in {layout:?}"
            );

            // The language control is a rail control: a top bar has room for the
            // destinations and the appearance control, and no more.
            let has_language = shell.len() == 7;
            assert_eq!(
                has_language,
                layout.has_sidebar(),
                "the language control is listed only beside a rail, in {layout:?}"
            );
        }
    }

    #[test]
    fn the_clock_page_lists_the_three_chips_and_the_header_shares_the_third() {
        let app = app_with(vec![], vec![], vec![]);
        assert_eq!(
            app.clock_page_actions(),
            vec![
                Message::ToggleHourFormat,
                Message::ToggleSeconds,
                Message::ToggleAnalog
            ],
            "the three chips, whether or not the dial is on screen"
        );
    }

    #[test]
    fn the_world_page_lists_the_header_and_every_card() {
        let mut app = app_with(vec![], vec![], vec![]);
        on_page(&mut app, 1);

        let actions = app.world_page_actions();
        assert_eq!(actions[0], Message::ToggleCityPicker, "the add-city button");
        assert_eq!(actions[1], Message::CycleSort, "the sort button");

        assert_eq!(actions.len(), 2 + 2 * app.snapshot.world.len());
        for (index, row) in app.snapshot.world.iter().enumerate() {
            let base = app.world_card_base(index);
            assert_eq!(actions[base], Message::TogglePin(row.location.id));
            assert_eq!(actions[base + 1], Message::RemoveCity(row.location.id));
        }

        // And the bases agree with the list they were taken from.
        let mut expected = 2;
        for index in 0..app.snapshot.world.len() {
            assert_eq!(app.world_card_base(index), expected);
            expected += 2;
        }
        assert_eq!(expected, actions.len());
    }

    #[test]
    fn a_world_card_without_a_time_lists_only_the_button_it_draws() {
        let mut app = app_with(vec![], vec![], vec![]);
        assert!(app.world.add_city("Olympus", chrono_tz::UTC).is_some());

        // A zone the build cannot resolve is drawn as a warning card with one
        // button on it, so it takes one index and not two.
        app.snapshot.world.push(crate::domain::world::WorldRow {
            location: crate::domain::world::Location {
                id: 99,
                city: "Nowhere".to_string(),
                zone: "Not/AZone".to_string(),
                pinned: false,
            },
            at: None,
            offset: "—".to_string(),
            abbreviation: "Unknown zone".to_string(),
            daylight_saving: false,
            offset_from_local: 0,
            day_relation: crate::domain::world::DayRelation::Today,
            date: String::new(),
        });

        let actions = app.world_page_actions();
        let base = app.world_card_base(app.snapshot.world.len() - 1);
        assert_eq!(actions[base], Message::RemoveCity(99));
        assert_eq!(
            base,
            actions.len() - 1,
            "the card takes one index to the end"
        );
        assert!(
            !actions.contains(&Message::TogglePin(99)),
            "a card with no time on it has no pin to press"
        );
    }

    #[test]
    fn the_alarms_page_lists_the_missed_notice_only_while_it_is_drawn() {
        let alarm = crate::domain::alarm::Alarm::new(7, 6, 0);
        let mut app = app_with_alarms(vec![alarm.clone()], app_with_timers());

        on_page(&mut app, 2);
        assert_eq!(app.alarm_page_actions()[0], Message::NewAlarm);
        assert_eq!(
            app.alarm_row_base(),
            1,
            "no notice, so the list starts next"
        );
        assert!(!app
            .alarm_page_actions()
            .contains(&Message::AcknowledgeAllMissed));

        app.view
            .missed
            .push((alarm.clone(), Duration::from_secs(30)));
        assert_eq!(app.alarm_row_base(), 2, "the notice takes one index");
        assert_eq!(app.alarm_page_actions()[1], Message::AcknowledgeAllMissed);
        assert_eq!(app.alarm_page_actions()[2], Message::ToggleAlarm(7));

        // With no alarms at all the empty state offers its own button, and the
        // notice is not on screen to be listed.
        let mut empty = app_with_alarms(vec![], app_with_timers());
        on_page(&mut empty, 2);
        empty.view.missed.push((alarm, Duration::from_secs(30)));
        assert_eq!(
            empty.alarm_page_actions(),
            vec![Message::NewAlarm, Message::NewAlarm]
        );
    }

    #[test]
    fn the_timers_page_lists_the_presets_between_the_header_and_the_cards() {
        let mut app = app_with(vec![], vec![], vec![]);
        app.timers.add("Eggs", Duration::from_secs(600));

        let actions = app.timer_page_actions();
        assert_eq!(actions[0], Message::NewTimer);
        let presets = app.timers.presets().len();
        assert_eq!(app.timer_row_base(), 1 + presets);
        for (index, preset) in app.timers.presets().iter().enumerate() {
            assert_eq!(actions[1 + index], Message::StartPreset(preset.id));
        }
        let id = app.timers.timers()[0].id;
        assert_eq!(actions[app.timer_row_base()], Message::ToggleTimer(id));
        assert_eq!(actions.len(), 1 + presets + 3);
    }

    #[test]
    fn the_stopwatch_page_lists_the_three_buttons_and_every_lap() {
        let mut app = app_with(vec![], vec![], vec![]);
        on_page(&mut app, 4);
        assert_eq!(
            app.stopwatch_page_actions(),
            vec![
                Message::ToggleStopwatch,
                Message::LapStopwatch,
                Message::ResetStopwatch
            ]
        );

        // Three marks over three runs, so each lap is a known instant.
        for run in 1..=3u64 {
            app.stopwatch.start(Duration::from_secs(run));
            app.stopwatch
                .lap(Duration::from_secs(run) + Duration::from_millis(500));
            app.stopwatch
                .pause(Duration::from_secs(run) + Duration::from_millis(500));
        }
        assert_eq!(app.stopwatch.lap_count(), 3);

        // The lap rows are drawn newest first, and each row's remove button must
        // remove the lap *that row is showing* — which is the one recorded last,
        // not the one recorded first.
        let actions = app.stopwatch_page_actions();
        assert_eq!(actions.len(), 3 + 3);
        assert_eq!(app.lap_row_base(), 3);
        for position in 0..3 {
            assert_eq!(
                actions[3 + position],
                Message::RemoveLap(2 - position),
                "the top row removes the newest lap"
            );
        }
    }

    #[test]
    fn nothing_in_the_list_is_navigation_the_page_did_not_draw() {
        let mut app = app_with_alarms(
            vec![crate::domain::alarm::Alarm::new(1, 6, 0)],
            app_with_timers(),
        );

        for page in 0..5 {
            on_page(&mut app, page);
            let shell = shell_targets(&app).len();
            for (index, message) in app.focus_targets().into_iter().enumerate() {
                if index < shell {
                    continue;
                }
                assert!(
                    !matches!(message, Message::Navigate(_)),
                    "page {page}: a page must not list navigation as one of its own controls"
                );
            }
        }
    }

    #[test]
    fn a_modal_takes_the_whole_list() {
        let mut app = app_with_alarms(vec![], app_with_timers());
        on_page(&mut app, 2);

        app.view.alarm_draft = Some(crate::app::view::AlarmDraft::new(1, 7, 0));
        assert_eq!(
            app.focus_targets(),
            vec![
                Message::AlarmHourStep(1),
                Message::AlarmMinuteStep(1),
                Message::SaveAlarm
            ]
        );

        app.view.alarm_draft = None;
        app.view.timer_draft = Some(crate::app::view::TimerDraft::new(
            "Tea",
            Duration::from_secs(60),
        ));
        assert_eq!(
            app.focus_targets(),
            vec![
                Message::TimerDurationStep(60),
                Message::TimerDurationStep(5),
                Message::ConfirmTimer
            ],
            "each stepper raises its own step, which is the half a key can mean"
        );

        app.view.timer_draft = None;
        app.view.pending_delete = Some(4);
        assert_eq!(
            app.focus_targets(),
            vec![Message::ConfirmDeleteAlarm, Message::CancelDeleteAlarm]
        );

        app.view.pending_delete = None;
        app.view.city_picker = true;
        assert_eq!(app.focus_targets(), vec![Message::ToggleCityPicker]);
    }

    #[test]
    fn a_page_control_maps_to_the_index_the_key_activates() {
        let alarm = crate::domain::alarm::Alarm::new(3, 6, 0);
        let mut app = app_with_alarms(vec![alarm], app_with_timers());
        on_page(&mut app, 2);

        let index = app.page_focus(app.alarm_row_base()).expect("a row exists");
        assert_eq!(app.focus_at(index), Some(Message::ToggleAlarm(3)));
        assert!(app.view.focus != Some(index));
        app.view.focus = Some(index);
        assert!(!app.shell_focused(index), "a page index is not a shell one");
    }

    #[test]
    fn an_action_with_no_control_anywhere_is_not_in_the_list() {
        // Saving a preset and clearing the laps are both things the application
        // can do, and neither has a control on any page: the timer editor has
        // no save button, and the lap list has no clear button. Listing either
        // would hand the keyboard a target that does something nobody could
        // have clicked.
        let mut app = app_with(vec![], vec![], vec![]);
        on_page(&mut app, 3);
        assert!(!app.focus_targets().contains(&Message::SaveTimerPreset));

        on_page(&mut app, 4);
        assert!(!app.focus_targets().contains(&Message::ClearLaps));
    }

    #[test]
    fn an_index_past_the_page_is_not_mapped() {
        let mut app = app_with(vec![], vec![], vec![]);
        on_page(&mut app, 0);
        assert_eq!(app.page_focus(3), None, "there is no fourth chip");
        assert!(app.focus_at(app.focus_count()).is_none());
    }
}
