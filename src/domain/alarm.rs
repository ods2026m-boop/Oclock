//! Alarms: the model, the schedule, and the firing engine.
//!
//! # Scheduling model
//!
//! An alarm is a *wall-clock* intention: "07:30 on Mondays". It is stored as
//! a wall time of day plus a repeat rule and is never stored as a duration or
//! a countdown. Each alarm therefore has a **pending occurrence**: the next
//! absolute instant it should ring at, computed in the system timezone with
//! [`crate::core::tz::resolve_wall_time`] so daylight-saving gaps and
//! overlaps get a defined answer.
//!
//! The engine is a small state machine driven by wall-clock samples:
//!
//! * [`AlarmSet::refresh`] fills in a missing pending occurrence.
//! * [`AlarmSet::poll`] reports occurrences that have arrived and consumes
//!   them, so each occurrence can only ever fire once.
//! * [`AlarmSet::reschedule`] throws pending occurrences away and recomputes
//!   them, which is what a timezone change requires.
//!
//! All of it is pure with respect to the clock: pass in `now` and get events
//! out. That is what makes it testable without sleeping.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

use crate::core::tz::{self, Zoned};
use crate::domain::sound::Sound;

/// How far past its occurrence an alarm may be and still count as ringing
/// rather than missed. Covers a suspended machine or a momentarily stalled
/// process; anything longer is reported to the user instead of surprising
/// them with an alarm hours late.
pub const RINGING_GRACE: Duration = Duration::from_secs(10 * 60);

/// The set of weekdays an alarm can repeat on, stored as a bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Weekdays {
    bits: u8,
}

impl Weekdays {
    /// No days.
    pub const NONE: Weekdays = Weekdays { bits: 0 };
    /// Every day.
    pub const ALL: Weekdays = Weekdays { bits: 0b0111_1111 };
    /// Monday through Friday.
    pub const WORKDAYS: Weekdays = Weekdays { bits: 0b0001_1111 };

    fn bit(day: Weekday) -> u8 {
        1 << (day.number_from_monday() - 1)
    }

    /// Builds a set from an iterator of weekdays.
    pub fn from_days(days: impl IntoIterator<Item = Weekday>) -> Weekdays {
        let mut set = Weekdays::NONE;
        for day in days {
            set = set.with(day, true);
        }
        set
    }

    /// Adds or removes a day.
    pub fn with(self, day: Weekday, included: bool) -> Weekdays {
        if included {
            Weekdays {
                bits: self.bits | Weekdays::bit(day),
            }
        } else {
            Weekdays {
                bits: self.bits & !Weekdays::bit(day),
            }
        }
    }

    /// Whether a day is in the set.
    pub fn contains(self, day: Weekday) -> bool {
        self.bits & Weekdays::bit(day) != 0
    }

    /// Whether the set is empty.
    pub fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// How many days are in the set.
    pub fn len(self) -> u32 {
        self.bits.count_ones()
    }

    /// The days, Monday first — the order they are displayed in.
    pub fn days(self) -> Vec<Weekday> {
        [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .into_iter()
        .filter(|day| self.contains(*day))
        .collect()
    }
}

/// How often an alarm repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    /// Rings once, then disarms itself.
    #[default]
    Once,
    /// Rings every day.
    Daily,
    /// Rings on the selected weekdays.
    Weekly(Weekdays),
}

impl Repeat {}

/// A single alarm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Alarm {
    /// Stable identity, used by the UI and by notification callbacks.
    pub id: u64,
    /// User-facing name.
    pub label: String,
    /// Hour of day, `0..=23`.
    pub hour: u32,
    /// Minute of hour, `0..=59`.
    pub minute: u32,
    /// Whether the alarm is armed.
    pub enabled: bool,
    /// The repeat rule.
    pub repeat: Repeat,
    /// Which sound to play.
    pub sound: Sound,
    /// Snooze length in minutes.
    pub snooze_minutes: u8,
    /// Unix timestamp of the occurrence that most recently rang, so an
    /// occurrence can never fire twice.
    #[serde(skip)]
    pub last_fired: Option<i64>,
    /// Set once a one-time alarm has rung; such an alarm is spent.
    #[serde(skip)]
    pub spent: bool,
}

impl Default for Alarm {
    fn default() -> Self {
        Alarm {
            id: 0,
            label: "Alarm".to_string(),
            hour: 7,
            minute: 0,
            enabled: true,
            repeat: Repeat::Once,
            sound: Sound::default(),
            snooze_minutes: 9,
            last_fired: None,
            spent: false,
        }
    }
}

