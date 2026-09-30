# OClock

**OClock** is the clock application for **ODS-os**: a clock, a world clock,
alarms, timers and a stopwatch in one window.

It is a first-party system application rather than a utility script, so it is
built like one — a Rust and [Iced](https://iced.rs) desktop application, a
single design system, motion that communicates rather than decorates, and a
timing model that does not depend on how often the window happens to redraw.

---

## Contents

- [Features](#features)
- [Themes](#themes)
- [Languages and right-to-left support](#languages-and-right-to-left-support)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Using it](#using-it)
- [Building and running](#building-and-running)
- [Installing](#installing)
- [Runtime requirements](#runtime-requirements)
- [Where your settings live](#where-your-settings-live)
- [Project structure](#project-structure)
- [Testing and verification](#testing-and-verification)
- [Licence](#licence)

---

## Features

| Section | What it is |
|---|---|
| **Clock** | The local time, at a glance and to the second, with the date, the timezone, daylight-saving state and a continuously sweeping second. An analog dial is available. 12- or 24-hour, seconds on or off, and it follows the desktop's light/dark preference or the one you choose. |
| **World** | Any number of cities, each on its own card with an analog face, its UTC offset, its zone abbreviation and how far ahead or behind you it is. Search by city, country or zone name. Ordered by hand, alphabetically, or by offset. |
| **Alarms** | Once, daily, or on chosen weekdays; each with a name, a sound and a snooze length. They survive sleep, resume, a changed system clock and a changed timezone. One-time alarms disarm themselves; a snooze survives a timezone change because it is a promise about elapsed time, not a wall clock reading. |
| **Timer** | Countdown timers with a progress ring, start and pause, several at once, a preset strip, and a completion sound and notification. |
| **Stopwatch** | Centisecond resolution with lap times, fastest and slowest highlighted. |

**Alarms** repeat `Once`, `Daily` or on a set of weekdays, and each carries its
own name, one of four sounds and its own snooze length. A missed alarm is
reported in the window on the next launch rather than being dropped.

**Timers** are independent of one another: several can run at once, each is
paused and resumed on its own, and a preset strip starts one without opening
the editor. A countdown that reaches zero while the window is closed finishes
on the next tick, because elapsed time is recomputed from a monotonic clock
rather than counted down.

**Sounds** are synthesised rather than sampled: each of the four is a short
recipe of tones and an envelope, rendered to PCM at the output device's own
sample rate. OClock therefore ships no audio assets, and the result is
identical on every machine.

**The world clock** is built from the system's own timezone database. OClock
ships no city list: a place is named by its timezone identifier, and the set of
identifiers comes from the tzdata your distribution installed — so the picker
offers the same places the rest of the desktop knows about, and keeps up with
it. On a machine with no tzdata the picker says so rather than inventing a list.

## Themes

Three choices, in the sidebar: **Match system**, **Light** and **Dark**. The
default follows the desktop's setting, so a machine that switches to dark at
dusk takes the application with it.

Both appearances are complete. Every colour in the design system is a
*semantic* role — `surface`, `text_faint`, `accent_soft`, `dial_hand` — and
light and dark each fill in every one of them, so a component cannot ask for a
role that only exists in one of them. Contrast is measured against the
surfaces text is actually drawn on, in both appearances, by tests that fail
the build rather than the user's eyes.

Switching themes is a dissolve rather than a colour cross-fade. Interpolating
a light palette into a dark one drags every colour towards the middle, and at
the halfway point the canvas is a mid grey and the body text is a slightly
different mid grey. No easing can avoid it, because any continuous path
between a light theme and a dark one must pass through the foreground meeting
its own background. So the content is covered by a veil of the new theme's
canvas colour, the palette is swapped while the veil is opaque, and the content
is revealed.

## Languages and right-to-left support

OClock speaks **twelve** languages, which is the complete set — the catalogue
is compiled in, so a new one is a table rather than a file to ship:

| | | | |
|---|---|---|---|
| English | Français | Deutsch | Español |
| Bahasa Indonesia | العربية | 中文 | 한국어 |
| فارسی | Русский | ไทย | اردو |

**A fresh installation starts in English**, whatever the machine's locale is. A
locale is an ambient guess about the computer, not a preference anyone
expressed, and the first run has no preference to honour. The choice is then
stored like any other setting and is never overwritten afterwards: pick a
language once and it survives every later launch, including upgrades.

Change it from the sidebar. The globe chip always names the language that
pressing it will move to, in **that language's own spelling** — someone
looking for their language does not recognise it written in the one they are
currently reading. The twelve are cycled in a fixed order, so the sequence is
the same whichever language it is cycled from.

**Arabic, Persian and Urdu** are written from the right, and the interface is
mirrored for them: rows hang from the right edge, the navigation rail sits on
the right, and icons that *name* a direction are mirrored while icons that mean
the same thing everywhere — a reset sweep, a pencil, a struck-through bell —
are not.

What is *not* mirrored is the data. A time, a date, a duration and a zone
abbreviation are the same reading in every language — `04:55:47`, `UTC-04:00`,
`EDT` — and are laid out so that they still read that way inside an Arabic
sentence.

No face is named. Each role asks for a *family* — a proportional one for text,
a tabular one for digits — which leaves the text shaper free to pick a face per
character, and so free to reach for a second one when the first has no glyph.
That single choice is the whole of Arabic, CJK, Korean, Thai and Cyrillic
support, and it is why adding a language is a matter of translating strings
rather than of shipping a font.

## Keyboard shortcuts

Every control is reachable from the keyboard and every one draws a visible
focus ring. Press <kbd>?</kbd> in the application for this list.

The controls are listed once, in reading order: the sections, the appearance
and language controls, and then the page's own. That one list is what draws the
rings *and* what <kbd>Enter</kbd> acts on, so a ring cannot land on a control
that does nothing. A dialog takes the list over while it is open — nothing
behind a dialog is reachable, so nothing behind it is numbered.

| Keys | Action |
|---|---|
| <kbd>1</kbd> – <kbd>5</kbd> | Go straight to a section |
| <kbd>Tab</kbd> / <kbd>Shift</kbd>+<kbd>Tab</kbd> | Move the focus forwards and back |
| <kbd>Enter</kbd> / <kbd>Space</kbd> | Activate what holds the focus |
| <kbd>Escape</kbd> | Close the topmost dialog, without saving |
| <kbd>S</kbd> | Start or pause the stopwatch |
| <kbd>L</kbd> | Record a lap |
| <kbd>R</kbd> | Reset the stopwatch or a timer |
| <kbd>N</kbd> | New alarm (Alarms), new timer (Timers) |
| <kbd>?</kbd> | Open and close this list |

OClock claims only unmodified presses. Anything with <kbd>Ctrl</kbd>,
<kbd>Alt</kbd> or <kbd>Super</kbd> held is deliberately left alone, so the
desktop's own <kbd>Ctrl</kbd>+<kbd>D</kbd>, <kbd>Ctrl</kbd>+<kbd>1</kbd> and the
rest keep working on the window behind.

Iced 0.13 has no focus traversal for buttons, so OClock has its own: each page
builds its list of focusable actions **in reading order** and reports the same
list for the focus ring, so the highlight and the activation can never disagree
about what comes next. A modal dialog holds the page still and takes
<kbd>Escape</kbd> first.

The layouts are continuous rather than stepped: the sidebar loses its labels
before it loses its marks, and below that it becomes a top bar, while cards
re-flow from a wrapping row. A world clock with thirty cities is a list, not a
grid that overflows.

## Using it

Launch **OClock** from your desktop's application menu. The window opens on the
clock, with the section rail down the side.

1. **Add a city.** Open *World* and press *Add city*, then search by city,
   country or zone name — `Tokyo`, `Japan`, `Asia/Tokyo` all find the same
   place. A pinned city leads the list whichever way the rest of it is ordered,
   and the rest can be ordered by hand, alphabetically, or by UTC offset.
2. **Set an alarm.** Open *Alarms* and press *New alarm*, or press
   <kbd>N</kbd>. Give it a name, set the time with the steppers, choose whether
   it repeats, and pick a sound. **Save** keeps it; **Cancel**, or
   <kbd>Escape</kbd>, closes the editor without keeping it.
3. **Start a timer.** Open *Timers* and press *New timer*, or <kbd>N</kbd>. The
   preset strip starts one straight away without opening the editor. Several
   timers run at once, each with its own ring.
4. **Run the stopwatch.** Open *Stopwatch* and press <kbd>S</kbd>. <kbd>L</kbd>
   records a lap, <kbd>R</kbd> resets.
5. **Change the language or the theme.** The globe chip in the sidebar cycles
   the language, always naming in the endonym of the language it will move to.
   The chip below it sets the theme: *Match system*, *Light* or *Dark*.

Everything is saved as you go. There is no *Apply* button, because there is
nothing to apply.

## Building and running

```bash
cargo build --release
./target/release/oclock
```

A development build is the same thing without optimisation:

```bash
cargo run
```

## Installing

`data/` holds the desktop integration — the launcher entry, the AppStream
metadata and the icon set — and a `Makefile` that installs them to the standard
freedesktop locations:

```bash
make -C data install                 # into /usr/local
make -C data install PREFIX=~/.local # into your own home
make -C data uninstall
```

That installs the binary to `$PREFIX/bin/oclock`, the desktop entry to
`$PREFIX/share/applications/org.odsos.OClock.desktop`, the metadata to
`$PREFIX/share/metainfo/`, and both icon variants into the `hicolor` theme.
A distribution package should use the same paths, because that is what makes
the launcher, the taskbar and a software centre find the application without
being told where it is. `DESTDIR` is honoured for staged builds.

| File | What it is |
|---|---|
| `data/org.odsos.OClock.desktop` | The launcher entry. Carries the name, the icon, the binary to run and the `Clock` category a launcher searches for. |
| `data/org.odsos.OClock.metainfo.xml` | AppStream metadata, so a software centre can describe the application rather than only list it. |
| `data/icons/hicolor/scalable/apps/org.odsos.OClock.svg` | **The** icon. Every raster size is rendered from this one file. |
| `data/icons/hicolor/*/apps/*.png` | The rendered sizes: 16, 22, 24, 32, 48, 64, 128, 256 and 512. |
| `data/icons/hicolor/symbolic/apps/org.odsos.OClock-symbolic.svg` | The monochrome variant a panel or a title bar recolours for itself. GLib asks for this one by name whether or not it exists. |
| `data/icons/render.py` | Renders the PNGs from the SVG. `make -C data icons` runs it. |

To change the icon, edit the SVG and re-run `make -C data icons`; the PNGs are
committed so that installing needs no rasteriser, and regenerated so that they
cannot drift from the artwork. The SVG is drawn in the application's own
palette and is checked against it by a test, so a hand-drawn icon in a
different indigo fails the build.

Validate the packaging data with the same tools a distribution would use:

```bash
make -C data check
```

## Runtime requirements

OClock is a GPU-accelerated Linux desktop application. It needs:

- **A Rust toolchain** (1.85 or newer) to build, and the system libraries Iced
  needs: `libxkbcommon`, and a Wayland or X11 display server.
- **A GPU or a software Vulkan/GL driver** — Iced renders through `wgpu`.
- **ALSA** for sound, and the **system tzdata** for the world clock's city
  index. Both are optional in the sense that the application runs without them:
  with no audio device the sounds are dropped with a message on stderr, and
  with no tzdata the city picker says so instead of offering a list. Desktop
  notifications are optional too, and a machine with no notification daemon
  gets the in-window presentation.
- **The system timezone database** is the *source* of the city list rather than
  a separate requirement: it is what the rest of the desktop already uses.

There is no configuration file to create, no daemon to run, and no network
access at any point. The application talks to the display server, to ALSA, to
the notification daemon over D-Bus, and to nothing else.

## Where your settings live

`$XDG_CONFIG_HOME/oclock/settings.json` — or `~/.config/oclock/settings.json`
if that is not set. The file is written atomically, the previous good copy is
kept alongside it, and a file that has been edited by hand is read field by
field: one unreadable setting costs only itself, and the repair is reported in
the window rather than hidden.

Timer presets in that file are plain numbers of seconds, so it is reasonable to
edit by hand. A document with no `version` field is version 0 and is migrated
forward on load, and a document written by a *newer* build is left alone.

## Project structure

The codebase is layered, and each layer is only allowed to know about the ones
above it in this list.

```text
oclock/
├── src/
│   ├── main.rs            the window and the run loop; deliberately thin
│   ├── lib.rs             the library root
│   ├── app/               the application: state, messages, wiring
│   │   ├── mod.rs           OClock: state, update, subscriptions, view
│   │   ├── message.rs       every message, in one place
│   │   ├── view.rs          what the interface is showing
│   │   └── icon.rs          the window's icon, drawn not shipped
│   ├── design/            the design system
│   │   ├── tokens.rs        spacing, radii, type ramp, motion, easing curves
│   │   ├── palette.rs       semantic colour, in both appearances
│   │   ├── motion.rs        the animation engine
│   │   └── theme.rs         light/dark, and the transition between them
│   ├── domain/            feature logic; knows nothing about the interface
│   │   ├── clock.rs         formatting and the per-second snapshot
│   │   ├── alarm.rs         the alarm model and the schedule engine
│   │   ├── timer.rs         countdown state machine
│   │   ├── stopwatch.rs     stopwatch and laps
│   │   ├── world.rs         world-clock locations and search
│   │   ├── sound.rs         the sound vocabulary
│   │   └── fmt.rs           duration formatting
│   ├── core/              the platform, and nothing above it
│   │   ├── time.rs          wall and monotonic clocks; jump detection
│   │   ├── tz.rs            timezone detection and DST-exact conversion
│   │   └── ids.rs           identifiers
│   ├── services/          the desktop
│   │   ├── storage.rs       atomic writes, backup, tolerant reads
│   │   ├── settings.rs      the persisted document and its migrations
│   │   ├── i18n/            the language list, the catalogue, the strings
│   │   ├── notify.rs        notifications, behind a trait
│   │   ├── audio.rs         synthesised sounds, behind a trait
│   │   └── tzdata.rs        the city index, from the system tzdata
│   └── ui/                the interface
│       ├── typography.rs     the type ramp
│       ├── text_metrics.rs   natural widths, measured by the drawing shaper
│       ├── components/       the design system's vocabulary
│       │   ├── surfaces.rs       cards, wells, rules, the recede and slide
│       │   ├── controls.rs       buttons, switches, chips, badges
│       │   ├── inputs.rs         fields, steppers, segmented choices
│       │   ├── indicators.rs     the ring, the dial, the sweep
│       │   ├── navigation.rs     the rail and its sliding indicator
│       │   ├── dialog.rs         dialogs, empty states, toasts
│       │   ├── icons.rs          the vector icon set
│       │   └── interaction.rs    per-control animated values
│       ├── shell.rs          the window's frame
│       └── pages/            clock, world, alarms, timer, stopwatch
├── tests/
│   └── packaging.rs        the desktop entry, the metadata and the icon set
└── data/                  the desktop entry, the metadata, the icons, the Makefile
```

### Why it is shaped this way

**`domain` never imports `ui`, and `services` never imports `ui`.** Business
logic is therefore testable without a window, and the interface is a
*projection* of state rather than a second source of truth. Most of the tests
in this repository are in `domain` and `services`, and most of them never
construct a widget at all.

**One `Message` enum.** The whole surface the application can be asked to do
is readable in `app/message.rs`. A new feature cannot introduce a second,
parallel way of talking to the interface.

**Feature state and view state are separate.** The alarms live in
`domain::alarm::AlarmSet`; which alarm is being edited, whether the city picker
is open and where the keyboard focus sits live in `app::view::View`. A change
to the interface cannot disturb a feature's state, and vice versa.

**One snapshot per second, not per frame.** Everything calendar-derived — the
formatted clock, the date, the world clock's rows — is computed once per second
in `app::Snapshot` and read by the view. A 60 Hz window does not reformat a
date sixty times a second for a string that changes once a minute. The
*smooth* parts of the clock — the second sweep, the ring, the hands — are
driven by continuous fractions rather than by the digits.

**Identifiers are allocated per collection.** Alarm ids, world-clock city ids
and timer-preset ids are three separate spaces, each with its own allocator.
Sharing one counter would let a newly added city land on an existing alarm's
id; the tests for each collection's allocator exist because of that.

### Timing

Two clocks, kept strictly apart.

**Wall time** (`core::time::utc_now`) is the civil calendar. The displayed
time, the date, and when an alarm is due all come from it, because that is what
"07:30 tomorrow" means.

**Monotonic time** (`core::time::monotonic`) only moves forward and is
unaffected by clock changes. Timer elapsed time, stopwatch elapsed time and
every animation come from it, because that is what "elapsed" means.

Nothing measures a duration by subtracting one wall-clock reading from
another, and nothing counts down a number:

```text
// domain::timer — the whole idea
elapsed = banked + (now - running_since)
```

`now` comes from a caller-supplied monotonic reading, so a stall, a busy event
loop or a slow frame cannot make a timer drift: the value is recomputed from
the clock, never accumulated. The same is true of the stopwatch.

**Suspend and resume** are handled by noticing that the two clocks disagree.
`core::time::Watchdog` samples both on every tick; a difference above
`NOISE_FLOOR` is a discontinuity. On Linux a suspend looks like the wall clock
jumping forward, because `CLOCK_MONOTONIC` stops while the machine sleeps, so
the handling is: bring the duration measurements forward by the missing
amount, and recompute the calendar-derived state. A timer that was counting
down finishes on waking; a stopwatch has counted the sleep; the alarm schedule
is unaffected because it is a wall-clock intention either way. A clock stepped
*backwards* never shortens a measurement.

**Daylight saving** is resolved rather than avoided.
`core::tz::resolve_wall_time` gives a defined answer to both awkward cases:
a wall time that happens **twice** takes the first occurrence, and a wall time
that **does not exist** takes the instant it shifts to, which is what a desktop
clock does. Daylight saving itself is *derived* — a zone is in summer time when
its current offset is greater than its smallest offset across the year — so
there is no rule table, and a zone that has abolished daylight saving correctly
reports none.

**Cost.** The subscription tree has four streams: a tick whose interval the
application chooses (16 ms while anything is animating, 250 ms when nothing
is), the window's own events, keyboard input, and notification actions. There
is no other polling. The alarm schedule is derived from the wall clock on the
tick, not watched; the system timezone is probed a couple of times a minute. An
idle window costs four wakeups a second, and a running stopwatch makes the tick
60 Hz without any change in the amount of work a tick does.

### Persistence

`services::storage::Store` writes atomically: a temporary file in the same
directory is written, flushed, and renamed over the original, and the previous
good file is retained. A crash mid-write cannot leave a half-written
configuration.

Reads are tolerant, field by field. The document is parsed as JSON and each
setting is decoded individually, so an unreadable hour format costs the hour
format and nothing else; the repairs are collected and reported in the window.
A document with no `version` field is version 0 and is migrated forward;
`Config::migrate` is idempotent and leaves a document written by a *newer*
build alone. `Config::sanitised` then repairs values that are individually
impossible — an alarm at 07:99, a zero-length preset, a duplicate id — because
a hand-edited file can contain them and the interface has no way to express
them.

### Desktop integration

The application ID is **`org.odsos.OClock`**, and it is spelled the same way in
three files: the desktop entry, the AppStream metadata and the icon's name.
`tests/packaging.rs` checks that they agree, that the desktop entry's name is
the application's name, and that every icon size exists at the dimensions its
directory claims — none of which anything compiled would otherwise notice.

Notifications go through `services::notify::Notifier`. The implementation is
the freedesktop D-Bus protocol; an ODS-os-native backend is a second
implementation of one trait with two methods, and nothing above `services`
knows which is in use. The wording and the offered actions are built as plain
data, so they are tested on a machine with no notification daemon at all. A
snooze pressed on a desktop notification produces the same `Message` as a
snooze pressed in the window, so both take the same path. Failures are
reported, never raised: a machine with no daemon still gets the in-window
presentation.

## Testing and verification

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The test suite runs in about two seconds and needs no display, no audio device
and no notification daemon. Most of it is in `domain` and `services` and never
constructs a widget; the interface is tested by building the widget tree and
asserting on the state that produced it, and by projecting every page into
every right-to-left language so a layout that only works in English cannot
pass.

There is also `make -C data check`, which runs `desktop-file-validate` and
`appstreamcli` over the packaging data when they are installed.

## Known limitations

- **The readouts are sized against the window, not against nothing.** A page
  knows roughly how wide its largest readout is — the type ramp can estimate
  it — and there is a test that the clock's and the stopwatch's readouts still
  fit the content column at the *narrowest* supported window. This is a
  consequence of Iced 0.13 having no way to measure a string: an estimate that
  is good enough to catch a readout which would overflow is worth more than
  none.
- **The city index depends on the system tzdata.** On a machine with no tzdata
  the picker is empty and says so. A bundled city list would fix that, at the
  cost of a data file to keep in step with the rest of the desktop.
- **Alarms are wall-clock alarms.** There is no per-alarm timezone: an alarm
  means "07:30 where I am", and a timezone change re-anchors it. A snooze is
  the exception, because it is a promise about elapsed time.
- **`iced` 0.13 has no per-widget opacity or translation.** The page
  transition and the theme veil are therefore built from layout and from an
  overlay rectangle; both are commented where the workaround lives.
- **The theme transition is deliberately a dissolve rather than a fade between
  two palettes.** This is a considered choice with a measured reason, not an
  omission; see *Themes* above.
- **Iced 0.13 exposes no way to set the X11 `WM_CLASS`.** The window therefore
  reports an empty one, and a compositor that identifies a running window by
  its class cannot match it to the desktop entry. The window's title and its
  icon are set and correct, which is what most compositors match on instead.

## Licence

OClock is licensed under the **BSD 3-Clause License**. The full text is in
[`LICENSE`](LICENSE), and the copyright is held by ODS.

That grant covers OClock's own work: the Rust source, the icon artwork, the
packaging data and this README. It does not extend to the crates OClock
depends on — they are separate works under their own licences, and none of
them is vendored into this repository, so there is nothing here to relicense.
