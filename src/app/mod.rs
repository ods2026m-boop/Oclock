//! The OClock application.
//!
//! This module owns the state and decides what each message does. The five
//! feature engines it coordinates live in [`crate::domain`]; the services it
//! drives live in [`crate::services`]; and the view is a projection of this
//! state plus the snapshot taken on the last tick.
//!
//! # Timing
//!
//! Two clocks, kept strictly apart:
//!
//! * a **wall clock**, sampled once per second, which drives the display, the
//!   alarm schedule and the world clock. Anything a human would call "now".
//! * a **monotonic clock**, sampled every frame, which drives the stopwatch,
//!   the timers and every animation. Anything a human would call "elapsed".
//!
//! The snapshot ([`Snapshot`]) is built once per second rather than per frame,
//! which is what keeps a 60 Hz window from reformatting the date sixty times a
//! second for a string that changes once a minute.

pub mod focus;
pub mod icon;
pub mod message;
pub mod view;

use std::time::{Duration, Instant};

use iced::futures;
use iced::{keyboard, time, window};
use iced::{Element, Subscription, Task};

use crate::app::message::{Key, Message};
use crate::app::view::{AlarmDraft, Layout, Notice, Ringing, TimerDraft, View};
use crate::core::ids::Id;
use crate::core::time::{Discontinuity, SystemTimeline, Watchdog};
use crate::core::tz::{self, TimeZoneMonitor, Zoned};
use crate::design::motion::{FrameClock, Presence, Spec, Tween};
use crate::design::palette::Palette;
use crate::design::theme::{Appearance, ThemeMode, ThemeState};
use crate::design::tokens::{layout, motion};
use crate::domain::alarm::{Alarm, AlarmEvent, AlarmSet, RINGING_GRACE};
use crate::domain::clock::{ClockDisplay, ClockSnapshot};
use crate::domain::sound::Sound;
use crate::domain::stopwatch::Stopwatch;
use crate::domain::timer::TimerSet;
use crate::domain::world::WorldClocks;
use crate::services::audio::{ScriptedPlayer, SilentPlayer, SoundPlayer, SystemPlayer};
use crate::services::i18n::{Arg, Catalog, Language, Text};
use crate::services::notify::{self, Actions, Chosen, Notifier};
use crate::services::settings::{Config, SoundSettings};
use crate::services::storage::{Loaded, Store};
use crate::services::tzdata;
use crate::ui::components::interaction::Interactions;
use crate::ui::components::navigation::Navigation;

/// Which sound player the application is using.
///
/// Two implementations exist so a machine with no audio hardware — and a test
/// run, which has none — behave identically as far as the state machine is
/// concerned.
pub enum Player {
    /// The real output device.
    System(SystemPlayer),
    /// No output device; requests are dropped.
    Silent(SilentPlayer),
    /// A recorder, for tests.
    Scripted(ScriptedPlayer),
}

impl Player {
    /// A player that plays through the system.
    pub fn system() -> Player {
        Player::System(SystemPlayer::new())
    }

    /// A player that records requests.
    pub fn scripted() -> Player {
        Player::Scripted(ScriptedPlayer::default())
    }

    /// Plays a sound.
    pub fn play(&self, sound: Sound) {
        match self {
            Player::System(player) => player.play(sound),
            Player::Silent(player) => player.play(sound),
            Player::Scripted(player) => player.play(sound),
        }
    }

    /// Stops playback.
    pub fn stop(&self) {
        match self {
            Player::System(player) => player.stop(),
            Player::Silent(player) => player.stop(),
            Player::Scripted(player) => player.stop(),
        }
    }

    /// Turns playback on or off.
    pub fn set_enabled(&self, enabled: bool) {
        match self {
            Player::System(player) => player.set_enabled(enabled),
            Player::Silent(player) => player.set_enabled(enabled),
            Player::Scripted(player) => player.set_enabled(enabled),
        }
    }
}

/// Everything the clock page needs, captured once per second.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The local time.
    pub now: Zoned,
    /// The timezone it is expressed in.
    pub zone: chrono_tz::Tz,
    /// The formatted clock, the date and the calendar metadata.
    pub clock: ClockSnapshot,
    /// The world-clock rows, all measured from the same instant.
    pub world: Vec<crate::domain::world::WorldRow>,
}

/// The two ends of the notification action channel, kept together so a single
/// mutex guards both.
struct ActionChannel {
    /// Sends the messages the worker produced from a desktop notification.
    sender: futures::channel::mpsc::UnboundedSender<Message>,
    /// The receiving end, in an `Option` so the stream can be *taken* rather
    /// than shared: a `Stream` needs `&mut` to poll, and a stream that is
    /// still alive has to keep being able to say "there is nothing left to
    /// receive on".
    receiver: Option<futures::channel::mpsc::UnboundedReceiver<Message>>,
}

/// The notification action stream, sharing the application's channel.
///
/// A `Stream` rather than a subscription of polling: the runtime wakes this
/// task when a message arrives, and an application with nothing waiting on the
/// desktop sleeps. The lock is taken only for the duration of one poll, so the
/// worker is never blocked for longer than a poll takes.
///
/// A genuinely empty channel leaves the stream *pending*, which is the whole
/// difference from folding a `try_recv` into it: `try_recv` cannot wait, so a
/// stream built from one has to answer an empty channel with "finished", and a
/// finished stream is never polled again — the subscription would be dead from
/// the first frame, before a single notification action had arrived.
struct ActionStream {
    channel: std::sync::Arc<std::sync::Mutex<ActionChannel>>,
}

impl futures::Stream for ActionStream {
    type Item = Message;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Message>> {
        let Ok(mut channel) = self.channel.lock() else {
            // A poisoned channel is a channel nobody can trust; ending the
            // stream is the one thing that cannot make it worse.
            return std::task::Poll::Ready(None);
        };

        match channel.receiver.as_mut() {
            // The receiver registers the task's waker with the channel while it
            // is being polled, so the lock is not held across the wait.
            Some(receiver) => std::pin::Pin::new(receiver).poll_next(context),
            // Already taken: the stream has ended, and ending it twice is not a
            // thing that can go wrong.
            None => std::task::Poll::Ready(None),
        }
    }
}

/// How often the wall clock is sampled.
const SECOND: Duration = Duration::from_secs(1);

/// How often the desktop is asked what its light/dark preference is, while the
/// mode is *Match system*.
///
/// Slow on purpose: the answer is read by asking the desktop, and a desktop
/// preference changes a handful of times in a session rather than a hundred
/// times a minute. A minute is short enough that the window has already
/// repainted by the time it looks wrong, and long enough to be free.
const APPEARANCE_POLL: Duration = Duration::from_secs(60);

/// How long the application waits after a change before writing it out.
///
/// A debounce rather than a save per keystroke: a change is written once the
/// interface has settled, so dragging a stepper across its range writes once
/// at the end instead of twelve times on the way.
const AUTOSAVE: Duration = Duration::from_millis(900);

/// The application.
pub struct OClock {
    // --- design and view ---
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The appearance, and the transition between appearances.
    pub theme: ThemeState,
    /// Per-control interaction state.
    pub interactions: Interactions,
    /// What the interface is showing.
    pub view: View,
    /// The navigation rail's state, including the sliding indicator.
    pub navigation: Navigation,
    /// The page cross-fade.
    pub page_tween: Tween,
    /// The alarm editor's appearance.
    pub dialog: Presence,

    // --- time ---
    /// The monotonic origin.
    timeline: SystemTimeline,
    /// Divergence between the wall clock and the monotonic clock.
    watchdog: Watchdog,
    /// The system timezone, and a watcher for changes to it.
    zones: TimeZoneMonitor,
    /// The current wall-clock time.
    now: Zoned,
    /// The last per-second snapshot.
    pub snapshot: Snapshot,
    /// The delta the last tick carried.
    last_delta: Duration,
    /// The last instant the tick stream fired, so deltas are measured from a
    /// real clock rather than from how often a message happened to arrive.
    last_instant: Instant,
    /// How long since the last per-second sample.
    since_second: Duration,
    /// When the configuration was last written, on the monotonic clock.
    last_save: Duration,
    /// How long since the configuration was last marked changed.
    since_change: Duration,

    // --- features ---
    /// Alarms and their schedule.
    pub alarms: AlarmSet,
    /// Countdown timers and their presets.
    pub timers: TimerSet,
    /// The stopwatch.
    pub stopwatch: Stopwatch,
    /// The world clock.
    pub world: WorldClocks,

    // --- services ---
    store: Store,
    notifier: Box<dyn Notifier>,
    /// Both ends of the notification action channel: the worker sends, the
    /// subscription tree receives, and one mutex keeps them paired.
    action_channel: std::sync::Arc<std::sync::Mutex<ActionChannel>>,
    pub player: Player,
    /// The sound preferences, mirrored from the configuration.
    pub sound: SoundSettings,
    /// The theme mode, mirrored from the configuration.
    pub mode: ThemeMode,
    /// Whether the desktop's preference is being followed.
    pub follow_system: bool,
    /// The strings in use, rebuilt whenever the language changes.
    ///
    /// Held as a value rather than reached through `&self` at every label,
    /// because every view function needs it and threading it through each of
    /// them would put a translation concern in the middle of every signature.
    pub catalog: Catalog,
    /// The language in use, mirrored from the configuration.
    pub language: Language,
    /// 12- or 24-hour time, mirrored from the configuration.
    display_format: crate::domain::clock::HourFormat,
    /// Whether the seconds column is shown.
    show_seconds: bool,
    /// Whether the analog dial is shown.
    show_analog: bool,
    /// Whether resetting the stopwatch keeps its lap history.
    keep_laps: bool,
    /// The desktop's light/dark preference, as last read.
    system_dark: bool,
    /// Monotonic time when the preference was last read.
    appearance_read: Duration,
    /// How often it is read again.
    appearance_poll: Duration,
    /// Whether the configuration has unsaved changes.
    dirty: bool,
    /// Whether a save has been attempted at least once.
    saved_once: bool,

    /// The monotonic instant the application started, for a first-paint fade.
    started: Instant,
}

impl OClock {
    /// Builds the application from a configuration and a store.
    ///
    /// Split from [`Self::start`] so a test can construct the whole thing
    /// without touching a window, a clock or the desktop.
    pub fn with_config(
        config: Config,
        store: Store,
        notifier: Box<dyn Notifier>,
        actions: Actions,
        player: Player,
    ) -> OClock {
        // A notification's action arrives on a blocking channel from a worker
        // thread; the runtime wants a stream. One long-lived bridge thread
        // moves actions across, and the application's own [`ActionStream`]
        // reads from the far side of that bridge.
        //
        // The worker owns the receiving end outright: a `Receiver` can be
        // moved into a thread but not shared, and exactly one thread should be
        // reading it.
        let mut actions = actions;
        let source = actions.take_receiver();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        let action_channel = std::sync::Arc::new(std::sync::Mutex::new(ActionChannel {
            sender,
            receiver: Some(receiver),
        }));

        {
            let channel = std::sync::Arc::clone(&action_channel);
            std::thread::Builder::new()
                .name("oclock-notifications".to_string())
                .spawn(move || {
                    for chosen in source.iter() {
                        for message in on_action(chosen) {
                            let Ok(channel) = channel.lock() else {
                                return;
                            };
                            if channel.sender.unbounded_send(message).is_err() {
                                // The application has gone; nothing left to do.
                                return;
                            }
                        }
                    }
                })
                .ok();
        }

        let system_is_dark = crate::design::theme::system_is_dark();
        let zone = tz::system_timezone();
        let now = tz::now_in(zone);
        let clock = FrameClock::new();

        let mut view = View::new(Duration::ZERO);
        view.layout = Layout::for_size(layout::WINDOW.0 as f32, layout::WINDOW.1 as f32);

        let display = config.clock;
        let world = WorldClocks::new(config.world.locations, config.world.sort);
        let started = Instant::now();

        // The world rows are built by the same function the per-second resample
        // uses. Constructing them here by hand meant the first frame the window
        // ever drew carried placeholders — no daylight-saving flag, no
        // difference from the viewer's own zone, no date, every card reading
        // "Today" — and they were still on screen a second later, when the
        // resample replaced them with the real thing.
        let snapshot = Snapshot {
            now,
            zone,
            clock: ClockSnapshot::capture(now, zone, display),
            world: world.rows(now, zone),
        };

        let mut app = OClock {
            clock,
            theme: ThemeState::new(config.appearance.mode, system_is_dark),
            interactions: Interactions::new(),
            navigation: Navigation::new(Message::destinations().to_vec(), 0),
            page_tween: Tween::at(1.0),
            dialog: Presence::hidden(),

            timeline: SystemTimeline::new(),
            watchdog: Watchdog::new(&SystemTimeline::new()),
            zones: TimeZoneMonitor::new(),
            now,
            snapshot,
            last_delta: Duration::ZERO,
            last_instant: Instant::now(),
            since_second: Duration::ZERO,
            last_save: Duration::ZERO,
            since_change: AUTOSAVE,

            alarms: AlarmSet::new(config.alarms),
            timers: TimerSet::new(config.timers.presets),
            stopwatch: Stopwatch::new(),
            world,

            store,
            notifier,
            action_channel,
            player,
            catalog: Catalog::new(config.language),
            language: config.language,
            sound: config.sound,
            mode: config.appearance.mode,
            follow_system: config.appearance.mode.follows_system(),
            display_format: display.hour_format,
            show_seconds: display.show_seconds,
            show_analog: display.show_analog,
            keep_laps: config.timers.keep_laps,
            system_dark: system_is_dark,
            // The preference was read a moment ago, a few lines up, so the first
            // poll is a whole interval away rather than a tick away.
            appearance_read: started.elapsed(),
            appearance_poll: APPEARANCE_POLL,
            dirty: false,
            saved_once: false,

            view,
            started,
        };

        app.alarms.refresh(zone, now);
        app.player.set_enabled(app.sound.enabled);
        app
    }

    /// Builds the application the way the program does.
    pub fn start() -> OClock {
        let store = Store::in_dir(
            crate::core::tz::config_dir(crate::APP_SLUG).unwrap_or_else(std::env::temp_dir),
            "settings",
        )
        .unwrap_or_else(|_| Store::new("oclock.json"));

        let loaded = store.load_sections(Config::read);
        let fresh = loaded.is_fresh();
        let unreadable = matches!(loaded, Loaded::Failed { .. });
        let notices = loaded.notices();

        // A first launch starts from the built-in configuration — the world
        // clock a new user expects, and the timer presets — rather than from an
        // empty document. Anything the file did manage to say still wins.
        let config = if fresh {
            Config::first_run()
        } else {
            loaded.value()
        }
        .migrate()
        .sanitised();

        let actions = Actions::new();
        let notifier = notify::connect(crate::APP_NAME, &actions);
        let player = Player::system();

        let mut app = OClock::with_config(config, store, notifier, actions, player);

        if fresh {
            // A first run has nothing to migrate, but the built-in world clock
            // and the timer presets are still worth writing out, so that
            // editing the file is a supported way to configure OClock.
            app.dirty = true;
        }

        for notice in notices {
            app.view.notice = Some(Notice {
                headline: if unreadable {
                    app.tr(Text::NoticeUnreadable).to_string()
                } else {
                    app.tr(Text::NoticeRepaired).to_string()
                },
                detail: notice,
                tone: app.palette(),
                recoverable: true,
            });
        }

        app
    }