impl Alarm {
    /// A new alarm at `hour:minute`.
    pub fn new(id: u64, hour: u32, minute: u32) -> Alarm {
        Alarm {
            id,
            hour,
            minute,
            ..Alarm::default()
        }
    }

    /// `07:30` in 24-hour form, the canonical machine-readable time.
    pub fn canonical_time(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// The occurrence on `date`, or `None` when the repeat rule skips it.
    pub fn occurrence_on(&self, zone: chrono_tz::Tz, date: NaiveDate) -> Option<Zoned> {
        match self.repeat {
            Repeat::Once | Repeat::Daily => {
                tz::resolve_wall_time(zone, date, self.hour, self.minute)
            }
            Repeat::Weekly(days) if days.contains(date.weekday()) => {
                tz::resolve_wall_time(zone, date, self.hour, self.minute)
            }
            Repeat::Weekly(_) => None,
        }
    }

    /// The next instant this alarm should ring, strictly after `after`.
    ///
    /// Returns `None` once a one-time alarm has fired, or when a weekly rule
    /// selects no days at all.
    pub fn next_after(&self, zone: chrono_tz::Tz, after: Zoned) -> Option<Zoned> {
        if self.spent {
            return None;
        }
        if let Repeat::Weekly(days) = self.repeat {
            if days.is_empty() {
                return None;
            }
        }

        // Eight candidate days covers every weekly rule: today's may already
        // have passed, and the same weekday next week is the last candidate.
        for offset in 0..=7i64 {
            let Some(date) = after
                .date_naive()
                .checked_add_signed(ChronoDuration::days(offset))
            else {
                continue;
            };
            let Some(occurrence) = self.occurrence_on(zone, date) else {
                continue;
            };
            if occurrence <= after {
                continue;
            }
            if self.last_fired == Some(occurrence.timestamp()) {
                continue;
            }
            return Some(occurrence);
        }

        None
    }

    /// Whether the alarm can still ring at all.
    pub fn is_armed(&self) -> bool {
        self.enabled && !self.spent
    }
}

/// A pending occurrence, and whether it came from a snooze.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    /// When the alarm rings.
    pub at: Zoned,
    /// True when the user pressed snooze rather than this being the original
    /// scheduled time.
    pub snoozed: bool,
}

/// Something the schedule did.
#[derive(Clone, Debug, PartialEq)]
pub enum AlarmEvent {
    /// The alarm rang.
    Ringing {
        alarm: Alarm,
        at: Zoned,
        late_by: Duration,
    },
    /// The occurrence passed more than [`RINGING_GRACE`] ago, so it is
    /// reported rather than rung — for example after a long suspend.
    Missed {
        alarm: Alarm,
        at: Zoned,
        late_by: Duration,
    },
}

impl AlarmEvent {
    /// The alarm the event concerns.
    pub fn alarm(&self) -> &Alarm {
        match self {
            AlarmEvent::Ringing { alarm, .. } | AlarmEvent::Missed { alarm, .. } => alarm,
        }
    }

    /// When the alarm was due.
    pub fn at(&self) -> Zoned {
        match self {
            AlarmEvent::Ringing { at, .. } | AlarmEvent::Missed { at, .. } => *at,
        }
    }
}

/// The alarm collection plus its runtime schedule.
#[derive(Clone, Debug, Default)]
pub struct AlarmSet {
    alarms: Vec<Alarm>,
    pending: BTreeMap<u64, Pending>,
}

impl AlarmSet {
    /// Adopts a restored set of alarms, clearing any runtime state.
    pub fn new(alarms: Vec<Alarm>) -> AlarmSet {
        AlarmSet {
            alarms,
            pending: BTreeMap::new(),
        }
    }

    /// The alarms, in creation order.
    pub fn alarms(&self) -> &[Alarm] {
        &self.alarms
    }

    /// How many alarms exist.
    pub fn len(&self) -> usize {
        self.alarms.len()
    }

    /// True when there are no alarms.
    pub fn is_empty(&self) -> bool {
        self.alarms.is_empty()
    }

    /// Looks an alarm up.
    pub fn get(&self, id: u64) -> Option<&Alarm> {
        self.alarms.iter().find(|alarm| alarm.id == id)
    }