    /// A string in the current language.
    ///
    /// Shorthand for [`Catalog::text`], which is what a view almost always
    /// wants; the catalogue is a public field for the few callers that need
    /// something more than a lookup.
    pub fn tr(&self, key: Text) -> &'static str {
        self.catalog.text(key)
    }

    /// A string in the current language, with arguments substituted.
    pub fn tr_with(&self, key: Text, args: &[crate::services::i18n::Arg]) -> String {
        self.catalog.format(key, args)
    }

    /// Which way the interface is laid out.
    pub fn direction(&self) -> crate::services::i18n::Direction {
        self.catalog.direction()
    }

    /// Whether the interface is written right to left.
    pub fn is_rtl(&self) -> bool {
        self.catalog.is_rtl()
    }

    /// The palette currently in effect.
    pub fn palette(&self) -> Palette {
        self.theme.palette(&self.clock)
    }

    /// The current wall-clock time.
    pub fn now(&self) -> Zoned {
        self.now
    }

    /// The monotonic reading, for anything measuring elapsed time.
    ///
    /// Read straight from the monotonic clock rather than accumulated from
    /// tick deltas: a dropped or coalesced tick then costs nothing at all,
    /// because elapsed time was never being summed in the first place.
    pub fn elapsed(&self) -> Duration {
        use crate::core::time::Timeline;
        self.timeline.mono()
    }

    /// The clock page's preferences.
    pub fn display(&self) -> ClockDisplay {
        ClockDisplay {
            hour_format: self.display_format,
            show_seconds: self.show_seconds,
            show_analog: self.show_analog,
        }
    }

    /// The system timezone in effect.
    pub fn zone(&self) -> chrono_tz::Tz {
        self.snapshot.zone
    }

    /// Handles a message, returning any work the runtime should do.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                // The tick carries nothing, and needs nothing: the true delta
                // comes from the monotonic clock, not from the interval the
                // message happened to arrive on.
                let now = Instant::now();
                let delta = now.saturating_duration_since(self.last_instant);
                self.last_instant = now;
                self.tick(delta.min(Duration::from_secs(1)))
            }
            Message::Resized(width, height) => {
                // The window's size is kept, not just the layout it implies:
                // a dialog is sized against the window rather than against a
                // constant, which is what keeps it inside a small one.
                self.view.window = (width, height);
                let layout = Layout::for_size(width, height);
                if self.view.layout != layout {
                    self.view.layout = layout;
                }
                Task::none()
            }
            Message::Discontinuity(discontinuity) => {
                self.reconcile(discontinuity);
                Task::none()
            }
            Message::SystemAppearance(dark) => {
                if self.theme.follow_system(dark, &self.clock) {
                    self.mark_dirty();
                }
                Task::none()
            }

            Message::ChooseLanguage(language) => {
                self.set_language(language);
                Task::none()
            }
            Message::Navigate(index) => {
                if !self.view.has_modal() {
                    self.navigation.select(index, &self.clock);
                    self.view.go_to(self.navigation.selected());
                    self.start_page_transition();
                }
                Task::none()
            }
            Message::ToggleAppearance => {
                let next = self.mode.next();
                self.set_appearance(next);
                Task::none()
            }
            Message::ChooseAppearance(mode) => {
                self.set_appearance(mode);
                Task::none()
            }

            Message::FocusNext => {
                let count = self.focus_count();
                self.view.focus = Some(match self.view.focus {
                    Some(index) if count > 0 => (index + 1) % count,
                    Some(index) => index,
                    None => 0,
                });
                Task::none()
            }
            Message::FocusPrevious => {
                let count = self.focus_count();
                self.view.focus = Some(match self.view.focus {
                    Some(0) | None if count > 0 => count - 1,
                    Some(index) => index - 1,
                    None => 0,
                });
                Task::none()
            }
            Message::ActivateFocus => {
                let Some(index) = self.view.focus else {
                    return Task::none();
                };
                self.activate(index)
            }
            Message::Key(key) => self.on_key(key),
            Message::ToggleShortcuts => {
                self.view.shortcuts = !self.view.shortcuts;
                Task::none()
            }
            Message::Dismiss => {
                self.dismiss_topmost();
                Task::none()
            }

            Message::ToggleHourFormat => {
                self.display_format = self.display_format.toggled();
                self.refresh_snapshot();
                self.mark_dirty();
                Task::none()
            }
            Message::ToggleSeconds => {
                self.show_seconds = !self.show_seconds;
                self.refresh_snapshot();
                self.mark_dirty();
                Task::none()
            }
            Message::ToggleAnalog => {
                self.show_analog = !self.show_analog;
                self.mark_dirty();
                Task::none()
            }

            Message::SearchCities(query) => {
                self.view.typing = !query.is_empty();
                self.view.query = query;
                self.view.city_picker = true;
                Task::none()
            }
            Message::ToggleCityPicker => {
                if self.view.city_picker {
                    self.close_city_picker();
                } else {
                    self.view.city_picker = true;
                    self.open_dialog();
                }
                Task::none()
            }
            Message::AddCity(zone) => {
                self.add_city(&zone);
                // Adding is a choice about one city, so the picker closes
                // rather than staying open behind the new card.
                self.close_city_picker();
                Task::none()
            }
            Message::RemoveCity(id) => {
                self.world.remove(id);
                self.mark_dirty();
                self.refresh_snapshot();
                Task::none()
            }
            Message::TogglePin(id) => {
                let pinned = self.world.get(id).is_some_and(|city| !city.pinned);
                self.world.set_pinned(id, pinned);
                self.mark_dirty();
                self.refresh_snapshot();
                Task::none()
            }
            Message::CycleSort => {
                let next = self.world.sort().next();
                self.world.set_sort(next);
                self.mark_dirty();
                self.refresh_snapshot();
                Task::none()
            }
            Message::ChooseSort(sort) => {
                self.world.set_sort(sort);
                self.mark_dirty();
                self.refresh_snapshot();
                Task::none()
            }

            Message::NewAlarm => {
                let (hour, minute) = self.default_alarm_time();
                let mut draft = AlarmDraft::new(Id::next().raw(), hour, minute);
                // A new alarm is *named* by the user, so the name is theirs to
                // keep — but the name it starts with is the application's word
                // for an unnamed alarm, and that word belongs to the language
                // being read. Only the untouched default is replaced; anything
                // the user typed is left exactly as they left it.
                if draft.alarm.label == Alarm::default().label {
                    draft.alarm.label = self.tr(Text::NotifyAlarmFallback).to_string();
                }
                self.view.alarm_draft = Some(draft);
                self.open_dialog();
                Task::none()
            }
            Message::EditAlarm(id) => {
                if let Some(alarm) = self.alarms.get(id) {
                    self.view.alarm_draft = Some(AlarmDraft::existing(alarm.clone()));
                    self.open_dialog();
                }
                Task::none()
            }
            Message::CancelAlarmEdit => {
                if self.view.alarm_draft.take().is_some() {
                    self.close_dialog();
                }
                Task::none()
            }
            Message::CancelDeleteAlarm => {
                if self.view.pending_delete.take().is_some() {
                    self.close_dialog();
                }
                Task::none()
            }
            Message::SaveAlarm => {
                self.save_alarm();
                Task::none()
            }
            Message::DeleteAlarm(id) => {
                self.view.pending_delete = Some(id);
                self.open_dialog();
                Task::none()
            }
            Message::ConfirmDeleteAlarm => {
                if let Some(id) = self.view.pending_delete.take() {
                    self.alarms.remove(id);
                    self.mark_dirty();
                    self.alarms.refresh(self.zone(), self.now);
                    self.view.toast(
                        self.tr(Text::ToastAlarmDeleted),
                        crate::design::palette::Tone::Positive,
                    );
                }
                self.close_dialog();
                Task::none()
            }
            Message::ToggleAlarm(id) => {
                let enabled = self.alarms.get(id).is_some_and(|alarm| !alarm.enabled);
                self.alarms.set_enabled(id, enabled);
                if enabled {
                    self.alarms.refresh(self.zone(), self.now);
                }
                self.mark_dirty();
                Task::none()
            }
            Message::SetAlarmName(name) => {
                self.view.typing = true;
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.alarm.label = name;
                }
                Task::none()
            }
            Message::AlarmHourStep(delta) => {
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.step_hour(delta);
                }
                Task::none()
            }
            Message::AlarmMinuteStep(delta) => {
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.step_minute(delta);
                }
                Task::none()
            }
            Message::SetRepeat(repeat) => {
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.alarm.repeat = repeat;
                }
                Task::none()
            }
            Message::ToggleWeekday(index) => {
                let Ok(day) = <chrono::Weekday as TryFrom<u8>>::try_from(index) else {
                    return Task::none();
                };
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    // Touching a day is how a weekly rule is built, so a draft
                    // that is not weekly yet starts from an empty set: pressing
                    // Monday and then Wednesday means Monday and Wednesday,
                    // not everything except them.
                    let current = match draft.alarm.repeat {
                        crate::domain::alarm::Repeat::Weekly(days) => days,
                        _ => crate::domain::alarm::Weekdays::NONE,
                    };
                    let next = if current.contains(day) {
                        current.with(day, false)
                    } else {
                        current.with(day, true)
                    };
                    draft.alarm.repeat = crate::domain::alarm::Repeat::Weekly(next);
                }
                Task::none()
            }
            Message::SetAlarmSound(sound) => {
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.alarm.sound = sound;
                }
                Task::none()
            }
            Message::SetSnoozeMinutes(minutes) => {
                if let Some(draft) = self.view.alarm_draft.as_mut() {
                    draft.alarm.snooze_minutes = minutes.clamp(1, 60);
                }
                Task::none()
            }
            Message::DismissAlarm(id) => {
                self.alarms.dismiss(id);
                self.view.ringing.retain(|ring| ring.alarm.id != id);
                self.player.stop();
                self.alarms.refresh(self.zone(), self.now);
                self.mark_dirty();
                Task::none()
            }
            Message::SnoozeAlarm(id) => {
                self.snooze(id, None);
                Task::none()
            }
            Message::SnoozeAlarmFor(id, minutes) => {
                self.snooze(id, Some(minutes));
                Task::none()
            }
            Message::AcknowledgeMissed(id) => {
                self.view.missed.retain(|(alarm, _)| alarm.id != id);
                Task::none()
            }
            Message::AcknowledgeAllMissed => {
                self.view.missed.clear();
                Task::none()
            }

            Message::StartPreset(id) => {
                if let Some(preset) = self
                    .timers
                    .presets()
                    .iter()
                    .find(|preset| preset.id == id)
                    .cloned()
                {
                    let _timer_id = self.start_timer(&preset.name, preset.duration);
                    self.view.toast(
                        self.tr_with(Text::ToastStarted, &[Arg::from(&preset.name)]),
                        crate::design::palette::Tone::Positive,
                    );
                }
                Task::none()
            }
            Message::NewTimer => {
                self.view.timer_draft = Some(TimerDraft::new("", Duration::from_secs(300)));
                self.open_dialog();
                Task::none()
            }
            Message::CancelTimerEdit => {
                // The timer's own draft, and nothing else: this button closes
                // the timer editor, so it must not be able to withdraw an alarm
                // that happens to be open behind it, or a deletion question.
                if self.view.timer_draft.take().is_some() {
                    self.close_dialog();
                }
                Task::none()
            }
            Message::SetTimerName(name) => {
                self.view.typing = !name.is_empty();
                if let Some(draft) = self.view.timer_draft.as_mut() {
                    draft.name = name;
                }
                Task::none()
            }
            Message::TimerDurationStep(minutes) => {
                if let Some(draft) = self.view.timer_draft.as_mut() {
                    draft.step(minutes);
                }
                Task::none()
            }
            Message::ConfirmTimer => {
                if let Some(draft) = self.view.timer_draft.take() {
                    let name = draft.name_or_default(self.catalog);
                    self.start_timer(&name, draft.duration);
                    self.view.toast(
                        self.tr_with(Text::ToastStarted, &[Arg::from(&name)]),
                        crate::design::palette::Tone::Positive,
                    );
                }
                self.close_dialog();
                Task::none()
            }
            Message::ToggleTimer(id) => {
                let now = self.elapsed();
                if let Some(timer) = self.timers.get_mut(id) {
                    match timer.state {
                        crate::domain::timer::TimerState::Running => timer.pause(now),
                        _ => timer.start(now),
                    }
                }
                Task::none()
            }
            Message::ResetTimer(id) => {
                if let Some(timer) = self.timers.get_mut(id) {
                    timer.reset();
                }
                Task::none()
            }
            Message::RemoveTimer(id) => {
                self.timers.remove(id);
                Task::none()
            }
            Message::SaveTimerPreset => {
                if let Some(draft) = self.view.timer_draft.as_ref() {
                    let name = draft.name_or_default(self.catalog);
                    self.timers.add_preset(name, draft.duration);
                    self.mark_dirty();
                    self.view.toast(
                        self.tr(Text::ToastPresetSaved),
                        crate::design::palette::Tone::Positive,
                    );
                }
                Task::none()
            }
            Message::RemovePreset(id) => {
                self.timers.remove_preset(id);
                self.mark_dirty();
                Task::none()
            }
            Message::SetTimerSound(sound) => {
                self.sound.timer_sound = sound;
                self.mark_dirty();
                Task::none()
            }

            Message::ToggleStopwatch => {
                let now = self.elapsed();
                if self.stopwatch.is_running() {
                    self.stopwatch.pause(now);
                } else {
                    self.stopwatch.start(now);
                }
                Task::none()
            }
            Message::ResetStopwatch => {
                // Whether a reset takes the laps with it is the user's stored
                // preference, not a decision made here: somebody timing a
                // sequence of laps wants them to survive the reset, and
                // somebody timing a single run does not.
                if self.keep_laps {
                    self.stopwatch.restart();
                } else {
                    self.stopwatch.reset();
                }
                Task::none()
            }
            Message::LapStopwatch => {
                self.stopwatch.lap(self.elapsed());
                Task::none()
            }
            Message::ClearLaps => {
                self.stopwatch.clear_laps();
                Task::none()
            }
            Message::RemoveLap(index) => {
                // The lap is a recorded mark, so removing it renumbers and
                // recomputes what is left rather than leaving a hole.
                self.stopwatch.remove_lap(index);
                Task::none()
            }

            Message::ToggleSound => {
                self.sound.enabled = !self.sound.enabled;
                self.player.set_enabled(self.sound.enabled);
                self.mark_dirty();
                Task::none()
            }
            Message::ToggleNotifications => {
                self.sound.notify = !self.sound.notify;
                self.mark_dirty();
                Task::none()
            }

            Message::Save => {
                self.persist();
                Task::none()
            }
            Message::DismissNotice => {
                self.view.notice = None;
                Task::none()
            }
            Message::Reload => {
                self.reload();
                Task::none()
            }
            Message::Exit => iced::window::close(window::Id::unique()),
        }
    }

    /// The palette, from the theme.
    pub fn theme_palette(&self) -> Palette {
        self.palette()
    }
}

// --- the timing, reconciliation and feature helpers the update loop uses ---

impl OClock {
    /// Starts a timer and returns its id.
    fn start_timer(&mut self, name: &str, duration: Duration) -> u64 {
        let sound = self.sound.timer_sound;
        let now = self.elapsed();
        let id = self.timers.add(name, duration);
        if let Some(timer) = self.timers.get_mut(id) {
            timer.sound = sound;
            timer.start(now);
        }
        id
    }

    /// The current wall-clock time, resampled and re-propagated.
    ///
    /// Everything derived from the civil calendar — the displayed time, the
    /// world clock's rows, the alarm schedule — comes from here, and is only
    /// recomputed once per second.
    pub fn refresh_snapshot(&mut self) {
        let zone = self.zone();
        let now = tz::now_in(zone);
        self.now = now;

        self.snapshot = Snapshot {
            now,
            zone,
            clock: ClockSnapshot::capture(now, zone, self.display()),
            world: self.world.rows(now, zone),
        };

        self.alarms.refresh(zone, now);
    }