    /// Mutable access to one alarm.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Alarm> {
        self.alarms.iter_mut().find(|alarm| alarm.id == id)
    }

    /// Inserts a new alarm, or replaces the one with the same id.
    pub fn upsert(&mut self, alarm: Alarm) {
        match self
            .alarms
            .iter_mut()
            .find(|existing| existing.id == alarm.id)
        {
            Some(existing) => *existing = alarm,
            None => self.alarms.push(alarm),
        }
    }

    /// Removes an alarm and forgets its schedule.
    pub fn remove(&mut self, id: u64) -> bool {
        self.pending.remove(&id);
        let before = self.alarms.len();
        self.alarms.retain(|alarm| alarm.id != id);
        self.alarms.len() != before
    }

    /// Arms or disarms an alarm, dropping its schedule if disarmed.
    pub fn set_enabled(&mut self, id: u64, enabled: bool) {
        if let Some(alarm) = self.get_mut(id) {
            alarm.enabled = enabled;
        }
        if !enabled {
            self.pending.remove(&id);
        }
    }

    /// The pending occurrence of an alarm, if one is scheduled.
    pub fn pending(&self, id: u64) -> Option<Pending> {
        self.pending.get(&id).copied()
    }

    /// How many alarms are ringing or due.
    pub fn scheduled_count(&self) -> usize {
        self.pending.len()
    }

    /// Fills in a pending occurrence for every armed alarm that lacks one.
    pub fn refresh(&mut self, zone: chrono_tz::Tz, now: Zoned) {
        self.prune();

        let mut wanted = Vec::new();
        for alarm in &self.alarms {
            if !alarm.is_armed() || self.pending.contains_key(&alarm.id) {
                continue;
            }
            if let Some(at) = alarm.next_after(zone, now) {
                wanted.push((alarm.id, Pending { at, snoozed: false }));
            }
        }

        for (id, pending) in wanted {
            self.pending.entry(id).or_insert(pending);
        }
    }

    /// Drops schedule entries that are no longer meaningful.
    fn prune(&mut self) {
        let live: Vec<u64> = self
            .alarms
            .iter()
            .filter(|alarm| alarm.is_armed())
            .map(|alarm| alarm.id)
            .collect();
        self.pending.retain(|id, _| live.contains(id));
    }

    /// Recomputes every schedule from scratch.
    ///
    /// Called when the system timezone changes: an alarm means a wall time,
    /// so its absolute instant must be recomputed against the new zone.
    pub fn reschedule(&mut self, zone: chrono_tz::Tz, now: Zoned) {
        let mut pending = std::mem::take(&mut self.pending);

        // A snooze is a promise about elapsed time, not a wall time, so it
        // survives a timezone change; a normal occurrence does not.
        for alarm in self.alarms.iter_mut() {
            if let Some(previous) = pending.get(&alarm.id) {
                if previous.snoozed {
                    continue;
                }
            }
            alarm.last_fired = None;
        }

        pending.retain(|id, entry| {
            self.get(*id)
                .is_some_and(|alarm| alarm.is_armed() && entry.snoozed)
        });

        self.pending = pending;
        self.refresh(zone, now);
    }

    /// Collects occurrences that have arrived, consuming each exactly once.
    pub fn poll(&mut self, now: Zoned) -> Vec<AlarmEvent> {
        let mut due: Vec<(u64, Zoned)> = self
            .pending
            .iter()
            .filter(|(_, entry)| entry.at <= now)
            .map(|(id, entry)| (*id, entry.at))
            .collect();

        due.sort_by_key(|(_, at)| *at);
        let mut events = Vec::with_capacity(due.len());

        for (id, at) in due {
            self.pending.remove(&id);

            let Some(alarm) = self.get_mut(id) else {
                continue;
            };

            alarm.last_fired = Some(at.timestamp());

            let late_by = (now - at).to_std().unwrap_or(Duration::ZERO);

            if matches!(alarm.repeat, Repeat::Once) {
                // A one-time alarm is spent the moment it rings, whether or
                // not the user was there to hear it.
                alarm.spent = true;
                alarm.enabled = false;
            }

            let alarm = alarm.clone();
            if late_by <= RINGING_GRACE {
                events.push(AlarmEvent::Ringing { alarm, at, late_by });
            } else {
                events.push(AlarmEvent::Missed { alarm, at, late_by });
            }
        }

        events
    }

    /// Postpones an alarm by `minutes`, or by its own configured snooze
    /// length when `minutes` is `None`.
    pub fn snooze(&mut self, id: u64, now: Zoned, minutes: Option<u8>) -> bool {
        let Some(alarm) = self.get(id) else {
            return false;
        };
        if !alarm.enabled || alarm.spent {
            return false;
        }

        let minutes = minutes.unwrap_or(alarm.snooze_minutes).max(1) as i64;
        let at = now + ChronoDuration::minutes(minutes);

        self.pending.insert(id, Pending { at, snoozed: true });
        true
    }

    /// Stops a ringing alarm without re-arming it: the next occurrence is
    /// recomputed from the wall clock, so a daily alarm resumes tomorrow.
    pub fn dismiss(&mut self, id: u64) -> bool {
        self.pending.remove(&id).is_some()
    }

    /// The soonest armed occurrence across all alarms.
    pub fn next_up(&self) -> Option<Zoned> {
        self.pending.values().map(|entry| entry.at).min()
    }

    /// Alarms ordered by when they next ring, for the "up next" panel.
    pub fn by_urgency(&self) -> Vec<(u64, Zoned)> {
        let mut entries: Vec<(u64, Zoned)> =
            self.pending.iter().map(|(id, p)| (*id, p.at)).collect();
        entries.sort_by_key(|(_, at)| *at);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;
    use chrono_tz::Tz;

    fn istanbul() -> Tz {
        "Europe/Istanbul".parse().expect("known zone")
    }

    fn at(zone: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Zoned {
        tz::resolve_wall_time_parts(zone, y, mo, d, h, mi).expect("resolvable")
    }

    fn alarm(id: u64, hour: u32, minute: u32, repeat: Repeat) -> Alarm {
        Alarm {
            id,
            hour,
            minute,
            repeat,
            enabled: true,
            ..Alarm::default()
        }
    }

    #[test]
    fn weekdays_round_trip_days() {
        let days = Weekdays::from_days([Weekday::Mon, Weekday::Wed, Weekday::Fri]);
        assert_eq!(days.len(), 3);
        assert!(days.contains(Weekday::Mon));
        assert!(!days.contains(Weekday::Tue));
        assert_eq!(days.days(), vec![Weekday::Mon, Weekday::Wed, Weekday::Fri]);
    }

    #[test]
    fn removing_a_day_works() {
        let days = Weekdays::ALL.with(Weekday::Wed, false);
        assert!(!days.contains(Weekday::Wed));
        assert_eq!(days.len(), 6);
    }

    #[test]
    fn one_time_alarm_fires_once() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Once)]);

        // The set is created after the alarm's time today.
        let now = at(zone, 2026, 6, 15, 8, 0);
        set.refresh(zone, now);

        let pending = set.pending(1).expect("scheduled");
        assert_eq!(pending.at, at(zone, 2026, 6, 16, 7, 0));
        assert!(!pending.snoozed);
    }

    #[test]
    fn one_time_alarm_schedules_today_when_still_ahead() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 9, 0, Repeat::Once)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 8, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 15, 9, 0));
    }

    #[test]
    fn a_fired_one_time_alarm_is_spent() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 9, 0, Repeat::Once)]);
        let now = at(zone, 2026, 6, 15, 8, 0);
        set.refresh(zone, now);

        let due = at(zone, 2026, 6, 15, 9, 0);
        let events = set.poll(due);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AlarmEvent::Ringing { .. }));
        assert!(!set.get(1).unwrap().is_armed());

        // It must never ring again, and re-polling yields nothing.
        set.refresh(istanbul(), due);
        assert!(set.pending(1).is_none());
        assert!(set.poll(due + ChronoDuration::days(1)).is_empty());
    }

    #[test]
    fn daily_alarm_advances_every_day() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 6, 30, Repeat::Daily)]);

        let first = at(zone, 2026, 6, 15, 7, 0);
        set.refresh(zone, first);
        let first_due = set.pending(1).unwrap().at;
        assert_eq!(first_due, at(zone, 2026, 6, 16, 6, 30));

        set.poll(first_due);
        let second = at(zone, 2026, 6, 16, 7, 0);
        set.refresh(zone, second);
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 17, 6, 30));
    }

    #[test]
    fn weekly_alarm_selects_only_its_days() {
        // 2026-06-15 is a Monday.
        let zone = istanbul();
        let days = Weekdays::from_days([Weekday::Wed, Weekday::Sun]);
        let mut set = AlarmSet::new(vec![alarm(1, 5, 0, Repeat::Weekly(days))]);

        set.refresh(zone, at(zone, 2026, 6, 15, 6, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 17, 5, 0));
    }

    #[test]
    fn weekly_alarm_wraps_to_next_week_after_firing() {
        let zone = istanbul();
        let days = Weekdays::from_days([Weekday::Mon]);
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Weekly(days))]);

        let monday = at(zone, 2026, 6, 15, 0, 1);
        set.refresh(zone, monday);
        let due = set.pending(1).unwrap().at;
        assert_eq!(due, at(zone, 2026, 6, 15, 7, 0));

        set.poll(due);
        set.refresh(zone, due);
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 22, 7, 0));
    }

    #[test]
    fn a_weekday_set_is_independent_of_each_alarm() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![
            alarm(1, 7, 0, Repeat::Weekly(Weekdays::from_days([Weekday::Mon]))),
            alarm(2, 7, 0, Repeat::Weekly(Weekdays::from_days([Weekday::Fri]))),
        ]);

        set.refresh(zone, at(zone, 2026, 6, 15, 6, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 15, 7, 0));
        assert_eq!(set.pending(2).unwrap().at, at(zone, 2026, 6, 19, 7, 0));
    }

    #[test]
    fn multiple_alarms_all_fire() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![
            alarm(1, 7, 0, Repeat::Daily),
            alarm(2, 7, 30, Repeat::Daily),
            alarm(3, 9, 0, Repeat::Once),
        ]);

        let now = at(zone, 2026, 6, 15, 0, 0);
        set.refresh(zone, now);
        assert_eq!(set.scheduled_count(), 3);

        let after = at(zone, 2026, 6, 15, 10, 0);
        let events = set.poll(after);
        assert_eq!(events.len(), 3);
        // Ordered by when they were due.
        let order: Vec<u64> = events.iter().map(|event| event.alarm().id).collect();
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn a_disabled_alarm_never_schedules_or_fires() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![Alarm {
            enabled: false,
            ..alarm(1, 7, 0, Repeat::Daily)
        }]);

        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));
        assert!(set.pending(1).is_none());
        assert!(set.poll(at(zone, 2026, 6, 15, 8, 0)).is_empty());
    }

    #[test]
    fn re_arming_a_disabled_alarm_schedules_it_again() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![Alarm {
            enabled: false,
            ..alarm(1, 7, 0, Repeat::Daily)
        }]);

        let now = at(zone, 2026, 6, 15, 6, 0);
        set.refresh(zone, now);
        assert!(set.pending(1).is_none());

        set.set_enabled(1, true);
        set.refresh(zone, now);
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 15, 7, 0));
    }

    #[test]
    fn snooze_postpones_and_is_marked_as_such() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![Alarm {
            snooze_minutes: 7,
            ..alarm(1, 7, 0, Repeat::Daily)
        }]);

        let now = at(zone, 2026, 6, 15, 6, 0);
        set.refresh(zone, now);
        let due = set.pending(1).unwrap().at;
        set.poll(due);

        assert!(set.snooze(1, due, None));
        let pending = set.pending(1).expect("rescheduled");
        assert!(pending.snoozed);
        assert_eq!(pending.at, due + ChronoDuration::minutes(7));

        // The snoozed alarm rings at the new time, not the original one.
        let events = set.poll(pending.at);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].at(), pending.at);
        // Polling consumed it; the next daily occurrence is scheduled after.
        assert!(set.pending(1).is_none());
        set.refresh(zone, pending.at);
        let next = set.pending(1).expect("rescheduled").at;
        assert!(next > pending.at);
        assert_eq!(
            (next.naive_local().hour(), next.naive_local().minute()),
            (7, 0),
            "the next daily occurrence, not another snooze"
        );
    }

    #[test]
    fn snooze_can_request_a_custom_length() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        let now = at(zone, 2026, 6, 15, 7, 0);

        assert!(set.snooze(1, now, Some(3)));
        assert_eq!(set.pending(1).unwrap().at, now + ChronoDuration::minutes(3));
    }

    #[test]
    fn snooze_of_zero_minutes_is_clamped_to_one() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        let now = at(zone, 2026, 6, 15, 7, 0);

        assert!(set.snooze(1, now, Some(0)));
        assert_eq!(set.pending(1).unwrap().at, now + ChronoDuration::minutes(1));
    }

    #[test]
    fn snoozing_an_unknown_or_spent_alarm_fails_cleanly() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![Alarm {
            repeat: Repeat::Daily,
            ..alarm(1, 7, 0, Repeat::Daily)
        }]);
        let now = at(zone, 2026, 6, 15, 7, 0);

        assert!(!set.snooze(404, now, None));

        set.get_mut(1).unwrap().spent = true;
        assert!(!set.snooze(1, now, None));
    }

    #[test]
    fn day_rollover_keeps_the_schedule_correct() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 0, 5, Repeat::Daily)]);

        // 23:50 on the 30th: the alarm belongs to the 31st.
        set.refresh(zone, at(zone, 2026, 6, 30, 23, 50));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 7, 1, 0, 5));

        // Past midnight, it is already due.
        set.poll(at(zone, 2026, 7, 1, 0, 5));
        set.refresh(zone, at(zone, 2026, 7, 1, 0, 6));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 7, 2, 0, 5));
    }

    #[test]
    fn year_rollover_is_handled() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 0, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 12, 31, 23, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2027, 1, 1, 0, 0));
    }

    #[test]
    fn a_weekly_alarm_crossing_a_dst_boundary_keeps_its_wall_time() {
        // Europe/Berlin springs forward on 2026-03-29.
        let zone: Tz = "Europe/Berlin".parse().unwrap();
        let days = Weekdays::from_days([Weekday::Sun]);
        // 01:00 local: the transition happens later that same morning, so the
        // first occurrence is in standard time and the second in summer time.
        let mut set = AlarmSet::new(vec![alarm(1, 1, 0, Repeat::Weekly(days))]);

        set.refresh(zone, at(zone, 2026, 3, 28, 12, 0));
        let next = set.pending(1).unwrap().at;
        assert_eq!(
            (next.naive_local().hour(), next.naive_local().minute()),
            (1, 0)
        );
        assert_eq!(tz::utc_offset_at(zone, next), 3600, "still CET");

        set.poll(next);
        set.refresh(zone, next);
        let after = set.pending(1).unwrap().at;
        assert_eq!(
            (after.naive_local().hour(), after.naive_local().minute()),
            (1, 0)
        );
        assert_eq!(
            tz::utc_offset_at(zone, after),
            tz::utc_offset_at(zone, next) + 3600,
            "the offset should have changed but the wall time should not"
        );
    }

    #[test]
    fn a_timezone_change_recomputes_absolute_occurrences() {
        let zone_before: Tz = "Europe/London".parse().unwrap();
        let zone_after: Tz = "Asia/Tokyo".parse().unwrap();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);

        set.refresh(zone_before, at(zone_before, 2026, 6, 15, 0, 0));
        let before = set.pending(1).unwrap().at;
        assert_eq!(tz::utc_offset_at(zone_before, before), 3600);

        // Same wall clock reading, different zone: a different instant.
        set.reschedule(zone_after, at(zone_after, 2026, 6, 15, 0, 0));
        let after = set.pending(1).unwrap().at;
        assert_eq!(tz::utc_offset_at(zone_after, after), 9 * 3600);
        assert_ne!(before.timestamp(), after.timestamp());
    }

    #[test]
    fn a_timezone_change_preserves_a_pending_snooze() {
        let zone_before: Tz = "Europe/London".parse().unwrap();
        let zone_after: Tz = "Asia/Tokyo".parse().unwrap();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);

        let now = at(zone_before, 2026, 6, 15, 7, 0);
        set.refresh(zone_before, now);
        assert!(set.snooze(1, now, Some(10)));
        let snoozed = set.pending(1).unwrap().at;

        set.reschedule(zone_after, now.with_timezone(&zone_after));
        // A snooze is a promise about elapsed time and must not move.
        assert_eq!(set.pending(1).unwrap().at.timestamp(), snoozed.timestamp());
        assert!(set.pending(1).unwrap().snoozed);
    }

    #[test]
    fn an_alarm_ringing_across_a_dst_gap_still_rings() {
        // 02:30 does not exist on the European spring-forward date; the
        // scheduler resolves it to 03:30 local and rings there.
        let zone: Tz = "Europe/Berlin".parse().unwrap();
        let mut set = AlarmSet::new(vec![alarm(1, 2, 30, Repeat::Daily)]);

        set.refresh(zone, at(zone, 2026, 3, 28, 12, 0));
        let next = set.pending(1).unwrap().at;
        assert_eq!(next.naive_local().hour(), 3);
        assert_eq!(next.naive_local().minute(), 30);

        assert_eq!(set.poll(next).len(), 1);
    }

    #[test]
    fn a_long_suspend_produces_a_missed_alarm_not_a_late_ring() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));

        // The machine sleeps and wakes three hours after the alarm.
        let woke = at(zone, 2026, 6, 15, 10, 0);
        let events = set.poll(woke);
        assert_eq!(events.len(), 1);
        match &events[0] {
            AlarmEvent::Missed { late_by, .. } => assert!(*late_by > RINGING_GRACE),
            other => panic!("expected a missed alarm, got {other:?}"),
        }
    }

    #[test]
    fn a_short_stall_still_rings() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));

        let woke = at(zone, 2026, 6, 15, 7, 1);
        let events = set.poll(woke);
        assert!(matches!(events[0], AlarmEvent::Ringing { .. }));
    }

    #[test]
    fn polling_consumes_each_occurrence_exactly_once() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));

        let now = at(zone, 2026, 6, 15, 7, 0);
        assert_eq!(set.poll(now).len(), 1);
        assert!(set.poll(now).is_empty());
        assert!(set.poll(now + ChronoDuration::hours(5)).is_empty());
    }

    #[test]
    fn editing_an_alarm_reschedules_it() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 15, 7, 0));

        let edited = Alarm {
            hour: 8,
            minute: 45,
            ..set.get(1).unwrap().clone()
        };
        set.upsert(edited);
        // The old pending entry is stale; refresh keeps it, so the caller
        // reschedules explicitly, which is what the app does on save.
        set.reschedule(zone, at(zone, 2026, 6, 15, 0, 0));
        assert_eq!(set.pending(1).unwrap().at, at(zone, 2026, 6, 15, 8, 45));
    }

    #[test]
    fn removing_an_alarm_clears_its_schedule() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Daily)]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));
        assert_eq!(set.scheduled_count(), 1);

        assert!(set.remove(1));
        assert_eq!(set.scheduled_count(), 0);
        assert!(set.poll(at(zone, 2026, 6, 15, 8, 0)).is_empty());
        assert!(!set.remove(1), "removing twice is a no-op");
    }

    #[test]
    fn next_up_and_urgency_order_across_alarms() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![
            alarm(1, 12, 0, Repeat::Daily),
            alarm(2, 6, 0, Repeat::Daily),
            alarm(3, 9, 0, Repeat::Daily),
        ]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));

        assert_eq!(set.next_up().unwrap(), at(zone, 2026, 6, 15, 6, 0));
        let order: Vec<u64> = set.by_urgency().iter().map(|(id, _)| *id).collect();
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn an_empty_weekly_rule_never_schedules() {
        let zone = istanbul();
        let mut set = AlarmSet::new(vec![alarm(1, 7, 0, Repeat::Weekly(Weekdays::NONE))]);
        set.refresh(zone, at(zone, 2026, 6, 15, 0, 0));
        assert!(set.pending(1).is_none());
    }

    #[test]
    fn alarms_round_trip_through_json_without_runtime_state() {
        let a = Alarm {
            id: 12,
            label: "Gym".to_string(),
            hour: 6,
            minute: 15,
            enabled: true,
            repeat: Repeat::Weekly(Weekdays::from_days([Weekday::Tue, Weekday::Thu])),
            sound: Sound::Pulse,
            snooze_minutes: 5,
            last_fired: Some(99),
            spent: true,
        };
        let json = serde_json::to_string(&a).expect("serializes");
        let restored: Alarm = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(restored.id, 12);
        assert_eq!(restored.repeat, a.repeat);
        assert_eq!(restored.sound, Sound::Pulse);
        // Runtime-only fields are not persisted: a restored alarm is live.
        assert_eq!(restored.last_fired, None);
        assert!(!restored.spent);
    }

    #[test]
    fn a_partial_alarm_json_fills_in_defaults() {
        let restored: Alarm =
            serde_json::from_str(r#"{"id":3,"hour":5,"minute":45}"#).expect("deserializes");
        assert_eq!(restored.hour, 5);
        assert_eq!(restored.minute, 45);
        assert_eq!(restored.repeat, Repeat::Once);
        assert_eq!(restored.sound, Sound::default());
        assert_eq!(restored.snooze_minutes, 9);
        assert!(restored.enabled);
    }
}