    /// The periodic tick.
    ///
    /// The monotonic delta advances every measurement; the wall clock is
    /// resampled at most once a second. That split is what keeps a slow frame
    /// from making the stopwatch jump while still leaving the displayed time
    /// correct to the second.
    pub fn tick(&mut self, delta: Duration) -> Task<Message> {
        self.since_second += delta;
        self.since_change += delta;
        self.last_delta = delta;
        self.view.advance(self.clock.now());

        // The page cross-fade and the dialog are the animations the application
        // itself owns; the controls animate themselves from their own handles.
        //
        // The cross-fade is only *read* here. It is started once, by
        // `start_page_transition`, and a tween interpolates from where it was
        // started on its own; re-stamping it every tick restarts its clock, so
        // it could never be seen to settle — and the page underneath would then
        // go on being laid out and drawn under the current one for the rest of
        // the session, at the price of a cross-fade that finished long ago.
        let now = self.clock.now();
        if self.view.leaving.is_some() && self.page_tween.settled(now) {
            self.view.leaving = None;
            self.view.leaving_progress = 1.0;
        }
        self.view.leaving_progress = 1.0 - self.page_tween.value(now);
        self.view.entering_progress = self.page_tween.value(now);

        if self.view.toast_expired() {
            self.view.dismiss_toast();
        }

        // Timers and the stopwatch are measured every frame; they do not need
        // the wall clock at all.
        self.poll_timers();

        // The two clocks are compared every tick, and a disagreement is acted
        // on before anything else: a measurement that has drifted is worse than
        // one that is late.
        let discontinuity = self.watchdog.sample(&self.timeline);
        if !matches!(discontinuity, Discontinuity::Steady) {
            self.reconcile(discontinuity);
        }

        // The timezone and the desktop's appearance are both probed from the
        // slow housekeeping side of the tick: neither changes often, and both
        // cost a question to somewhere outside the process.
        if let Some(zone) = self.zones.check() {
            self.on_timezone_change(zone);
        }
        self.poll_system_appearance();

        let resample = self.since_second >= SECOND;
        if resample {
            self.since_second = Duration::ZERO;
            self.refresh_snapshot();
        }

        // Alarms are checked against the wall clock, so a second of latency in
        // resampling is a second of latency in ringing — and no more.
        if resample {
            for event in self.alarms.poll(self.now) {
                self.handle_alarm_event(event);
            }
        }

        self.autosave();

        Task::none()
    }

    /// Writes the configuration once the interface has settled.
    ///
    /// Two conditions, both necessary: something has changed, and nothing is
    /// animating — so a stepper being dragged writes once, at the end, and an
    /// alarm that is mid-transition does not write on every frame of it.
    fn autosave(&mut self) {
        if !self.dirty || self.since_change < AUTOSAVE || self.is_animating() {
            return;
        }
        self.persist();
    }

    /// Advances every timer and announces the ones that finished.
    ///
    /// Called from the tick and again after a clock jump, because a machine
    /// coming back from sleep has skipped whatever the timers would have done
    /// while it was away — and a finished timer has to read as finished the
    /// moment it is looked at, not on the next frame.
    fn poll_timers(&mut self) {
        for (id, label, sound) in self.timers.poll(self.elapsed()) {
            if self.sound.enabled {
                self.player.play(sound);
            }
            if self.sound.notify {
                let notification = notify::compose_timer(&label, sound.duration(), self.catalog);
                if let Err(error) = self.notifier.notify(&format!("timer:{id}"), &notification) {
                    eprintln!("OClock: {error}");
                }
            }
        }
    }

    /// Reacts to a schedule event.
    fn handle_alarm_event(&mut self, event: AlarmEvent) {
        match event {
            AlarmEvent::Ringing { alarm, at, late_by } => {
                self.view.ringing.push(Ringing {
                    alarm: alarm.clone(),
                    due: at,
                    late_by,
                    // A snooze offered hours later would ring for an alarm the
                    // user has long since dealt with.
                    snoozable: late_by <= RINGING_GRACE,
                });

                if self.sound.enabled {
                    self.player.play(alarm.sound);
                }

                if self.sound.notify {
                    let notification =
                        notify::compose_alarm(&alarm, alarm.snooze_minutes, self.catalog);
                    let key = format!("alarm:{}", alarm.id);
                    if let Err(error) = self.notifier.notify(&key, &notification) {
                        // A missing notification service is not worth stopping
                        // for: the in-window presentation is still there.
                        eprintln!("OClock: {error}");
                    }
                }
            }
            AlarmEvent::Missed { alarm, late_by, .. } => {
                self.view.missed.push((alarm, late_by));
            }
        }
    }

    /// Brings every measurement back in line after the machine's clocks
    /// disagreed with one another.
    ///
    /// A suspend looks, from in here, like the wall clock jumping forward:
    /// monotonic time does not move while a machine is asleep. A timer that was
    /// counting down should therefore finish, not pause; a stopwatch should
    /// have counted the sleep; and the alarm schedule is recomputed from the
    /// calendar, which is unaffected either way.
    ///
    /// Only the *excess* of the wall delta over the monotonic one is handed to
    /// the durations, because the monotonic delta has already been accounted
    /// for by everything that reads the monotonic clock. Forwarding the whole
    /// wall delta counted the observed part a second time: eleven seconds of
    /// wall time read as twelve.
    ///
    /// A rewind advances nothing at all. The durations run on the monotonic
    /// clock, which did not move, so they are already right — and handing a
    /// rewind's magnitude to them would push a running timer *forward* by a
    /// correction that shortened nothing. The calendar is still re-derived,
    /// because the clock being set back is the clock that now says what time
    /// it is, but the alarms are not polled: a clock set back must not ring
    /// anything on the way back through an hour it has already been through.
    fn reconcile(&mut self, discontinuity: Discontinuity) {
        match discontinuity {
            Discontinuity::Steady => {}
            Discontinuity::Forwarded(missed) => {
                self.timers.catch_up(missed);
                self.stopwatch.catch_up(missed);
                // A timer that ran out while the machine was asleep has to say so
                // now.
                self.poll_timers();

                self.refresh_snapshot();
                for event in self.alarms.poll(self.now) {
                    self.handle_alarm_event(event);
                }
            }
            Discontinuity::Rewound(_) => {
                self.refresh_snapshot();
            }
        }
    }

    /// Reacts to the system timezone changing.
    ///
    /// Alarms mean a wall time, so every schedule has to be recomputed against
    /// the new zone. Timers and the stopwatch are unaffected: they measure
    /// elapsed time and have no calendar meaning.
    pub fn on_timezone_change(&mut self, zone: chrono_tz::Tz) {
        self.now = tz::now_in(zone);
        self.alarms.reschedule(zone, self.now);
        self.refresh_snapshot();
    }

    /// Applies an appearance choice and records it.
    fn set_appearance(&mut self, mode: ThemeMode) {
        self.mode = mode;
        self.follow_system = mode.follows_system();
        let dark = crate::design::theme::system_is_dark();
        self.theme.set_mode(mode, dark, &self.clock);
        self.mark_dirty();
    }

    /// Switches language, now and on every future launch.
    ///
    /// The catalogue is rebuilt rather than read through a pointer, so a view
    /// that captured one still holds a consistent set of strings rather than
    /// seeing half the interface change underneath it.
    fn set_language(&mut self, language: Language) {
        if self.language == language {
            return;
        }
        self.language = language;
        self.catalog = Catalog::new(language);
        // The weekday chips, the clock's date line and the world-clock rows all
        // carry translated text baked into the per-second snapshot, so the
        // snapshot is rebuilt rather than left a second out of date.
        self.refresh_snapshot();
        self.mark_dirty();
    }

    /// Starts the page cross-fade from the beginning.
    fn start_page_transition(&mut self) {
        self.page_tween.jump(0.0);
        self.page_tween.animate(1.0, self.clock.now(), Spec::MOVE);
        self.clock.request();
    }

    /// Starts the dialog's entrance.
    fn open_dialog(&mut self) {
        self.dialog.show(self.clock.now(), Spec::ENTER);
        self.clock.request();
    }

    /// Starts the dialog's exit and clears the state that was showing.
    fn close_dialog(&mut self) {
        self.dialog.hide(self.clock.now(), Spec::EXIT);
        self.clock.request();
    }

    /// Closes the city picker, forgetting what was typed into it.
    fn close_city_picker(&mut self) {
        self.view.city_picker = false;
        self.view.query.clear();
        self.close_dialog();
    }

    /// Closes whatever is on top.
    fn dismiss_topmost(&mut self) {
        if self.view.shortcuts {
            self.view.shortcuts = false;
        } else if !self.view.ringing.is_empty() {
            // Escape answers a ringing alarm, which is what a person reaching
            // for it means.
            for ringing in std::mem::take(&mut self.view.ringing) {
                self.alarms.dismiss(ringing.alarm.id);
            }
            self.player.stop();
            self.alarms.refresh(self.zone(), self.now);
            self.mark_dirty();
        } else if self.view.alarm_draft.is_some() {
            self.view.alarm_draft = None;
            self.close_dialog();
        } else if self.view.timer_draft.is_some() {
            self.view.timer_draft = None;
            self.close_dialog();
        } else if self.view.city_picker {
            self.close_city_picker();
        } else if self.view.pending_delete.is_some() {
            self.view.pending_delete = None;
            self.close_dialog();
        } else if self.view.toast_expired() {
            self.view.dismiss_toast();
        }
    }

    /// The keys the shell owns.
    ///
    /// Modelled on what the platform's own clock does: digits jump to a
    /// section, and Escape backs out one layer at a time.
    fn on_key(&mut self, key: Key) -> Task<Message> {
        // While the user is typing into a field, the field has the keyboard.
        // Escape still gets through, because that is how a field is left.
        if self.view.typing && !matches!(key, Key::Escape) {
            return Task::none();
        }
        self.view.typing = false;

        match key {
            Key::Escape => {
                self.dismiss_topmost();
                Task::none()
            }
            Key::Question => {
                self.view.shortcuts = !self.view.shortcuts;
                Task::none()
            }
            Key::Tab | Key::Down | Key::Right => self.update(Message::FocusNext),
            Key::BackTab | Key::Up | Key::Left => self.update(Message::FocusPrevious),
            Key::Enter | Key::Space => self.update(Message::ActivateFocus),
            Key::Digit(digit) => {
                if self.view.has_modal() {
                    return Task::none();
                }
                match Message::destinations()
                    .iter()
                    .position(|destination| destination.shortcut() == digit)
                {
                    Some(index) => self.update(Message::Navigate(index)),
                    None => Task::none(),
                }
            }
            // The page's own shortcuts, which act on whatever page is showing.
            Key::Start | Key::Lap | Key::Reset | Key::New => {
                if self.view.has_modal() {
                    return Task::none();
                }
                self.page_shortcut(key)
            }
        }
    }

    /// Applies a page-specific shortcut to the page that is showing.
    ///
    /// Each page claims only what means something on it, so a single key does
    /// the obvious thing wherever the user happens to be.
    fn page_shortcut(&mut self, key: Key) -> Task<Message> {
        let message = match (self.view.page, key) {
            // The stopwatch owns S, L and R.
            (4, Key::Start) => Message::ToggleStopwatch,
            (4, Key::Lap) => Message::LapStopwatch,
            (4, Key::Reset) => Message::ResetStopwatch,
            // N means "something new" on the two pages that can make one.
            (2, Key::New) => Message::NewAlarm,
            (3, Key::New) => Message::NewTimer,
            _ => return Task::none(),
        };
        self.update(message)
    }

    /// Writes the edited alarm.
    fn save_alarm(&mut self) {
        let Some(draft) = self.view.alarm_draft.take() else {
            return;
        };

        let mut alarm = draft.alarm;
        alarm.label = if alarm.label.trim().is_empty() {
            self.tr(Text::NotifyAlarmFallback).to_string()
        } else {
            alarm.label.trim().to_string()
        };
        if alarm.snooze_minutes == 0 {
            alarm.snooze_minutes = self.sound.snooze_minutes;
        }

        // Editing an alarm invalidates its old schedule; the engine is told to
        // recompute rather than left to notice a stale entry.
        self.alarms.reschedule(self.zone(), self.now);
        self.alarms.upsert(alarm);
        self.alarms.refresh(self.zone(), self.now);
        self.mark_dirty();
        self.view.toast(
            self.tr(Text::ToastAlarmSaved),
            crate::design::palette::Tone::Positive,
        );
        self.close_dialog();
    }

    /// Postpones a ringing alarm.
    fn snooze(&mut self, id: u64, minutes: Option<u8>) {
        if self.alarms.snooze(id, self.now, minutes) {
            self.view.ringing.retain(|ring| ring.alarm.id != id);
            self.player.stop();
            self.alarms.refresh(self.zone(), self.now);
            self.mark_dirty();
            self.view.toast(
                self.tr(Text::ToastSnoozed),
                crate::design::palette::Tone::Positive,
            );
        }
    }

    /// Adds a city from the picker.
    fn add_city(&mut self, zone: &str) {
        let Some(parsed) = tz::parse(zone) else {
            return;
        };
        let name = tzdata::index()
            .iter()
            .find(|city| city.zone == parsed.name())
            .map(|city| city.name.clone())
            .unwrap_or_else(|| {
                parsed
                    .name()
                    .rsplit('/')
                    .next()
                    .unwrap_or(parsed.name())
                    .replace('_', " ")
            });

        self.world.add_city(name, parsed);
        self.mark_dirty();
        self.refresh_snapshot();
        self.view.toast(
            self.tr(Text::ToastCityAdded),
            crate::design::palette::Tone::Positive,
        );
    }

    /// A sensible time for a new alarm: the next round half hour, or an hour
    /// from now.
    fn default_alarm_time(&self) -> (u32, u32) {
        use chrono::Timelike;

        let at = self.now;
        let minutes = at.minute();
        let rounded = (minutes / 30 + 1) * 30;
        match rounded {
            minute if minute < 60 => (at.hour(), minute),
            _ => ((at.hour() + 1) % 24, 0),
        }
    }

    /// The configuration to write.
    pub fn config(&self) -> Config {
        Config {
            version: crate::services::settings::CURRENT_VERSION,
            appearance: crate::services::settings::Appearance { mode: self.mode },
            clock: self.display(),
            alarms: self.alarms.alarms().to_vec(),
            world: crate::services::settings::World {
                locations: self.world.locations().to_vec(),
                sort: self.world.sort(),
            },
            timers: crate::services::settings::Timers {
                presets: self.timers.presets().to_vec(),
                keep_laps: self.keep_laps,
            },
            sound: self.sound,
            language: self.language,
        }
    }

    /// Writes the configuration, reporting failure rather than losing it.
    pub fn persist(&mut self) {
        match self.store.save(&self.config()) {
            Ok(()) => {
                self.dirty = false;
                self.saved_once = true;
                self.last_save = self.clock.now();
                self.since_change = Duration::ZERO;
            }
            Err(error) => {
                eprintln!("OClock: {error}");
                self.view.notice = Some(Notice {
                    headline: self.tr(Text::NoticeUnsaved).to_string(),
                    detail: error.to_string(),
                    tone: self.palette(),
                    recoverable: true,
                });
            }
        }
    }

    /// Re-reads the configuration, discarding unsaved changes.
    pub fn reload(&mut self) {
        let loaded = self.store.load_sections(Config::read);
        let config = loaded.value().migrate().sanitised();

        self.alarms = AlarmSet::new(config.alarms);
        self.timers = TimerSet::new(config.timers.presets);
        self.world = WorldClocks::new(config.world.locations, config.world.sort);
        self.sound = config.sound;
        self.mode = config.appearance.mode;
        self.follow_system = self.mode.follows_system();
        self.display_format = config.clock.hour_format;
        self.show_seconds = config.clock.show_seconds;
        self.show_analog = config.clock.show_analog;
        self.keep_laps = config.timers.keep_laps;
        self.system_dark = crate::design::theme::system_is_dark();
        self.appearance_read = self.uptime();
        self.player.set_enabled(self.sound.enabled);

        self.theme = ThemeState::new(self.mode, crate::design::theme::system_is_dark());
        self.set_language(config.language);
        self.refresh_snapshot();
        self.dirty = false;
    }

    /// Whether the configuration has changes that are not yet on disk.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marks the configuration as changed, restarting the autosave debounce.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.since_change = Duration::ZERO;
    }

    /// The instant of the last successful write, on the monotonic clock.
    pub fn last_save(&self) -> Duration {
        self.last_save
    }

    /// The delta the last tick carried.
    pub fn last_delta(&self) -> Duration {
        self.last_delta
    }

    /// How long the window has been open, for the first-paint fade.
    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// The appearance in effect.
    pub fn appearance(&self) -> Appearance {
        self.theme.appearance()
    }
}

// --- the runtime: what the application subscribes to and draws ---

impl OClock {
    /// The application's subscriptions.
    ///
    /// Four streams, and only four:
    ///
    /// * **the tick**, whose interval the application chooses — 16 ms while
    ///   anything is animating, 64 ms when nothing is. A running stopwatch
    ///   looks smooth, and an idle window costs a handful of wakeups a second
    ///   rather than sixty.
    /// * **window events**, for resizes.
    /// * **key presses**, mapped onto the shell's shortcuts by a plain
    ///   function.
    /// * **notification actions**, so a snooze pressed on a desktop
    ///   notification reaches the same code as one pressed in the window.
    ///
    /// There is no other polling: the alarm schedule is derived from the wall
    /// clock on the tick, not watched.
    ///
    /// # Why nothing here captures
    ///
    /// Iced identifies a running subscription by *hashing its recipe*, and
    /// the mapper is part of the recipe. A closure that captures anything
    /// therefore cannot be compared, cloned into a new stream or sent to the
    /// runtime's worker threads consistently — `Subscription::map` rejects
    /// capturing closures outright, and `filter_map` does the same.
    ///
    /// So every mapper below is non-capturing: the tick and the key press are
    /// plain function items, and the resize matcher is a closure over its
    /// argument alone. The state this application would have been tempted to
    /// capture does not reach a subscription at all:
    ///
    /// * the **tick interval** is a *value* handed to `time::every`, not
    ///   captured by its mapper. The duration is hashed into the recipe, so
    ///   switching between the frame and idle rates rebuilds the stream on its
    ///   own — the interval still follows the animation state, with no capture
    ///   and no [`Subscription::with`].
    /// * the **delta** is not carried by the message at all. It is measured
    ///   against the monotonic clock in [`OClock::update`], which is the only
    ///   place that could know it, and is therefore the correct place.
    /// * the **notification receiver** is captured by a *stream*, which iced
    ///   passes to its own executor and is not compared, so it is free to own
    ///   the state it reads from.
    ///
    /// [`OClock::update`]: OClock::update
    pub fn subscription(&self) -> Subscription<Message> {
        // The interval is state-dependent, and that is safe: it is an argument
        // to `time::every`, not a captured value. Because the duration is part
        // of the recipe's identity, a change here re-creates the stream — the
        // rate follows the animation state without the mapper capturing it.
        let interval = if self.needs_frames() {
            motion::FRAME
        } else {
            motion::IDLE
        };

        // `tick` is a plain function, not a closure: `Subscription::map`
        // requires a mapper that captures nothing, and a function item is
        // zero-sized where a closure over `interval` would be sixteen bytes of
        // state. The instant is dropped, because the real delta comes from the
        // monotonic clock in `update`, so a tick that arrives late still says
        // how late it was — which is what the stopwatch and the timers need,
        // and what the watchdog needs in order to spot a stall.
        let tick = time::every(interval).map(tick);

        // Read from the raw event stream, where the widgets have not yet had a
        // say — see `raw_event`.
        let events = iced::event::listen_with(raw_event);

        Subscription::batch([
            tick,
            events,
            keyboard::on_key_press(key_press),
            self.action_subscription(),
        ])
    }

    /// Whether anything is animating right now.
    ///
    /// This is the question the *configuration* asks: it is what holds an
    /// autosave off until a control has finished moving, because a stepper
    /// being dragged should write once, at the end, and not once per frame of
    /// itself. It is deliberately not the question the *frame rate* asks — see
    /// [`OClock::needs_frames`].
    pub fn is_animating(&self) -> bool {
        self.clock.active()
            || self.navigation.is_moving(&self.clock)
            || self.theme.is_transitioning(&self.clock)
            || self.view.has_overlay()
            || self.stopwatch.is_running()
            || self.timers.any_running(self.elapsed())
    }

    /// Whether the interface has to be redrawn on every frame.
    ///
    /// Wider than [`OClock::is_animating`], because a sweeping second hand is
    /// motion as much as a tween is. What makes it different is that it is
    /// driven by the wall clock rather than by anything the application owns:
    /// there is no tween to settle and no state to write, so it must not be
    /// allowed to hold the autosave off the way a transition holds it off.
    ///
    /// The clock page is the one place the two answers disagree, and only
    /// while the analog face is on screen — which is the face's whole reason
    /// for being there. Redrawing it at the idle rate would leave the hand
    /// stepping fifteen times a second while the numbers beside it read to the
    /// second.
    fn needs_frames(&self) -> bool {
        self.is_animating() || (self.view.page == 0 && self.show_analog)
    }

    /// The notification action stream.
    ///
    /// Built fresh on every call — which the runtime does each frame — as a
    /// handle onto the one shared receiver. Reading it is a poll of the
    /// channel, not a spin: an application with nothing waiting on a desktop
    /// notification is left asleep, and the runtime wakes it when a worker
    /// sends something.
    ///
    /// The stream ends when the channel does, and only then: every sender is
    /// gone, or the receiver has been taken. An empty channel is not an ending,
    /// which is what keeps the subscription alive between one action and the
    /// next.
    fn action_subscription(&self) -> Subscription<Message> {
        Subscription::run_with_id("oclock/notification-actions", self.action_stream())
    }

    /// The same stream the subscription is built from, so a test can drive it
    /// exactly as the runtime does.
    fn action_stream(&self) -> ActionStream {
        ActionStream {
            channel: std::sync::Arc::clone(&self.action_channel),
        }
    }

    /// Applies a choice made on a desktop notification.
    #[allow(dead_code)]
    fn on_action(&self, chosen: Chosen) -> Vec<Message> {
        on_action(chosen)
    }

    /// The window.
    pub fn view(&self) -> Element<'static, Message> {
        crate::ui::shell::view(self)
    }

    /// The iced theme, derived from the palette in effect.
    ///
    /// OClock styles every widget itself; this is here so that the parts iced
    /// draws on the application's behalf — the window background, the scrollbar,
    /// the text cursor — are ours too.
    pub fn theme(&self) -> iced::Theme {
        let palette = self.palette();
        iced::Theme::custom(crate::APP_NAME.to_string(), palette.to_iced())
    }

    /// Applies the system appearance message.
    pub fn on_system_appearance(&mut self, dark: bool) {
        if self.theme.follow_system(dark, &self.clock) {
            self.mark_dirty();
        }
    }

    /// Asks the desktop what its light/dark preference is, and says so if it
    /// has changed.
    ///
    /// `Match system` is the one mode where this can matter: the user has asked
    /// for whatever the desktop says, so a desktop that changed its mind has to
    /// be noticed. An explicit Light or Dark is the user's own decision and is
    /// not second-guessed, so nothing is read at all in those modes.
    ///
    /// Iced 0.13 has no event for a desktop appearance change — the surface it
    /// renders into says nothing about the theme the compositor is drawing it
    /// with — so there is nothing to subscribe to and this is a poll. It is
    /// deliberately a slow one, on the one-second resample rather than on the
    /// frame clock: reading it is a question to the desktop, and a clock that
    /// redraws sixty times a second must not ask sixty times a second.
    fn poll_system_appearance(&mut self) {
        if !self.mode.follows_system() {
            return;
        }

        let elapsed = self.uptime();
        if elapsed.saturating_sub(self.appearance_read) < self.appearance_poll {
            return;
        }
        self.appearance_read = elapsed;

        let dark = crate::design::theme::system_is_dark();
        if dark != self.system_dark {
            self.system_dark = dark;
            self.on_system_appearance(dark);
        }
    }
}

/// Maps a tick instant onto the tick message.
///
/// A plain function because that is what `Subscription::map` takes: iced hashes
/// a subscription's mapper as part of the subscription's identity, so a mapper
/// that captured the tick interval could not be compared across frames, and
/// `Subscription::map` rejects capturing closures outright. A function item
/// captures nothing, and the interval does not need to travel with the message
/// because the delta is measured against the monotonic clock in
/// [`OClock::update`] when the message arrives.
fn tick(_instant: std::time::Instant) -> Message {
    Message::Tick
}

/// Maps a key press onto one of the shell's shortcuts.
///
/// A plain function because that is what `keyboard::on_key_press` takes: the
/// mapping has to be free of any captured state, which is also what makes it
/// obvious at a glance exactly which keys OClock claims. Everything not listed
/// here — every letter, every function key, every modifier combination the
/// shell does not use — is left alone, so a future text field can have them.
///
/// Escape is not in the list, and that is not an oversight: `on_key_press`
/// only hears the events no widget captured, and a dialog's field is a widget
/// that captures all of them. See [`raw_event`].
fn key_press(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::key::Named;
    use iced::keyboard::Key as Pressed;

    if !unmodified(modifiers) {
        return None;
    }

    let key = match key {
        Pressed::Named(Named::Enter) => Key::Enter,
        Pressed::Named(Named::Tab) => Key::Tab,
        Pressed::Named(Named::Space) => Key::Space,
        Pressed::Named(Named::ArrowLeft) => Key::Left,
        Pressed::Named(Named::ArrowRight) => Key::Right,
        Pressed::Named(Named::ArrowUp) => Key::Up,
        Pressed::Named(Named::ArrowDown) => Key::Down,
        Pressed::Character(character) => match character.as_str() {
            "?" => Key::Question,
            "s" => Key::Start,
            "l" => Key::Lap,
            "r" => Key::Reset,
            "n" => Key::New,
            other if other.len() == 1 && other.starts_with(|c: char| c.is_ascii_digit()) => {
                Key::Digit(other.parse().unwrap_or(0))
            }
            _ => return None,
        },
        Pressed::Unidentified => return None,
        _ => return None,
    };

    Some(Message::Key(key))
}

/// Whether a key press is OClock's rather than the desktop's.
///
/// A modified press is the desktop's, so that a desktop's Ctrl+d, Ctrl+1 and
/// so on keep working. Shift is not treated as a modifier here, because it is
/// how `?` is typed.
///
/// Both of the shell's listeners ask this, so the keys one of them claims and
/// the keys the other does not cannot drift apart.
fn unmodified(modifiers: iced::keyboard::Modifiers) -> bool {
    !(modifiers.command() || modifiers.control() || modifiers.alt())
}

/// Maps a runtime event onto the shell's own messages.
///
/// A plain function for the same reason [`key_press`] is one: `listen_with`
/// compares its mapper as part of the subscription's identity, so it has to
/// capture nothing.
///
/// Two things are read here rather than from `keyboard::on_key_press`:
///
/// * the window's size, which nothing else wants, and which is consumed here so
///   that no other listener sees it;
/// * **Escape**, which has to be read before the widgets have had their say.
///
/// `on_key_press` is told about an event only if no widget captured it, and a
/// dialog with a text field is precisely the case that matters: a focused
/// `TextInput` captures every key it is handed, Escape among them — it spends
/// Escape putting its own focus down — so the key that was meant to close the
/// dialog was swallowed by the field inside it, and the dialog stayed. Reading
/// the raw stream means Escape arrives exactly once whatever is on screen.
///
/// Nothing is taken away by this. The event still reaches the widget tree
/// first, so a field still spends Escape on its focus; a message is simply
/// raised alongside it.
fn raw_event(
    event: iced::Event,
    _status: iced::event::Status,
    _window: window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Window(window::Event::Resized(size)) => {
            Some(Message::Resized(size.width, size.height))
        }
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            modifiers,
            ..
        }) if unmodified(modifiers) => Some(Message::Key(Key::Escape)),
        _ => None,
    }
}

/// Turns a choice made on a desktop notification into messages.
///
/// A notification's action arrives as a string with no type attached, so the
/// mapping is done once, here, and the result is exactly the message the
/// in-window button would have sent.
pub fn on_action(chosen: Chosen) -> Vec<Message> {
    let Some(id) = chosen.key.strip_prefix("alarm:") else {
        // A timer notification has nothing to do but be dismissed.
        return Vec::new();
    };
    let Ok(id) = id.parse::<u64>() else {
        return Vec::new();
    };

    // The dismiss button carries the id "dismiss"; a notification that was
    // closed without a choice arrives as DISMISSED. Both mean "I am done with
    // this alarm", so both dismiss: the old code only recognised DISMISSED and
    // sent a snooze for the button that plainly says "Dismiss".
    if chosen.is(crate::services::notify::DISMISSED) || chosen.is(crate::services::notify::DISMISS)
    {
        vec![Message::DismissAlarm(id)]
    } else {
        vec![Message::SnoozeAlarm(id)]
    }
}

/// A test harness: complete applications with recording services, built
/// without a window, a clock or a desktop.
///
/// The page tests need a fully wired application to render, and the
/// application tests need one to drive. Both are served from here rather than
/// from two harnesses that could drift apart.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::services::notify::{Actions, RecordingNotifier, DISMISSED};

    /// A scratch directory for the store, cleaned up afterwards.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            let mut path = std::env::temp_dir();
            path.push(format!("oclock-app-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch directory");
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    use std::path::PathBuf;

    /// An application with the default configuration, for a page test.
    pub fn sample() -> OClock {
        let (app, _scratch, _notifier) = harness("page");
        app
    }

    /// An application with one alarm, for a page test.
    pub fn sample_with_alarm() -> OClock {
        let (mut app, _scratch, _notifier) = harness("page-alarm");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Wake up".into()));
        let _ = app.update(Message::SaveAlarm);
        app
    }

    /// An application with one running timer, for a page test.
    pub fn sample_with_timer() -> OClock {
        let (mut app, _scratch, _notifier) = harness("page-timer");
        let preset = app.timers.presets()[1].id;
        let _ = app.update(Message::StartPreset(preset));
        app
    }

    /// The timers a default installation starts with: the built-in presets.
    pub fn app_with_timers() -> TimerSet {
        TimerSet::new(Config::first_run().timers.presets)
    }

    /// An application with the given alarms, on the given timers.
    pub fn app_with_alarms(alarms: Vec<Alarm>, timers: TimerSet) -> OClock {
        let (mut app, _scratch, _notifier) = harness("app-alarms");
        app.timers = timers;
        for alarm in alarms {
            app.alarms.upsert(alarm);
        }
        app
    }

    /// An application with the given alarms, timers and presets, on the
    /// default configuration.
    ///
    /// Timers arrive as a name and a total because that is the only way to make
    /// one: the collection allocates the identifier, so that no two timers can
    /// ever be handed the same one.
    pub fn app_with(
        alarms: Vec<Alarm>,
        timers: Vec<(String, Duration)>,
        presets: Vec<Preset>,
    ) -> OClock {
        let (mut app, _scratch, _notifier) = harness("app-with");
        for preset in presets {
            app.timers.upsert_preset(preset);
        }
        for alarm in alarms {
            app.alarms.upsert(alarm);
        }
        for (label, total) in timers {
            app.timers.add(label, total);
        }
        app
    }

    /// A complete application, with recording services, for tests.
    fn harness(label: &str) -> (OClock, Scratch, std::sync::Arc<RecordingNotifier>) {
        let scratch = Scratch::new(label);
        let store = Store::new(scratch.0.join("settings.json"));
        let actions = Actions::new();
        let notifier = std::sync::Arc::new(RecordingNotifier::new(&actions));
        let player = Player::scripted();

        // The receiver is left for the application to take. Taking it here as
        // well would swap in a fresh, never-written-to channel underneath the
        // worker, so every notification action a test produced would be dropped
        // on the floor — and a test asserting on one would pass for the wrong
        // reason.
        let app = OClock::with_config(
            Config::first_run(),
            store,
            Box::new(NotifierProxy(notifier.clone())),
            actions,
            player,
        );
        (app, scratch, notifier)
    }

    /// Wraps the recorder so the application owns a boxed notifier.
    struct NotifierProxy(std::sync::Arc<RecordingNotifier>);

    impl Notifier for NotifierProxy {
        fn notify(&self, key: &str, notification: &Notification) -> Result<(), NotifyError> {
            self.0.notify(key, notification)
        }
    }

    use crate::domain::alarm::{Alarm, Repeat};
    use crate::domain::stopwatch::Stopwatch;
    use crate::domain::timer::TimerPreset as Preset;

    use crate::services::notify::{Notification, Notifier, NotifyError};
    use crate::services::storage;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    /// How close two readings of the same running measurement have to be.
    ///
    /// A measurement is read from the live monotonic clock rather than from
    /// accumulated tick deltas, so the few microseconds that pass between one
    /// assertion's reading and the next are real elapsed time, not drift.
    const TOLERANCE: Duration = Duration::from_millis(50);

    fn near(left: Duration, right: Duration) -> bool {
        left.abs_diff(right) <= TOLERANCE
    }

    #[test]
    fn the_subscription_tree_can_be_built() {
        // Building the subscriptions is where the runtime panics if a mapper
        // captures anything, and it is the first thing `main` does after the
        // window opens. Exercising it here means a capture cannot reach a
        // release build's silence and a user's first launch.
        //
        // Both rates are covered, because the interval is chosen from the
        // animation state and each choice used to be a different closure.
        let (mut app, _scratch, _notifier) = harness("subscription");
        let _: Subscription<Message> = app.subscription();

        let _ = app.update(Message::Navigate(1));
        assert!(app.is_animating(), "the page slide is animating");
        let _: Subscription<Message> = app.subscription();
    }

    /// An application built from `config`, the way `main` builds one.
    fn app_from(config: Config, store: Store) -> OClock {
        let actions = Actions::new();
        let notifier = std::sync::Arc::new(RecordingNotifier::new(&actions));
        // The receiver is left for the application to take, as it does at
        // startup.
        OClock::with_config(
            config,
            store,
            Box::new(NotifierProxy(notifier)),
            actions,
            Player::scripted(),
        )
    }

    #[test]
    fn the_language_is_read_from_the_configuration() {
        // Loading: the application's language is whatever the document said.
        let app = app_from(
            Config {
                language: Language::French,
                ..Config::first_run()
            },
            Store::new(std::path::PathBuf::from("settings.json")),
        );
        assert_eq!(app.language, Language::French);
        assert_eq!(app.tr(Text::PageClock), "Horloge");
        assert_eq!(app.tr(Text::ActionSave), "Enregistrer");
        assert!(!app.is_rtl());
    }

    #[test]
    fn switching_language_rebuilds_the_strings_immediately() {
        let (mut app, _scratch, _notifier) = harness("lang-switch");
        assert_eq!(app.tr(Text::PageClock), "Clock");

        let _ = app.update(Message::ChooseLanguage(Language::Persian));
        assert_eq!(app.language, Language::Persian);
        assert_eq!(app.tr(Text::PageClock), "ساعت");
        assert!(app.is_rtl(), "Persian is written right to left");

        let _ = app.update(Message::ChooseLanguage(Language::Arabic));
        assert_eq!(app.tr(Text::PageClock), "الساعة");

        let _ = app.update(Message::ChooseLanguage(Language::Chinese));
        assert_eq!(app.tr(Text::PageClock), "时钟");
        assert!(!app.is_rtl(), "Chinese is written left to right");
    }

    #[test]
    fn choosing_the_language_already_in_use_changes_nothing() {
        let (mut app, _scratch, _notifier) = harness("lang-noop");
        let _ = app.update(Message::ChooseLanguage(Language::German));
        let before = app.catalog;
        let _ = app.update(Message::ChooseLanguage(Language::German));
        // A no-op must not mark the configuration dirty, or every accidental
        // re-selection would queue a write.
        assert_eq!(app.catalog.language(), before.language());
    }

    #[test]
    fn the_language_is_written_to_the_configuration() {
        let (mut app, _scratch, _notifier) = harness("lang-save");
        let _ = app.update(Message::ChooseLanguage(Language::Korean));
        assert_eq!(app.config().language, Language::Korean);
    }

    #[test]
    fn a_saved_language_survives_a_restart() {
        // Persistence, end to end: choose, write the document the autosave would
        // write, and read it back the way a next launch does.
        let scratch = Scratch::new("lang-persist");
        let path = scratch.0.join("settings.json");
        let store = Store::new(path.clone());

        let mut app = app_from(Config::first_run(), store.clone());
        let _ = app.update(Message::ChooseLanguage(Language::Russian));
        assert_eq!(app.config().language, Language::Russian);

        let saved = app.config();
        std::fs::write(&path, serde_json::to_string_pretty(&saved).unwrap()).unwrap();

        // The launch path: hand the document to the same reader `start` uses.
        let loaded = store.load_sections(Config::read);
        assert!(!loaded.is_fresh(), "the document should have been read");
        let config = loaded.value();
        assert_eq!(config.language, Language::Russian);

        let reopened = app_from(config, store);
        assert_eq!(reopened.language, Language::Russian);
        assert_eq!(reopened.tr(Text::PageClock), "Часы");
        assert!(!reopened.is_rtl());
    }

    #[test]
    fn the_right_to_left_languages_are_the_three_that_are() {
        let (mut app, _scratch, _notifier) = harness("lang-rtl");
        let rtl: Vec<Language> = Language::ALL
            .into_iter()
            .filter(|language| {
                let _ = app.update(Message::ChooseLanguage(*language));
                app.is_rtl()
            })
            .collect();
        assert_eq!(
            rtl,
            vec![Language::Arabic, Language::Persian, Language::Urdu],
            "Persian, Arabic and Urdu, and nothing else, are right to left"
        );
    }

    #[test]
    fn the_left_to_right_languages_are_the_other_nine() {
        let (mut app, _scratch, _notifier) = harness("lang-ltr");
        for language in Language::ALL {
            let _ = app.update(Message::ChooseLanguage(language));
            assert_eq!(
                app.is_rtl(),
                language.is_rtl(),
                "{language:?} disagrees with the language table"
            );
        }
    }

    #[test]
    fn an_unknown_stored_language_does_not_stop_a_launch() {
        // A document naming a language this build does not speak must not fail
        // to load; the loader keeps the detected default and reports the
        // repair, which is the same path as any other unreadable value.
        let scratch = Scratch::new("lang-unknown");
        let path = scratch.0.join("settings.json");
        std::fs::write(&path, r#"{"language": "klingon"}"#).unwrap();

        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut loader = storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        assert!(
            Language::ALL.contains(&config.language),
            "a usable language must come out, got {:?}",
            config.language
        );
    }

    #[test]
    fn a_fresh_application_starts_on_the_clock() {
        let (app, _scratch, _notifier) = harness("fresh");
        assert_eq!(app.view.page, 0);
        assert_eq!(app.navigation.selected(), 0);
        assert!(app.view.toasts.is_empty());
        assert!(!app.is_animating(), "nothing should be animating at rest");
    }

    #[test]
    fn a_missing_file_gives_the_built_in_configuration() {
        // The regression this guards: reading an absent document yields
        // `Config::read`'s defaults, which have no cities in them. A first
        // launch has to start from `Config::first_run()` instead, or the world
        // clock comes up empty and the timer strip is bare.
        let scratch = Scratch::new("first-run");
        let store = Store::new(scratch.0.join("settings.json"));
        assert!(!store.exists());

        let loaded = store.load_sections(Config::read);
        assert!(loaded.is_fresh());
        assert!(
            loaded.value().world.locations.is_empty(),
            "the document itself has no cities, which is why the first-run path exists"
        );

        let config = Config::first_run().migrate().sanitised();
        assert_eq!(config.world.locations.len(), 3);
        assert_eq!(config.timers.presets.len(), 5);
        assert!(config.clock.show_seconds);
    }

    #[test]
    fn the_built_in_configuration_is_usable() {
        let (app, _scratch, _notifier) = harness("defaults");
        assert_eq!(app.world.len(), 3, "the built-in world clock");
        assert_eq!(app.timers.presets().len(), 5, "the built-in presets");
        assert!(app.sound.enabled);
        assert!(app.alarms.is_empty(), "no alarms are invented for the user");
        assert!(app.snapshot.world.len() == 3, "every city has a row");
        for row in &app.snapshot.world {
            assert!(
                row.at.is_some(),
                "{} could not be placed",
                row.location.city
            );
        }
    }

    #[test]
    fn navigating_changes_the_page_and_starts_a_transition() {
        let (mut app, _scratch, _notifier) = harness("navigate");
        let _ = app.update(Message::Navigate(2));

        assert_eq!(app.view.page, 2);
        assert_eq!(app.navigation.selected(), 2);
        assert!(app.is_animating(), "the page slide is an animation");
    }

    #[test]
    fn a_modal_blocks_navigation() {
        let (mut app, _scratch, _notifier) = harness("modal");
        let _ = app.update(Message::NewAlarm);
        assert!(app.view.has_modal());

        let _ = app.update(Message::Navigate(3));
        assert_eq!(app.view.page, 0, "a dialog takes precedence over a page");
    }

    #[test]
    fn an_alarm_can_be_created_edited_and_deleted() {
        let (mut app, _scratch, _notifier) = harness("alarms");

        let _ = app.update(Message::NewAlarm);
        let id = app.view.alarm_draft.as_ref().expect("a draft").alarm.id;
        let _ = app.update(Message::SetAlarmName("Gym".into()));
        let _ = app.update(Message::AlarmHourStep(1));
        let _ = app.update(Message::SaveAlarm);

        assert_eq!(app.alarms.len(), 1);
        assert_eq!(app.alarms.get(id).expect("saved").label, "Gym");
        assert!(app.view.alarm_draft.is_none());

        let _ = app.update(Message::EditAlarm(id));
        let _ = app.update(Message::SetAlarmName("Gym class".into()));
        let _ = app.update(Message::SaveAlarm);
        assert_eq!(app.alarms.get(id).expect("edited").label, "Gym class");

        let _ = app.update(Message::DeleteAlarm(id));
        assert!(app.view.pending_delete.is_some());
        let _ = app.update(Message::ConfirmDeleteAlarm);
        assert!(app.alarms.is_empty());
    }

    #[test]
    fn an_unnamed_alarm_gets_a_name() {
        let (mut app, _scratch, _notifier) = harness("unnamed");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("   ".into()));
        let _ = app.update(Message::SaveAlarm);
        assert_eq!(app.alarms.alarms()[0].label, "Alarm");
    }

    #[test]
    fn editing_a_weekly_alarm_toggles_its_days() {
        let (mut app, _scratch, _notifier) = harness("weekly");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::ToggleWeekday(0));
        let _ = app.update(Message::ToggleWeekday(2));
        let _ = app.update(Message::SaveAlarm);

        match app.alarms.alarms()[0].repeat {
            Repeat::Weekly(days) => {
                assert_eq!(
                    days.days(),
                    vec![chrono::Weekday::Mon, chrono::Weekday::Wed]
                );
            }
            other => panic!("expected a weekly rule, got {other:?}"),
        }
    }

    #[test]
    fn an_alarm_can_be_armed_and_disarmed() {
        let (mut app, _scratch, _notifier) = harness("arm");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        let id = app.alarms.alarms()[0].id;
        assert!(app.alarms.get(id).expect("saved").is_armed());

        let _ = app.update(Message::ToggleAlarm(id));
        assert!(!app.alarms.get(id).expect("still there").is_armed());

        let _ = app.update(Message::ToggleAlarm(id));
        assert!(app.alarms.get(id).expect("still there").is_armed());
    }

    #[test]
    fn a_timer_runs_and_pauses() {
        let (mut app, _scratch, _notifier) = harness("timer");
        let preset = app.timers.presets()[0].id;
        let _ = app.update(Message::StartPreset(preset));

        assert_eq!(app.timers.len(), 1);
        let id = app.timers.timers()[0].id;
        assert!(app.timers.timers()[0].state.is_running());

        let _ = app.update(Message::ToggleTimer(id));
        assert!(!app.timers.timers()[0].state.is_running());

        let _ = app.update(Message::ResetTimer(id));
        assert_eq!(
            app.timers.timers()[0].elapsed(app.elapsed()),
            Duration::ZERO
        );

        let _ = app.update(Message::RemoveTimer(id));
        assert!(app.timers.is_empty());
    }

    #[test]
    fn a_custom_timer_can_be_made() {
        let (mut app, _scratch, _notifier) = harness("custom");
        let _ = app.update(Message::NewTimer);
        let _ = app.update(Message::SetTimerName("Pasta".into()));
        let _ = app.update(Message::TimerDurationStep(5));
        let _ = app.update(Message::ConfirmTimer);

        assert_eq!(app.timers.len(), 1);
        let timer = app.timers.timers()[0].clone();
        assert_eq!(timer.label, "Pasta");
        assert_eq!(timer.total, Duration::from_secs(600));
    }

    #[test]
    fn a_preset_can_be_saved_and_removed() {
        let (mut app, _scratch, _notifier) = harness("presets");
        let before = app.timers.presets().len();

        let _ = app.update(Message::NewTimer);
        let _ = app.update(Message::SetTimerName("Tea".into()));
        let _ = app.update(Message::SaveTimerPreset);
        assert_eq!(app.timers.presets().len(), before + 1);

        let custom = app.timers.presets().last().expect("the new one").id;
        let _ = app.update(Message::RemovePreset(custom));
        assert_eq!(app.timers.presets().len(), before);
    }

    #[test]
    fn the_stopwatch_starts_pauses_and_records_laps() {
        let (mut app, _scratch, _notifier) = harness("stopwatch");
        let _ = app.update(Message::ToggleStopwatch);
        assert!(app.stopwatch.is_running());

        let _ = app.update(Message::LapStopwatch);
        assert_eq!(app.stopwatch.lap_count(), 1);

        let _ = app.update(Message::ToggleStopwatch);
        assert!(!app.stopwatch.is_running());

        let _ = app.update(Message::ClearLaps);
        assert_eq!(app.stopwatch.lap_count(), 0);

        let _ = app.update(Message::ResetStopwatch);
        assert_eq!(app.stopwatch.elapsed(app.elapsed()), Duration::ZERO);
    }

    #[test]
    fn a_lap_needs_a_running_stopwatch() {
        let (mut app, _scratch, _notifier) = harness("lap");
        let _ = app.update(Message::LapStopwatch);
        assert_eq!(
            app.stopwatch.lap_count(),
            0,
            "a stopped stopwatch records nothing"
        );
    }

    #[test]
    fn a_city_can_be_added_searched_for_and_removed() {
        let (mut app, _scratch, _notifier) = harness("world");

        let _ = app.update(Message::AddCity("Europe/Paris".into()));
        assert_eq!(app.world.len(), 4);
        let id = app.world.locations().last().expect("added").id;

        let _ = app.update(Message::SearchCities("tokyo".into()));
        assert!(app.view.city_picker);
        assert!(app.view.query.contains("tokyo"));

        let _ = app.update(Message::ToggleCityPicker);
        assert!(!app.view.city_picker);

        let _ = app.update(Message::TogglePin(id));
        assert!(app.world.get(id).expect("still there").pinned);

        let _ = app.update(Message::RemoveCity(id));
        assert_eq!(app.world.len(), 3);
    }

    #[test]
    fn an_unknown_city_is_refused_rather_than_guessed() {
        let (mut app, _scratch, _notifier) = harness("badcity");
        let before = app.world.len();
        let _ = app.update(Message::AddCity("Middle/Earth".into()));
        assert_eq!(app.world.len(), before);
    }

    #[test]
    fn the_sort_order_cycles() {
        let (mut app, _scratch, _notifier) = harness("sort");
        let first = app.world.sort();
        let _ = app.update(Message::CycleSort);
        assert_ne!(app.world.sort(), first);
    }

    #[test]
    fn the_appearance_can_be_chosen_and_cycled() {
        let (mut app, _scratch, _notifier) = harness("theme");
        let _ = app.update(Message::ChooseAppearance(
            crate::design::theme::ThemeMode::Dark,
        ));
        assert_eq!(app.mode, crate::design::theme::ThemeMode::Dark);
        assert!(app.is_animating(), "the theme change is a transition");

        // Pressing the control walks the whole cycle, ending back where the
        // desktop's own preference takes over again.
        let _ = app.update(Message::ToggleAppearance);
        assert_eq!(app.mode, crate::design::theme::ThemeMode::System);
        assert!(app.follow_system, "which is a mode, not a one-off");

        let _ = app.update(Message::ToggleAppearance);
        assert_eq!(app.mode, crate::design::theme::ThemeMode::Light);

        let _ = app.update(Message::ToggleAppearance);
        assert_eq!(app.mode, crate::design::theme::ThemeMode::Dark);
    }

    #[test]
    fn the_desktop_is_only_asked_when_the_mode_is_following_it() {
        // The poll is the only live source of the system appearance there is,
        // and an explicit choice is the user's: nothing is read in one.
        let (mut app, _scratch, _notifier) = harness("appearance-poll");

        // The interval is a minute of real time, which a test cannot wait for,
        // so it is shortened here and the gate is what is under test.
        app.appearance_poll = Duration::ZERO;
        let read = app.appearance_read;
        let _ = app.tick(Duration::from_millis(16));
        assert!(
            app.appearance_read > read,
            "following the desktop, the tick asks it"
        );

        let _ = app.update(Message::ChooseAppearance(
            crate::design::theme::ThemeMode::Dark,
        ));
        let read = app.appearance_read;
        let _ = app.tick(Duration::from_millis(16));
        assert_eq!(
            app.appearance_read, read,
            "a chosen appearance is not re-read"
        );

        // And the message still lands when the desktop does move.
        let _ = app.update(Message::SystemAppearance(true));
        assert_eq!(app.theme.appearance(), Appearance::Dark);
    }

    #[test]
    fn a_save_that_fails_is_reported_and_the_change_kept() {
        let scratch = Scratch::new("unwritable");
        // A path under a *file* cannot be created, so the write fails.
        let blocker = scratch.0.join("blocker");
        std::fs::write(&blocker, "not a directory").expect("writes");

        let actions = Actions::new();
        let notifier = std::sync::Arc::new(RecordingNotifier::new(&actions));
        let mut app = OClock::with_config(
            Config::first_run(),
            Store::new(blocker.join("oclock/settings.json")),
            Box::new(NotifierProxy(notifier)),
            actions,
            Player::scripted(),
        );

        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        assert!(app.is_dirty());

        let _ = app.update(Message::Save);
        assert!(
            app.view.notice.is_some(),
            "a save that could not happen has to say so"
        );
        assert!(
            app.is_dirty(),
            "and the change must not be silently discarded"
        );

        let _ = app.update(Message::DismissNotice);
        assert!(app.view.notice.is_none());
    }

    #[test]
    fn the_configuration_survives_a_save_and_reload() {
        let (mut app, _scratch, _notifier) = harness("persist");

        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Swim".into()));
        let _ = app.update(Message::SaveAlarm);
        let _ = app.update(Message::AddCity("Europe/Paris".into()));
        let _ = app.update(Message::ToggleSeconds);
        assert!(app.is_dirty());

        let _ = app.update(Message::Save);
        assert!(!app.is_dirty(), "saving clears the dirty flag");

        // The reload re-reads the file, so the in-memory state is rebuilt from
        // what was actually written.
        let _ = app.update(Message::Reload);
        assert_eq!(app.alarms.len(), 1);
        assert_eq!(app.alarms.alarms()[0].label, "Swim");
        assert_eq!(app.world.len(), 4);
        assert!(!app.display().show_seconds);
    }

    #[test]
    fn the_sound_settings_toggle() {
        let (mut app, _scratch, _notifier) = harness("sound");
        let before = app.sound.enabled;
        let _ = app.update(Message::ToggleSound);
        assert_ne!(app.sound.enabled, before);

        let notify_before = app.sound.notify;
        let _ = app.update(Message::ToggleNotifications);
        assert_ne!(app.sound.notify, notify_before);
    }

    #[test]
    fn a_tick_advances_time_and_polls_the_schedule() {
        let (mut app, _scratch, _notifier) = harness("tick");
        let before = app.now();
        let _ = app.update(Message::Tick);
        assert!(
            app.now() >= before,
            "the wall clock is resampled each second"
        );
    }

    #[test]
    fn a_tick_resamples_the_clock_and_the_world_rows() {
        let (mut app, _scratch, _notifier) = harness("resample");
        let before = app.snapshot.now;

        // `update` measures its own delta from the monotonic clock, so the
        // resample cadence is driven through `tick` directly rather than by
        // waiting on real time. This is the path the tick subscription feeds,
        // and it is the one the message payload used to describe.
        let _ = app.tick(Duration::from_millis(1_200));

        assert_eq!(
            app.last_delta(),
            Duration::from_millis(1_200),
            "the tick carried its delta through to the application"
        );
        assert!(
            app.snapshot.now >= before,
            "the wall clock is resampled at the one-second boundary"
        );
        assert_eq!(
            app.snapshot.world.len(),
            app.world.len(),
            "every city has a row again after the resample"
        );
        for row in &app.snapshot.world {
            assert!(
                row.at.is_some(),
                "{} could not be placed",
                row.location.city
            );
        }
    }

    #[test]
    fn a_clock_jump_brings_the_measurements_forward() {
        let (mut app, _scratch, _notifier) = harness("jump");
        let preset = app.timers.presets()[3].id; // 25 minutes
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;
        let total = app.timers.get(id).expect("running").total;

        let before = app.timers.get(id).expect("running").elapsed(app.elapsed());
        assert!(before < Duration::from_secs(1), "it has just started");

        // Half an hour passes with nothing running: on a suspended machine
        // that is exactly what a timer should experience.
        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(1_800),
        )));

        let timer = app.timers.get(id).expect("still there");
        assert_eq!(
            timer.elapsed(app.elapsed()),
            total,
            "a half-hour gap finished a twenty-five minute timer"
        );
        assert_eq!(timer.state, crate::domain::timer::TimerState::Done);
    }

    #[test]
    fn a_backwards_clock_change_does_not_shorten_a_measurement() {
        let (mut app, _scratch, _notifier) = harness("rewind");
        let preset = app.timers.presets()[4].id;
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;

        let before = app.timers.get(id).expect("running").elapsed(app.elapsed());
        let laps_before = app.stopwatch.laps().to_vec();
        let _ = app.update(Message::Discontinuity(Discontinuity::Rewound(
            Duration::from_secs(600),
        )));
        let after = app
            .timers
            .get(id)
            .expect("still there")
            .elapsed(app.elapsed());
        assert!(
            after >= before,
            "a rewind must not make a timer run backwards"
        );
        assert_eq!(
            app.stopwatch.laps(),
            laps_before.as_slice(),
            "and must not advance one either"
        );
    }

    #[test]
    fn a_rewind_leaves_the_durations_exactly_where_they_were() {
        // The strong form of the same claim: not "did not go backwards", but
        // "did not move". A rewind used to be handed straight to `catch_up`,
        // which pushed a ten-minute-old timer ten minutes *further on*.
        let (mut app, _scratch, _notifier) = harness("rewind-exact");
        let preset = app.timers.presets()[4].id;
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;

        app.stopwatch.start(Duration::from_secs(1));
        app.stopwatch.lap(Duration::from_secs(2));
        app.stopwatch.start(Duration::from_secs(3));

        let timer = app.timers.get(id).expect("running").elapsed(app.elapsed());
        let stopwatch = app.stopwatch.elapsed(Duration::from_secs(3));

        let _ = app.update(Message::Discontinuity(Discontinuity::Rewound(
            Duration::from_secs(3_600),
        )));

        assert!(
            near(
                app.timers
                    .get(id)
                    .expect("still there")
                    .elapsed(app.elapsed()),
                timer
            ),
            "the timer has not moved"
        );
        assert!(
            near(app.stopwatch.elapsed(Duration::from_secs(3)), stopwatch),
            "the stopwatch has not moved"
        );
    }

    #[test]
    fn a_forward_jump_advances_a_measurement_by_what_was_missed_and_no_more() {
        let (mut app, _scratch, _notifier) = harness("forward-exact");
        let preset = app.timers.presets()[4].id;
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;

        // Monotonic progress so far, which has already been accounted for.
        let _ = app.tick(Duration::from_secs(1));
        let before = app.timers.get(id).expect("running").elapsed(app.elapsed());

        // The wall clock jumps ten seconds forward on top of it.
        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(10),
        )));
        let after = app.timers.get(id).expect("running").elapsed(app.elapsed());

        assert!(
            near(after - before, Duration::from_secs(10)),
            "eleven seconds have passed and one of them was already counted: {:?}",
            after - before
        );
    }

    #[test]
    fn a_second_jump_is_the_second_gap_and_not_the_first_one_again() {
        // Two gaps, reported as two gaps: neither is the other's. That is the
        // property the watchdog's anchoring gives — a gap is what the clocks
        // disagree about *since the last reading*, not a running total — and
        // `core::time` is where it is asserted on the watchdog itself.
        let (mut app, _scratch, _notifier) = harness("forward-twice");
        let preset = app.timers.presets()[4].id;
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;

        let before = app.timers.get(id).expect("running").elapsed(app.elapsed());

        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(10),
        )));
        let after_first = app.timers.get(id).expect("running").elapsed(app.elapsed());

        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(4),
        )));
        let after_second = app.timers.get(id).expect("running").elapsed(app.elapsed());

        assert!(
            near(after_first - before, Duration::from_secs(10)),
            "the first gap is ten seconds"
        );
        assert!(
            near(after_second - after_first, Duration::from_secs(4)),
            "and the second is four, not ten again: {:?}",
            after_second - after_first
        );
    }

    #[test]
    fn a_suspend_length_jump_brings_the_measurements_up_to_date() {
        // The suspend case: the machine slept for twenty minutes and the
        // monotonic clock did not move at all, so the whole gap is missed.
        let (mut app, _scratch, _notifier) = harness("suspend");
        // Ten minutes, so twenty minutes of sleep finishes it.
        let preset = app.timers.presets()[2].id;
        let _ = app.update(Message::StartPreset(preset));

        app.stopwatch.start(Duration::from_secs(1));
        let before = app.stopwatch.elapsed(Duration::from_secs(1));

        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(1_200),
        )));

        let after = app.stopwatch.elapsed(Duration::from_secs(1)) - before;
        assert!(
            near(after, Duration::from_secs(1_200)),
            "a stopwatch counts the sleep: {after:?}"
        );
        assert!(
            app.timers
                .timers()
                .iter()
                .any(|timer| timer.state == crate::domain::timer::TimerState::Done),
            "and a timer that ran out while the machine slept says so"
        );
    }

    #[test]
    fn a_resize_changes_the_layout() {
        let (mut app, _scratch, _notifier) = harness("resize");
        assert_eq!(app.view.layout, crate::app::view::Layout::Wide);

        let _ = app.update(Message::Resized(500.0, 700.0));
        assert_eq!(app.view.layout, crate::app::view::Layout::Stacked);

        let _ = app.update(Message::Resized(1200.0, 800.0));
        assert_eq!(app.view.layout, crate::app::view::Layout::Wide);
    }

    #[test]
    fn escape_closes_one_layer_at_a_time() {
        let (mut app, _scratch, _notifier) = harness("escape");

        let _ = app.update(Message::ToggleShortcuts);
        let _ = app.update(Message::Key(Key::Escape));
        assert!(!app.view.shortcuts);

        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::Key(Key::Escape));
        assert!(app.view.alarm_draft.is_none());
    }

    /// A synthetic Escape, as the window would deliver it.
    fn escape_event(modifiers: iced::keyboard::Modifiers) -> iced::Event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
        })
    }

    /// A synthetic window resize, as the window would deliver it.
    fn resized_event(width: f32, height: f32) -> iced::Event {
        iced::Event::Window(window::Event::Resized(iced::Size::new(width, height)))
    }

    #[test]
    fn the_shell_claims_only_the_keys_it_uses() {
        use iced::keyboard::{Key as Pressed, Modifiers};

        let plain = Modifiers::default();
        let ctrl = Modifiers::CTRL;

        assert!(matches!(
            key_press(Pressed::Character("3".into()), plain),
            Some(Message::Key(Key::Digit(3)))
        ));

        // A letter belongs to a field, not to the shell.
        assert!(key_press(Pressed::Character("a".into()), plain).is_none());
        // And a modified press is the desktop's, not OClock's.
        assert!(key_press(Pressed::Character("3".into()), ctrl).is_none());
    }

    #[test]
    fn escape_reaches_the_shell_from_the_raw_stream() {
        use iced::event::Status;
        use iced::keyboard::Modifiers;

        let window = window::Id::unique();

        for status in [Status::Ignored, Status::Captured] {
            // The captured half of this is the whole point. A focused field
            // takes Escape for itself, and a listener that only hears ignored
            // events never learns that the key was pressed at all — which is how
            // a dialog could sit there ignoring the Escape aimed at it.
            assert!(matches!(
                raw_event(escape_event(Modifiers::default()), status, window),
                Some(Message::Key(Key::Escape))
            ));
        }

        // A modified Escape is the desktop's, as everywhere else.
        assert!(raw_event(escape_event(Modifiers::CTRL), Status::Ignored, window).is_none());

        // Everything else stays where it was: another key, and Escape on its
        // way back up.
        assert!(raw_event(resized_event(500.0, 700.0), Status::Ignored, window).is_some());
        let released = iced::Event::Keyboard(iced::keyboard::Event::KeyReleased {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            location: iced::keyboard::Location::Standard,
            modifiers: Modifiers::default(),
        });
        assert!(raw_event(released, Status::Ignored, window).is_none());
    }

    #[test]
    fn one_escape_press_raises_one_message() {
        use iced::keyboard::Modifiers;

        // Escape is read from exactly one of the two listeners. Claimed by
        // both, a single press would raise two messages and close two layers at
        // once — a dialog and whatever was behind it.
        assert!(key_press(
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            Modifiers::default()
        )
        .is_none());

        let message = raw_event(
            escape_event(Modifiers::default()),
            iced::event::Status::Captured,
            window::Id::unique(),
        )
        .expect("escape is the shell's");
        assert!(matches!(message, Message::Key(Key::Escape)));
    }

    #[test]
    fn the_raw_stream_still_reports_resizes() {
        let message = raw_event(
            resized_event(500.0, 700.0),
            iced::event::Status::Ignored,
            window::Id::unique(),
        )
        .expect("a resize is the shell's");
        assert!(matches!(message, Message::Resized(500.0, 700.0)));
    }

    #[test]
    fn cancelling_the_timer_editor_throws_the_draft_away() {
        // The timer's own cancel. It used to be the alarm editor's cancel
        // message, which cleared the *alarm* draft and left the timer draft
        // behind: the dialog closed, and the next time it opened it came back
        // carrying what had been typed into it last time.
        let (mut app, _scratch, _notifier) = harness("cancel-timer");
        let _ = app.update(Message::NewTimer);
        assert!(app.view.timer_draft.is_some(), "the editor is open");

        let _ = app.update(Message::SetTimerName("Pasta".into()));
        let _ = app.update(Message::TimerDurationStep(60));
        assert!(app.view.has_modal());

        let _ = app.update(Message::CancelTimerEdit);
        assert!(
            app.view.timer_draft.is_none(),
            "the draft is gone, not just the dialog"
        );
        assert!(!app.view.has_modal());

        // And it stays gone: reopening starts from the defaults.
        let _ = app.update(Message::NewTimer);
        let draft = app.view.timer_draft.as_ref().expect("reopened");
        assert_eq!(draft.name, "");
        assert_eq!(draft.duration, Duration::from_secs(300));
    }

    #[test]
    fn cancelling_the_deletion_keeps_the_alarm() {
        // The other half of the same pair. Both cancels used to be one message,
        // so this one cleared the alarm draft — which is not what a dialog that
        // is asking about a deletion has any business touching.
        let (mut app, _scratch, _notifier) = harness("cancel-delete");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        let id = app.alarms.alarms()[0].id;

        let _ = app.update(Message::DeleteAlarm(id));
        assert_eq!(app.view.pending_delete, Some(id), "the question is up");

        let _ = app.update(Message::CancelDeleteAlarm);
        assert_eq!(app.view.pending_delete, None, "nothing is left to confirm");
        assert!(!app.view.has_modal());
        assert_eq!(
            app.alarms.alarms().len(),
            1,
            "and the alarm it was about is still there"
        );
    }

    #[test]
    fn each_cancel_only_withdraws_the_state_its_own_dialog_owns() {
        // The three dialogs hold three different pieces of state, and a cancel
        // that reached past its own would be a cancel nobody can predict. Each
        // one is opened with something in every other slot, so a cancel that
        // cleared the wrong thing has something to be caught doing.
        let (mut app, _scratch, _notifier) = harness("cancel-scope");

        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        let alarm = app.alarms.alarms()[0].id;
        let _ = app.update(Message::DeleteAlarm(alarm));

        // The alarm editor opens over the deletion question.
        let _ = app.update(Message::EditAlarm(alarm));
        assert!(app.view.alarm_draft.is_some());
        assert_eq!(app.view.pending_delete, Some(alarm));

        let _ = app.update(Message::CancelAlarmEdit);
        assert!(app.view.alarm_draft.is_none(), "the editor is gone");
        assert_eq!(
            app.view.pending_delete,
            Some(alarm),
            "and it took nothing else with it"
        );

        let _ = app.update(Message::CancelDeleteAlarm);
        assert_eq!(app.view.pending_delete, None);

        // The timer's editor is a separate draft again.
        let _ = app.update(Message::NewTimer);
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::CancelTimerEdit);
        assert!(app.view.timer_draft.is_none(), "the timer draft is gone");
        assert!(
            app.view.alarm_draft.is_some(),
            "and the alarm draft opened behind it is untouched"
        );
    }

    #[test]
    fn cancelling_a_dialog_that_is_not_open_changes_nothing() {
        // A cancel with nothing to cancel must not start closing something. The
        // dialog's own exit is a state change of its own, and asking for it
        // when there is no dialog to close leaves the window animating for
        // nothing.
        let (mut app, _scratch, _notifier) = harness("cancel-nothing");

        for cancel in [
            Message::CancelAlarmEdit,
            Message::CancelDeleteAlarm,
            Message::CancelTimerEdit,
        ] {
            let _ = app.update(cancel.clone());
            assert!(
                !app.view.has_modal(),
                "{} with nothing open has nothing to close",
                cancel.describe()
            );
        }

        // And opening one, then cancelling it, is the case that does animate.
        let _ = app.update(Message::NewTimer);
        let _ = app.update(Message::CancelTimerEdit);
        assert!(!app.view.has_modal());
    }

    #[test]
    fn removing_the_top_lap_takes_the_newest_one() {
        // The rows are drawn newest first, so the top row's remove button must
        // remove the lap that row is showing. Reading the drawn position as the
        // lap's own index removes the oldest lap from the newest row.
        let (mut app, _scratch, _notifier) = harness("remove-lap-top");

        for run in 1..=3u64 {
            app.stopwatch.start(Duration::from_secs(run));
            app.stopwatch
                .lap(Duration::from_secs(run) + Duration::from_millis(500));
        }
        let first = app.stopwatch.laps()[0].total;
        let newest = app.stopwatch.laps()[2].total;

        // The top row is the third lap, and its button is the first of the three
        // in the focus list.
        app.view.page = 4;
        let index = app.page_focus(app.lap_row_base()).expect("a lap row");
        assert_eq!(
            app.focus_at(index),
            Some(Message::RemoveLap(2)),
            "the top row names the newest lap"
        );

        let _ = app.update(Message::RemoveLap(2));
        assert_eq!(app.stopwatch.laps().len(), 2);
        assert_eq!(
            app.stopwatch.laps()[0].total,
            first,
            "the oldest is still first"
        );
        assert!(
            !app.stopwatch.laps().iter().any(|lap| lap.total == newest),
            "and the newest is the one that went"
        );
    }

    #[test]
    fn a_lap_can_be_removed_and_the_rest_keep_their_meaning() {
        // The remove button on a lap row used to do nothing at all: the message
        // arrived, the handler took the index and did nothing with it.
        let (mut app, _scratch, _notifier) = harness("remove-lap");

        // Three marks at known instants, so every split is known.
        app.stopwatch.start(Duration::from_secs(0));
        app.stopwatch.lap(Duration::from_secs(2));
        app.stopwatch.lap(Duration::from_secs(5));
        app.stopwatch.lap(Duration::from_secs(9));
        assert_eq!(app.stopwatch.lap_count(), 3);

        let _ = app.update(Message::RemoveLap(1));

        let laps = app.stopwatch.laps();
        assert_eq!(laps.len(), 2, "the lap is gone");
        assert_eq!(laps[0].index, 1);
        assert_eq!(laps[1].index, 2, "and the numbering has no hole in it");
        assert_eq!(
            laps[1].total,
            Duration::from_secs(9),
            "the marks are the marks"
        );

        let _ = app.update(Message::RemoveLap(0));
        assert_eq!(app.stopwatch.lap_count(), 1);

        let _ = app.update(Message::RemoveLap(0));
        assert_eq!(app.stopwatch.lap_count(), 0, "the last one goes too");

        // And an index that names no lap changes nothing at all.
        let _ = app.update(Message::RemoveLap(7));
        assert_eq!(app.stopwatch.lap_count(), 0);
    }

    #[test]
    fn a_removed_lap_leaves_the_rest_numbered_and_split_correctly() {
        let mut clock = Stopwatch::new();
        clock.start(Duration::from_secs(0));
        clock.lap(Duration::from_secs(2));
        clock.lap(Duration::from_secs(5));
        clock.lap(Duration::from_secs(9));

        assert!(clock.remove_lap(1));
        assert_eq!(
            clock
                .laps()
                .iter()
                .map(|lap| (lap.index, lap.total, lap.split))
                .collect::<Vec<_>>(),
            vec![
                (1, Duration::from_secs(2), Duration::from_secs(2)),
                (2, Duration::from_secs(9), Duration::from_secs(7)),
            ],
            "the split is the gap between the marks that remain"
        );

        assert!(
            !clock.remove_lap(2),
            "past the end there is nothing to remove"
        );
        assert!(!clock.remove_lap(usize::MAX));
        assert_eq!(
            clock.lap_count(),
            2,
            "and it says so without changing anything"
        );
    }

    #[test]
    fn escape_closes_every_dialog_that_is_open() {
        // The three dialogs that hold a field are the ones a focused field can
        // eat the key in, and the one that holds none has to behave the same
        // way: Escape means "close this", whatever this is.
        for open in [
            Message::NewAlarm,
            Message::NewTimer,
            Message::ToggleCityPicker,
            Message::ToggleShortcuts,
        ] {
            let (mut app, _scratch, _notifier) = harness("escape-dialog");
            let _ = app.update(open.clone());
            assert!(app.view.has_overlay(), "the dialog opened");

            let _ = app.update(Message::Key(Key::Escape));
            assert!(
                !app.view.has_overlay(),
                "{} was not closed by Escape",
                open.describe()
            );
        }
    }

    #[test]
    fn escape_closes_a_dialog_with_the_keyboard_in_its_field() {
        // The order a person actually presses them in: open the editor, put
        // the pointer in the name field, type, then reach for Escape. The field
        // has the focus and therefore takes the key, so the dialog used to stay
        // exactly where it was.
        let (mut app, _scratch, _notifier) = harness("escape-typing");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Gym".into()));
        assert!(app.view.typing, "the field has the keyboard");

        let _ = app.update(Message::Key(Key::Escape));
        assert!(
            app.view.alarm_draft.is_none(),
            "one press closed the editor"
        );
        assert!(!app.view.typing, "and the field let go");
    }

    #[test]
    fn a_press_a_field_took_still_closes_the_dialog() {
        // The whole path, from a key the window delivered to the state the
        // dialog leaves behind: a *captured* Escape — which is what a focused
        // field hands the runtime — is still the shell's, and still closes the
        // dialog in one press. The two halves are asserted apart because
        // either can be right while the other is wrong, and only together do
        // they make the key work.
        use iced::keyboard::Modifiers;

        let (mut app, _scratch, _notifier) = harness("escape-end-to-end");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Gym".into()));

        let message = raw_event(
            escape_event(Modifiers::default()),
            iced::event::Status::Captured,
            window::Id::unique(),
        )
        .expect("a key a field captured is still the shell's");

        let _ = app.update(message);
        assert!(app.view.alarm_draft.is_none(), "the editor closed");
    }

    #[test]
    fn escape_closes_the_city_picker_with_what_was_typed_in_it() {
        // The picker's field is the first thing a person clicks, so this is the
        // common case rather than the corner one.
        let (mut app, _scratch, _notifier) = harness("escape-picker");
        let _ = app.update(Message::ToggleCityPicker);
        let _ = app.update(Message::SearchCities("Tok".into()));

        let _ = app.update(Message::Key(Key::Escape));
        assert!(!app.view.city_picker);
        assert!(
            app.view.query.is_empty(),
            "closing forgets what was typed, as the button does"
        );
    }

    #[test]
    fn escape_answers_a_delete_confirmation_without_deleting() {
        let (mut app, _scratch, _notifier) = harness("escape-delete");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Wake up".into()));
        let _ = app.update(Message::SaveAlarm);
        let id = app.alarms.alarms()[0].id;

        let _ = app.update(Message::DeleteAlarm(id));
        assert_eq!(app.view.pending_delete, Some(id));

        let _ = app.update(Message::Key(Key::Escape));
        assert!(
            app.view.pending_delete.is_none(),
            "the question was withdrawn"
        );
        assert!(app.alarms.get(id).is_some(), "and nothing was deleted");
    }

    #[test]
    fn a_field_keeps_the_keyboard() {
        let (mut app, _scratch, _notifier) = harness("typing");
        app.view.typing = true;

        // Typing a digit into a field must not jump sections.
        let _ = app.update(Message::Key(Key::Digit(3)));
        assert_eq!(app.view.page, 0, "the field has the keyboard");

        // Escape is how a field is left, so it still works.
        let _ = app.update(Message::Key(Key::Escape));
        assert!(!app.view.typing, "the field let go");
    }

    #[test]
    fn typing_into_a_field_arms_the_guard() {
        let (mut app, _scratch, _notifier) = harness("arm");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SetAlarmName("Gym".into()));
        assert!(app.view.typing);

        let _ = app.update(Message::Key(Key::Digit(2)));
        assert_eq!(app.view.page, 0, "a dialog holds the page anyway");
    }

    #[test]
    fn the_page_shortcuts_act_on_the_page_that_is_showing() {
        let (mut app, _scratch, _notifier) = harness("shortcuts");

        // On the stopwatch, S, L and R do what the stopwatch says they do.
        app.view.page = 4;
        let _ = app.update(Message::Key(Key::Start));
        assert!(app.stopwatch.is_running());
        let _ = app.update(Message::Key(Key::Lap));
        assert_eq!(app.stopwatch.lap_count(), 1);
        let _ = app.update(Message::Key(Key::Reset));
        assert_eq!(app.stopwatch.lap_count(), 0);
        assert!(!app.stopwatch.is_running());

        // N opens whatever the page can make — but not while a dialog is up,
        // because then the dialog owns the keyboard.
        app.view.page = 2;
        let _ = app.update(Message::Key(Key::New));
        assert!(app.view.alarm_draft.is_some());

        let _ = app.update(Message::Key(Key::New));
        assert!(
            app.view.alarm_draft.is_some(),
            "a second N does not stack dialogs"
        );

        let _ = app.update(Message::Key(Key::Escape));
        app.view.page = 3;
        let _ = app.update(Message::Key(Key::New));
        assert!(app.view.timer_draft.is_some());
    }

    #[test]
    fn a_page_shortcut_means_nothing_on_the_wrong_page() {
        let (mut app, _scratch, _notifier) = harness("wrong-page");
        app.view.page = 1; // World: nothing to start, lap or reset.

        let _ = app.update(Message::Key(Key::Start));
        assert!(!app.stopwatch.is_running());
        assert_eq!(app.stopwatch.lap_count(), 0);
        assert!(app.view.alarm_draft.is_none());
        assert!(app.view.timer_draft.is_none());
    }

    #[test]
    fn a_digit_switches_section_unless_a_dialog_is_open() {
        let (mut app, _scratch, _notifier) = harness("digits");
        let _ = app.update(Message::Key(Key::Digit(3)));
        assert_eq!(app.view.page, 2);

        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::Key(Key::Digit(1)));
        assert_eq!(app.view.page, 2, "a dialog holds the page still");
    }

    #[test]
    fn tab_moves_the_focus_and_enter_activates_it() {
        let (mut app, _scratch, _notifier) = harness("focus");
        app.view.page = 2;

        let count = app.focus_count();
        assert!(count > 0);

        // The focus walks the whole window: the shell's controls come first, so
        // the first Tab lands on the first destination rather than on the page.
        let _ = app.update(Message::FocusNext);
        assert_eq!(app.view.focus, Some(0));

        let _ = app.update(Message::FocusNext);
        assert_eq!(app.view.focus, Some(1));

        let _ = app.update(Message::FocusPrevious);
        assert_eq!(app.view.focus, Some(0));

        let _ = app.update(Message::FocusPrevious);
        assert_eq!(
            app.view.focus,
            Some(count - 1),
            "and backwards from the first control wraps to the last"
        );

        // The page's own controls are the ones after the shell's, and the first
        // of them is the header's new-alarm button.
        let index = app.page_focus(0).expect("the page has controls");
        assert!(index > 0, "which is not the first control in the window");

        app.view.focus = Some(index);
        let _ = app.update(Message::ActivateFocus);
        assert!(
            app.view.alarm_draft.is_some(),
            "activating it opens the editor, as clicking it would"
        );
    }

    #[test]
    fn a_modal_swallows_the_pages_focus() {
        // While a dialog is up the keyboard is talking to the dialog. An index
        // that belongs to a page behind a scrim activates nothing rather than
        // reaching through it.
        let (mut app, _scratch, _notifier) = harness("focus-modal");
        app.view.page = 2;
        let _ = app.update(Message::DeleteAlarm(1));

        let page_index = app.page_focus(0).expect("the page has controls");
        assert!(page_index > 0);

        app.view.focus = Some(page_index);
        let _ = app.update(Message::ActivateFocus);
        assert!(
            app.view.pending_delete.is_some(),
            "a page control cannot be pressed through a dialog"
        );
    }

    #[test]
    fn every_page_reports_its_actions_in_reading_order() {
        let (app, _scratch, _notifier) = harness("actions");
        assert!(!app.clock_page_actions().is_empty());
        assert!(!app.world_page_actions().is_empty());
        assert!(!app.alarm_page_actions().is_empty());
        assert!(!app.timer_page_actions().is_empty());
        assert!(!app.stopwatch_page_actions().is_empty());
    }

    #[test]
    fn the_focus_ring_lands_on_a_control_that_exists() {
        let (mut app, _scratch, _notifier) = harness("ring");
        for page in 0..5 {
            app.view.page = page;
            let count = app.focus_count();
            assert!(count > 0, "page {page} offers no focusable control");
        }
    }

    #[test]
    fn a_ringing_alarm_can_be_snoozed_or_dismissed() {
        let (mut app, _scratch, _notifier) = harness("ringing");
        let preset = app.timers.presets()[0].id;
        let _ = preset;

        // An alarm due one second ago, polled, rings.
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        let id = app.alarms.alarms()[0].id;
        app.alarms.snooze(id, app.snapshot.now, None);
        app.alarms.get_mut(id).expect("the alarm").snooze_minutes = 9;

        // Force the schedule to be due.
        let soon = app.snapshot.now + chrono::Duration::seconds(1);
        for event in app.alarms.poll(soon) {
            let _ = app.update(match event {
                crate::domain::alarm::AlarmEvent::Ringing { alarm, .. } => {
                    Message::SnoozeAlarm(alarm.id)
                }
                crate::domain::alarm::AlarmEvent::Missed { alarm, .. } => {
                    Message::AcknowledgeMissed(alarm.id)
                }
            });
        }
    }

    #[test]
    fn the_configuration_written_to_disk_is_the_whole_state() {
        let (mut app, scratch, _notifier) = harness("config");
        let written = app.config();
        let _ = app.update(Message::Save);
        let text = std::fs::read_to_string(scratch.0.join("settings.json")).expect("written");
        let document: serde_json::Value = serde_json::from_str(&text).expect("parses");

        assert!(document["alarms"].is_array());
        assert!(document["world"]["locations"].is_array());
        assert!(document["timers"]["presets"].is_array());
        assert_eq!(
            document["clock"]["show_seconds"],
            serde_json::json!(written.clock.show_seconds)
        );
    }

    #[test]
    fn the_first_frame_already_shows_the_world_clock_it_is_showing_now() {
        // The constructor used to build the world rows by hand, with
        // placeholders: no daylight-saving flag, no difference from the viewer's
        // own zone, no date, every card reading "Today". The first frame the
        // window ever drew was wrong, and it was still on screen a second later,
        // when the resample replaced it with the real thing.
        let (mut app, _scratch, _notifier) = harness("first-frame");

        let first = app
            .snapshot
            .world
            .iter()
            .map(|row| {
                (
                    row.location.city.clone(),
                    row.offset.clone(),
                    row.abbreviation.clone(),
                    row.daylight_saving,
                    row.offset_from_local,
                    row.day_relation,
                    row.date.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert!(!first.is_empty(), "the default configuration has cities");

        app.refresh_snapshot();
        let refreshed = app
            .snapshot
            .world
            .iter()
            .map(|row| {
                (
                    row.location.city.clone(),
                    row.offset.clone(),
                    row.abbreviation.clone(),
                    row.daylight_saving,
                    row.offset_from_local,
                    row.day_relation,
                    row.date.clone(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            first, refreshed,
            "the initial rows and the refreshed ones are the same rows"
        );
    }

    #[test]
    fn resetting_the_stopwatch_keeps_the_laps_when_the_preference_says_so() {
        // `keep_laps` was read from the configuration and then written back as a
        // hardcoded `false`, so the stored preference had no effect at all and
        // could not even survive a save.
        let (mut app, _scratch, _notifier) = harness("keep-laps");

        app.keep_laps = true;
        let _ = app.update(Message::ToggleStopwatch);
        let _ = app.update(Message::LapStopwatch);
        assert_eq!(app.stopwatch.lap_count(), 1);

        let _ = app.update(Message::ResetStopwatch);
        assert_eq!(
            app.stopwatch.lap_count(),
            1,
            "the laps are a record of the session, and survive the run"
        );
        assert!(
            !app.stopwatch.is_running(),
            "while the run itself still starts again"
        );
        assert!(
            app.config().timers.keep_laps,
            "and the preference round-trips"
        );

        app.keep_laps = false;
        let _ = app.update(Message::ResetStopwatch);
        assert_eq!(
            app.stopwatch.lap_count(),
            0,
            "unless the user said otherwise"
        );
        assert!(!app.config().timers.keep_laps);
    }

    #[test]
    fn the_stored_lap_preference_is_read_at_startup() {
        let scratch = Scratch::new("keep-laps-config");
        let path = scratch.0.join("settings.json");
        let store = Store::new(path.clone());
        let mut config = Config::first_run();
        config.timers.keep_laps = true;

        let app = app_from(config, store);
        assert!(app.keep_laps, "the configuration is the authority");

        // And it survives the round trip through the document.
        let written = app.config();
        std::fs::write(&path, serde_json::to_string_pretty(&written).unwrap()).unwrap();
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"keep_laps\": true"));
    }

    #[test]
    fn a_notification_action_maps_onto_the_same_message_as_the_button() {
        let snooze = on_action(Chosen {
            key: "alarm:7".into(),
            action: "snooze".into(),
        });
        assert!(snooze
            .iter()
            .any(|message| message.describe() == "snooze alarm"));

        let dismiss = on_action(Chosen {
            key: "alarm:7".into(),
            action: DISMISSED.into(),
        });
        assert!(dismiss
            .iter()
            .any(|message| message.describe() == "dismiss alarm"));

        assert!(on_action(Chosen {
            key: "timer:3".into(),
            action: "dismiss".into(),
        })
        .is_empty());
        assert!(on_action(Chosen {
            key: "nonsense".into(),
            action: "dismiss".into(),
        })
        .is_empty());
    }

    #[test]
    fn the_dismiss_button_dismisses_instead_of_snoozing() {
        // The dismiss button on a ringing alarm's notification carries the id
        // "dismiss". That is not the same string as DISMISSED ("dismissed", the
        // value a closed-without-choice notification arrives as), so the two
        // used to be distinguished — and the button fell through to snooze.
        let dismissed = on_action(Chosen {
            key: "alarm:7".into(),
            action: "dismiss".into(),
        });
        assert_eq!(
            dismissed
                .iter()
                .filter(|m| m.describe() == "dismiss alarm")
                .count(),
            1,
        );
        assert!(
            dismissed.iter().all(|m| m.describe() != "snooze alarm"),
            "a dismiss press must not snooze"
        );

        // Both spellings of "dismissed" land on the same message, so the button
        // and the closed-notification case cannot disagree.
        assert_eq!(
            on_action(Chosen {
                key: "alarm:7".into(),
                action: "dismiss".into(),
            }),
            on_action(Chosen {
                key: "alarm:7".into(),
                action: DISMISSED.into(),
            }),
        );
    }

    /// Puts a message on the application's action channel, the way the bridge
    /// worker does.
    fn send(app: &OClock, message: Message) {
        app.action_channel
            .lock()
            .expect("lock")
            .sender
            .unbounded_send(message)
            .expect("the receiver is still there");
    }

    /// Polls a stream once, with a waker that does nothing.
    ///
    /// The runtime's waker is what tells a pending stream that something has
    /// arrived; a test drives the stream by polling it, so the waker has nothing
    /// to wake and nothing to be done.
    fn poll_once<S: futures::Stream<Item = Message> + Unpin>(
        stream: &mut S,
    ) -> Poll<Option<Message>> {
        let mut context = Context::from_waker(Waker::noop());
        std::pin::Pin::new(stream).poll_next(&mut context)
    }

    #[test]
    fn the_action_stream_survives_an_empty_channel() {
        // The subscription is built the moment the window opens, and a user who
        // has not been woken up yet has produced nothing. That must leave the
        // stream *pending*: an empty channel read as "finished" ended the
        // subscription on the first frame, and a stream that has ended is never
        // polled again — so no notification action could ever arrive.
        let (app, _scratch, _notifier) = harness("action-stream");
        let mut stream = app.action_stream();
        assert!(
            poll_once(&mut stream).is_pending(),
            "an empty channel is a stream waiting, not a stream finished"
        );
    }

    #[test]
    fn the_action_stream_delivers_a_later_action() {
        let (app, _scratch, _notifier) = harness("action-stream-later");
        let mut stream = app.action_stream();
        assert!(poll_once(&mut stream).is_pending());

        send(&app, Message::SnoozeAlarm(7));
        assert_eq!(
            poll_once(&mut stream),
            Poll::Ready(Some(Message::SnoozeAlarm(7))),
            "an action sent later arrives on the same stream"
        );
    }

    #[test]
    fn the_action_stream_keeps_delivering() {
        // One action must not consume the subscription: the second has to come
        // through the same stream, and a third.
        let (app, _scratch, _notifier) = harness("action-stream-many");
        let mut stream = app.action_stream();

        for action in [
            Message::SnoozeAlarm(1),
            Message::DismissAlarm(1),
            Message::ToggleStopwatch,
        ] {
            send(&app, action.clone());
            assert_eq!(
                poll_once(&mut stream),
                Poll::Ready(Some(action.clone())),
                "{} arrives on the same subscription",
                action.describe()
            );
        }

        assert!(
            poll_once(&mut stream).is_pending(),
            "and then it waits again rather than ending"
        );
    }

    #[test]
    fn a_notification_action_reaches_the_application() {
        // The whole bridge, end to end: the recorder hands the action to the
        // channel it was built with, the worker thread reads it and maps it to a
        // message, and the stream the subscription reads carries that message
        // out. The test harness used to take the receiver *before* the
        // application did, which swapped in an empty channel underneath the
        // worker — every action was dropped, and any test asserting on one
        // would have passed for the wrong reason.
        let (app, _scratch, notifier) = harness("notification-bridge");

        notifier.choose("alarm:9", "snooze");

        let mut stream = app.action_stream();
        let mut received = None;
        for _ in 0..400 {
            if let Poll::Ready(Some(message)) = poll_once(&mut stream) {
                received = Some(message);
                break;
            }
            // The worker is a thread; give it a moment to be scheduled.
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            received,
            Some(Message::SnoozeAlarm(9)),
            "the action arrives as the message the snooze button sends"
        );
    }

    #[test]
    fn the_action_stream_ends_when_the_channel_closes() {
        // The one thing that does end it. A stream that cannot end is a leak;
        // a stream that ends on an empty read is a dead subscription.
        let (app, _scratch, _notifier) = harness("action-stream-closed");
        let mut stream = app.action_stream();

        send(&app, Message::SnoozeAlarm(3));
        assert!(matches!(poll_once(&mut stream), Poll::Ready(Some(_))));

        // Every sender gone: what the worker thread dropping out of the loop
        // looks like from in here.
        let sender = std::mem::replace(
            &mut app.action_channel.lock().expect("lock").sender,
            futures::channel::mpsc::unbounded().0,
        );
        drop(sender);

        assert_eq!(
            poll_once(&mut stream),
            Poll::Ready(None),
            "a closed channel ends the stream, once"
        );
    }

    #[test]
    fn a_finished_timer_announces_itself_once() {
        let (mut app, _scratch, notifier) = harness("complete");
        let preset = app.timers.presets()[0].id;
        let _ = app.update(Message::StartPreset(preset));
        let id = app.timers.timers()[0].id;

        // Jump past the end of the countdown.
        app.timers.catch_up(Duration::from_secs(120));
        let _ = app.update(Message::Tick);
        let _ = app.update(Message::Tick);

        let shown = notifier.shown.lock().expect("lock");
        let timer_notifications = shown
            .iter()
            .filter(|(key, _)| key.starts_with("timer:"))
            .count();
        assert_eq!(timer_notifications, 1, "a timer announces itself once");
        assert!(!app.timers.get(id).expect("still there").state.is_running());
    }

    #[test]
    fn the_application_renders_in_every_page_and_layout() {
        // A fresh application per combination: a page's content depends on the
        // shape of the state, and every page has to survive every layout.
        for page in 0..5 {
            for layout in [
                crate::app::view::Layout::Wide,
                crate::app::view::Layout::Compact,
                crate::app::view::Layout::Stacked,
            ] {
                let (mut app, _scratch, _notifier) = harness("render");
                app.view.page = page;
                app.view.layout = layout;
                let _: Element<'static, Message> = app.view();
            }
        }
    }

    #[test]
    fn every_page_survives_every_right_to_left_language() {
        // The three right-to-left languages in the catalogue, on every page and
        // in every layout. A page builds its rows by name — the leading edge,
        // the trailing edge — and a name that no page asks for is a name that
        // silently keeps the left-to-right order.
        let rtl = Language::ALL
            .iter()
            .copied()
            .filter(|language| language.is_rtl())
            .collect::<Vec<_>>();
        assert_eq!(
            rtl,
            vec![Language::Arabic, Language::Persian, Language::Urdu],
            "the catalogue's right-to-left languages have changed"
        );

        for language in rtl {
            for page in 0..5 {
                for layout in [
                    crate::app::view::Layout::Wide,
                    crate::app::view::Layout::Compact,
                    crate::app::view::Layout::Stacked,
                ] {
                    let (mut app, _scratch, _notifier) = harness("rtl");
                    let _ = app.update(Message::ChooseLanguage(language));
                    assert!(
                        app.catalog.is_rtl(),
                        "{language:?} should read right to left"
                    );
                    app.view.page = page;
                    app.view.layout = layout;
                    let _: Element<'static, Message> = app.view();
                }
            }
        }
    }

    #[test]
    fn every_right_to_left_language_survives_every_dialog() {
        // A dialog is the one place the body is scrolled, and it is the one
        // place a text field is typed into. Both have a way of going wrong that
        // a page cannot show.
        for language in [Language::Persian, Language::Arabic, Language::Urdu] {
            let (mut app, _scratch, _notifier) = harness("rtl-dialogs");
            let _ = app.update(Message::ChooseLanguage(language));
            app.view.page = 2;
            let draft = AlarmDraft::new(1, 7, 30);
            let _: Element<'static, Message> =
                crate::ui::pages::alarms::editor(&app, Palette::DARK, &draft, 1.0);
            let _: Element<'static, Message> = crate::ui::pages::timer::editor(
                &app,
                Palette::DARK,
                &TimerDraft::new("شاي", std::time::Duration::from_secs(300)),
                1.0,
            );
            let _: Element<'static, Message> =
                crate::ui::pages::world::picker(&app, Palette::DARK, 1.0);
        }
    }

    #[test]
    fn the_stopwatch_survives_a_clock_jump() {
        let (mut app, _scratch, _notifier) = harness("swjump");
        let _ = app.update(Message::ToggleStopwatch);
        let before = app.stopwatch.elapsed(app.elapsed());

        let _ = app.update(Message::Discontinuity(Discontinuity::Forwarded(
            Duration::from_secs(600),
        )));
        assert!(
            app.stopwatch.elapsed(app.elapsed()) >= before,
            "a clock jump must not move a stopwatch backwards"
        );
    }

    #[test]
    fn the_alarm_schedule_survives_a_timezone_change() {
        let (mut app, _scratch, _notifier) = harness("tz");
        let _ = app.update(Message::NewAlarm);
        let _ = app.update(Message::SaveAlarm);
        assert!(app.alarms.next_up().is_some());

        app.on_timezone_change("Asia/Tokyo".parse().expect("known zone"));
        assert!(
            app.alarms.next_up().is_some(),
            "the schedule was recomputed"
        );
    }

    #[test]
    fn a_reloaded_configuration_replaces_every_feature() {
        let (mut app, _scratch, _notifier) = harness("reloadall");
        let _ = app.update(Message::Save);
        app.stopwatch = Stopwatch::new();
        let _ = app.update(Message::Reload);
        assert_eq!(app.stopwatch.lap_count(), 0);
        assert_eq!(app.world.locations().len(), 3);
    }

    #[test]
    fn a_finished_page_transition_drops_the_page_underneath() {
        // The regression this guards is expensive rather than visible: the
        // leaving page is drawn *under* the one that replaced it, so a
        // cross-fade that never settles leaves both being laid out and drawn
        // for the rest of the session and nobody can see why the machine is
        // busy.
        let (mut app, _scratch, _notifier) = harness("pageleave");
        let _ = app.update(Message::Navigate(2));
        assert_eq!(app.view.leaving, Some(0), "the transition started");

        let mut guard = 0;
        while app.view.leaving.is_some() && guard < 400 {
            std::thread::sleep(motion::FRAME);
            let _ = app.update(Message::Tick);
            guard += 1;
        }

        assert!(
            guard < 400,
            "the transition never finished after {} frames",
            guard
        );
        assert_eq!(app.view.leaving, None, "the old page is still being drawn");
        assert_eq!(app.view.page, 2);
        assert!((app.view.entering_progress - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn a_settled_page_stays_settled() {
        // And it has to *stay* settled: the tween must not be re-stamped on
        // every tick, or the leaving page would be re-attached with nothing to
        // show for it.
        let (mut app, _scratch, _notifier) = harness("pagestay");
        let _ = app.update(Message::Navigate(1));
        for _ in 0..80 {
            std::thread::sleep(motion::FRAME);
            let _ = app.update(Message::Tick);
        }
        assert_eq!(app.view.leaving, None);
        for _ in 0..10 {
            std::thread::sleep(motion::FRAME);
            let _ = app.update(Message::Tick);
        }
        assert_eq!(app.view.leaving, None, "a finished transition restarted");
    }

    #[test]
    fn the_clock_page_asks_for_frames_so_the_second_hand_sweeps() {
        // The face reads the wall clock as it draws, so the frame rate is what
        // decides how finely it sweeps. It must not be the question the
        // autosave asks, though — or the configuration would never be written
        // while the clock is on screen.
        let (mut app, _scratch, _notifier) = harness("frames");
        app.show_analog = true;
        let _ = app.update(Message::Navigate(0));

        // Let the cross-fade finish, so the only thing left is the sweep.
        for _ in 0..80 {
            std::thread::sleep(motion::FRAME);
            let _ = app.update(Message::Tick);
        }
        assert!(!app.is_animating(), "the cross-fade has not finished");

        assert!(app.needs_frames(), "the sweep was left at the idle rate");
        assert!(
            !app.is_animating(),
            "a sweeping clock must not be mistaken for a transition"
        );

        // And with the face turned off it goes back to the cheap rate.
        app.show_analog = false;
        assert!(!app.needs_frames(), "the page never goes idle");
    }
}
