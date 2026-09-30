//! System timezone detection and unambiguous local-time resolution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::Duration as ChronoDuration;
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Civil time in a specific timezone. The single currency for "what time is
/// it where", used for the clock, the world clock and alarm scheduling alike.
pub type Zoned = DateTime<Tz>;

/// The current time in `tz`, derived from the system wall clock.
pub fn now_in(tz: Tz) -> Zoned {
    DateTime::<Utc>::from_naive_utc_and_offset(Utc::now().naive_utc(), Utc).with_timezone(&tz)
}

/// The name of the timezone the system is configured to use, if it can be
/// determined.
///
/// Probed in the order the major distributions use:
/// `TZ` in the environment, then `/etc/timezone` (Debian, Ubuntu, ODS-os),
/// then the target of the `/etc/localtime` symlink, then
/// `/etc/sysconfig/clock` (Fedora, RHEL, Arch).
pub fn system_timezone_name() -> Option<String> {
    if let Some(name) = std::env::var("TZ").ok().and_then(normalise) {
        if is_known(&name) {
            return Some(name);
        }
    }

    if let Some(name) = read_zone_name(Path::new("/etc/timezone")) {
        if is_known(&name) {
            return Some(name);
        }
    }

    if let Some(name) = read_symlink_zone(Path::new("/etc/localtime")) {
        if is_known(&name) {
            return Some(name);
        }
    }

    if let Some(name) = read_sysconfig_zone(Path::new("/etc/sysconfig/clock")) {
        if is_known(&name) {
            return Some(name);
        }
    }

    None
}

/// The system timezone, falling back to UTC when the platform does not
/// disclose one. A clock app must always render *something* plausible.
pub fn system_timezone() -> Tz {
    system_timezone_name()
        .as_deref()
        .and_then(parse)
        .unwrap_or(Tz::UTC)
}

/// Parses an IANA timezone name.
///
/// Names that the database only defines as *links* — `Europe/Torino` is the
/// long-standing alias for `Europe/Rome` — are resolved to their canonical
/// zone. Without this, a city picked from the platform's own zone table would
/// come back as an unknown zone.
pub fn parse(name: &str) -> Option<Tz> {
    if let Ok(tz) = name.parse::<Tz>() {
        return Some(tz);
    }
    links().get(name).copied()
}

/// Canonical name for a zone alias, if it is one.
pub fn resolve_link(name: &str) -> Option<&'static str> {
    links().get(name).map(|tz| tz.name())
}

/// The `Link` aliases declared by the installed tzdata, resolved to zones.
///
/// Loaded once and shared. The primary source is `tzdata.zi`, which is
/// installed alongside the zone files and lists every alias; if it is missing,
/// the table is empty and only canonical names resolve, which is a
/// degradation rather than a failure.
fn links() -> &'static HashMap<String, Tz> {
    static LINKS: OnceLock<HashMap<String, Tz>> = OnceLock::new();
    LINKS.get_or_init(load_links)
}

fn load_links() -> HashMap<String, Tz> {
    let mut links = HashMap::new();

    // `tzdata.zi` follows the `backward` file's field order:
    // `L <target> <alias>`, i.e. "alias is another name for target".
    let Ok(contents) = std::fs::read_to_string("/usr/share/zoneinfo/tzdata.zi") else {
        return links;
    };

    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some("L") {
            continue;
        }
        let (Some(target), Some(alias)) = (fields.next(), fields.next()) else {
            continue;
        };
        if alias == target {
            continue;
        }
        // Only keep aliases we can actually resolve, so a stale or partial
        // table cannot turn a valid name into a broken one.
        if let Ok(tz) = target.parse::<Tz>() {
            links.insert(alias.to_string(), tz);
        }
    }

    links
}

/// Strips the `:` prefix some systems put in `TZ`, and quotes.
fn normalise(raw: String) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_start_matches(':');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// True when `name` is a timezone the compiled-in database knows about.
fn is_known(name: &str) -> bool {
    parse(name).is_some()
}

fn read_zone_name(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    normalise(contents)
}

/// Recovers a zone name from a `zoneinfo` path such as
/// `/usr/share/zoneinfo/Europe/Istanbul`.
fn zone_name_from_path(raw: &str) -> Option<String> {
    let marker = "zoneinfo/";
    let start = raw.find(marker)? + marker.len();
    let name = raw[start..].trim_start_matches('/');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn read_symlink_zone(path: &Path) -> Option<String> {
    let target = std::fs::read_link(path).ok()?;
    zone_name_from_path(&target.to_string_lossy())
}

fn read_sysconfig_zone(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("ZONE=") else {
            continue;
        };
        // `ZONE="Europe/Istanbul"` or `ZONE=Europe/Istanbul`.
        let value = rest.trim().trim_matches('"');
        if let Some(name) = zone_name_from_path(value) {
            return Some(name);
        }
        if let Some(name) = normalise(value.to_string()) {
            return Some(name);
        }
    }
    None
}

/// Resolves a *wall-clock* wall time in `tz` to an absolute instant.
///
/// `chrono`'s own local-time resolution is deliberately strict, and that
/// strictness is wrong for an alarm. Around a DST transition a wall time can
/// be **ambiguous** (it happens twice) or **nonexistent** (spring forward
/// skips it), and both need a defined answer:
///
/// * unambiguous — that instant;
/// * ambiguous — take the **first** of the two, so the alarm rings once, at
///   the first occurrence, like every desktop clock;
/// * nonexistent — take the instant the wall time shifts *to*, which is what
///   every desktop clock does: a 02:30 alarm on a spring-forward morning
///   rings at 03:30 local.
///
/// The first two come straight from `chrono`'s local-time resolution. The
/// third has to be constructed: the wall time is interpreted using the offset
/// that was in effect *before* the jump, which places it on the far side of
/// the gap by exactly the size of the jump.
pub fn resolve_wall_time(tz: Tz, day: NaiveDate, hour: u32, minute: u32) -> Option<Zoned> {
    let naive = day.and_hms_opt(hour, minute, 0)?;
    Some(resolve_naive(tz, naive))
}

/// Resolves a local wall-clock reading in `tz`, with the ambiguity and gap
/// policy described on [`resolve_wall_time`].
pub fn resolve_naive(tz: Tz, naive: NaiveDateTime) -> Zoned {
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(instant) => instant,
        LocalResult::Ambiguous(first, _) => first,
        LocalResult::None => {
            // The reading falls in a forward transition. Probe the offset a
            // day either side: if the offset grew, this is the gap, and
            // subtracting the *old* offset lands on the far side of it.
            let probe = |days: i64| {
                let shifted = naive + ChronoDuration::days(days);
                tz.from_local_datetime(&shifted)
                    .earliest()
                    .map(|instant| utc_offset_at(tz, instant))
            };

            match (probe(-1), probe(1)) {
                (Some(before), Some(after)) if after > before => (naive.and_utc()
                    - ChronoDuration::seconds(i64::from(before)))
                .with_timezone(&tz),
                // A genuine gap always shows a growing offset. If it somehow
                // does not, fall back to the reading taken as UTC rather than
                // guessing at a transition.
                _ => naive.and_utc().with_timezone(&tz),
            }
        }
    }
}

/// Resolves a wall time from calendar parts, returning `None` for impossible
/// dates such as the 30th of February.
pub fn resolve_wall_time_parts(
    tz: Tz,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<Zoned> {
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    resolve_wall_time(tz, date, hour, minute)
}

/// True when the offset in effect at `instant` is daylight saving time in
/// `tz`.
///
/// Derived by sampling the zone across a year and comparing against its
/// smallest offset, which by definition is standard time in both hemispheres.
/// No hard-coded rule tables, and correct for zones that abolished DST.
pub fn is_daylight_saving(tz: Tz, instant: Zoned) -> bool {
    standard_offset(tz, instant.date_naive()) < utc_offset(tz, instant.naive_local())
}

fn standard_offset(tz: Tz, on: NaiveDate) -> i32 {
    let year = on.format("%Y").to_string().parse().unwrap_or(1970);
    (1..=12)
        .filter_map(|month| {
            let day = NaiveDate::from_ymd_opt(year, month, 15)?;
            let naive = day.and_hms_opt(12, 0, 0)?;
            Some(utc_offset(tz, naive))
        })
        .min()
        .unwrap_or(0)
}

/// The zone's UTC offset, in seconds, that a wall-clock reading resolves to.
///
/// Resolves the reading first and then reads the offset off the resulting
/// instant, so the answer is exact even for a reading that sits next to a
/// daylight-saving transition — where simply asking "what offset does the zone
/// have at this naive time" is ambiguous.
pub fn utc_offset(tz: Tz, naive_local: NaiveDateTime) -> i32 {
    utc_offset_at(tz, resolve_naive(tz, naive_local))
}

/// The zone's UTC offset, in seconds, at an instant, taken from the instant
/// itself rather than recomputed.
pub fn utc_offset_at(_tz: Tz, instant: Zoned) -> i32 {
    use chrono::Offset;
    instant.offset().fix().local_minus_utc()
}

/// A readable offset such as `UTC+03:00` or `UTC-04:30`.
///
/// The sign is a plain hyphen: the typographic minus is not in every system's
/// default font, and a missing glyph in a timezone label is a very visible way
/// to look unfinished.
pub fn format_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let magnitude = seconds.unsigned_abs();
    format!(
        "UTC{sign}{:02}:{:02}",
        magnitude / 3600,
        (magnitude % 3600) / 60
    )
}

/// The short zone abbreviation, e.g. `CET` or `+03`, falling back to the
/// numeric offset when the database has no name for the period.
pub fn abbreviation(tz: Tz, instant: Zoned) -> String {
    use chrono_tz::OffsetName;

    let name = instant
        .offset()
        .abbreviation()
        .unwrap_or_default()
        .to_string();
    if name.is_empty() || name.starts_with('+') || name.starts_with('-') {
        format_offset(utc_offset_at(tz, instant))
    } else {
        name
    }
}

/// Watches the configured system timezone and reports changes.
///
/// The check is a couple of small file reads, so it is sampled from the slow
/// housekeeping tick rather than run on every frame.
#[derive(Debug)]
pub struct TimeZoneMonitor {
    current: Tz,
    since_check: u32,
}

/// How many housekeeping ticks pass between timezone probes.
const PROBE_INTERVAL_TICKS: u32 = 30;

impl TimeZoneMonitor {
    /// Starts monitoring the timezone that is configured right now.
    pub fn new() -> Self {
        TimeZoneMonitor {
            current: system_timezone(),
            since_check: 0,
        }
    }

    /// The timezone currently in effect.
    pub fn current(&self) -> Tz {
        self.current
    }

    /// Advances the probe, returning the new timezone if it changed.
    pub fn check(&mut self) -> Option<Tz> {
        self.since_check += 1;
        if self.since_check < PROBE_INTERVAL_TICKS {
            return None;
        }
        self.since_check = 0;

        let detected = system_timezone();
        if detected == self.current {
            return None;
        }

        self.current = detected;
        Some(detected)
    }
}

impl Default for TimeZoneMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the OClock configuration lives.
pub fn config_dir(app: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join(app))
}

/// Where the OClock state that should survive a reboot lives.
pub fn data_dir(app: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|base| base.join(app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn detects_a_known_zone_from_a_zoneinfo_path() {
        let name = zone_name_from_path("/usr/share/zoneinfo/Europe/Istanbul");
        assert_eq!(name.as_deref(), Some("Europe/Istanbul"));

        let name = zone_name_from_path("/etc/zoneinfo/America/Argentina/Buenos_Aires");
        assert_eq!(name.as_deref(), Some("America/Argentina/Buenos_Aires"));

        assert_eq!(zone_name_from_path("/usr/share/zoneinfo"), None);
        assert_eq!(zone_name_from_path("nonsense"), None);
    }

    #[test]
    fn normalise_strips_punctuation() {
        assert_eq!(
            normalise(":Europe/Istanbul".into()).as_deref(),
            Some("Europe/Istanbul")
        );
        assert_eq!(
            normalise("  Europe/Istanbul\n".into()).as_deref(),
            Some("Europe/Istanbul")
        );
        assert_eq!(normalise("\"UTC\"\n".into()).as_deref(), Some("UTC"));
        assert_eq!(normalise("   \n".into()), None);
    }

    #[test]
    fn every_alias_the_platform_declares_resolves() {
        // The world clock stores whatever zone name the platform's city list
        // used, and a name the resolver cannot turn into a zone would render a
        // broken card. Whatever source a name resolves through, it must
        // resolve.
        if links().is_empty() {
            // No tzdata table installed; `parse` still has to cope with the
            // canonical names chrono-tz knows, which the other tests cover.
            return;
        }

        for (alias, expected) in links() {
            let Some(resolved) = parse(alias) else {
                panic!("alias {alias} did not resolve to any zone");
            };
            // An alias must behave exactly like its target. Comparing offsets
            // rather than enum variants is the point: the same zone can be
            // spelled as a distinct entry in the compiled database, and all
            // that matters is that the clock comes out the same.
            let at = now_in(*expected);
            assert_eq!(
                utc_offset_at(resolved, at.with_timezone(&resolved)),
                utc_offset_at(*expected, at),
                "alias {alias} resolved to a zone with a different offset"
            );
        }
    }

    #[test]
    fn an_alias_resolves_to_a_zone_with_the_same_calendar() {
        // A single-char query is the hardest case for a fuzzy matcher: it will
        // find something in almost any string. Every alias must still land on
        // the same wall clock as its target.
        if links().is_empty() {
            return;
        }
        let day = NaiveDate::from_ymd_opt(2026, 6, 15).expect("valid date");
        for (alias, target) in links() {
            let Some(resolved) = parse(alias) else {
                continue;
            };
            let via_alias = resolve_wall_time(resolved, day, 12, 0).expect("resolvable");
            let via_target = resolve_wall_time(*target, day, 12, 0).expect("resolvable");
            assert_eq!(
                via_alias.naive_local(),
                via_target.naive_local(),
                "alias {alias} disagrees with its target"
            );
        }
    }

    #[test]
    fn a_canonical_name_resolves_to_itself() {
        let tokyo = parse("Asia/Tokyo").expect("known zone");
        assert_eq!(tokyo.name(), "Asia/Tokyo");
        assert_eq!(resolve_link("Asia/Tokyo"), None);
    }

    #[test]
    fn an_invented_name_still_fails() {
        assert_eq!(parse("Middle/Earth"), None);
        assert_eq!(resolve_link("Middle/Earth"), None);
    }

    #[test]
    fn system_timezone_always_resolves() {
        // Whatever the host looks like, the app must be able to render a time.
        let tz = system_timezone();
        assert!(parse(tz.name()).is_some());
    }

    #[test]
    fn resolve_wall_time_is_exact_when_no_transition_is_nearby() {
        let tz = parse("Europe/Istanbul").expect("known zone");
        let day = NaiveDate::from_ymd_opt(2026, 6, 15).expect("valid date");
        let resolved = resolve_wall_time(tz, day, 9, 41).expect("resolvable");

        assert_eq!(resolved.naive_local().hour(), 9);
        assert_eq!(resolved.naive_local().minute(), 41);
        // Turkey keeps a single UTC+3 offset year round, so this reading is
        // unambiguous and lands exactly.
        assert_eq!(utc_offset_at(tz, resolved), 3 * 3600);
    }

    #[test]
    fn ambiguous_wall_time_resolves_to_the_first_occurrence() {
        // Europe/Berlin falls back from 03:00 CEST to 02:00 CET on
        // 2026-10-25, so 02:30 local happens twice.
        let tz = parse("Europe/Berlin").expect("known zone");
        let day = NaiveDate::from_ymd_opt(2026, 10, 25).expect("valid date");

        let resolved = resolve_wall_time(tz, day, 2, 30).expect("resolvable");
        assert_eq!(resolved.naive_local().hour(), 2);
        assert_eq!(resolved.naive_local().minute(), 30);
        // The earlier (pre-transition, larger) offset is the first one.
        assert_eq!(utc_offset_at(tz, resolved), 2 * 3600);

        // And it really is ambiguous, so the choice was meaningful.
        let ambiguous = tz.from_local_datetime(&day.and_hms_opt(2, 30, 0).unwrap());
        assert!(matches!(ambiguous, chrono::LocalResult::Ambiguous(_, _)));
    }

    #[test]
    fn nonexistent_wall_time_shifts_forward() {
        // Europe/Berlin springs forward from 02:00 to 03:00 on 2026-03-29,
        // so 02:30 local does not exist.
        let tz = parse("Europe/Berlin").expect("known zone");
        let day = NaiveDate::from_ymd_opt(2026, 3, 29).expect("valid date");

        let gap = tz.from_local_datetime(&day.and_hms_opt(2, 30, 0).unwrap());
        assert!(
            matches!(gap, chrono::LocalResult::None),
            "precondition: a DST gap"
        );

        let resolved = resolve_wall_time(tz, day, 2, 30).expect("resolvable");
        assert_eq!(resolved.naive_local().hour(), 3);
        assert_eq!(resolved.naive_local().minute(), 30);
    }

    #[test]
    fn half_hour_offsets_are_formatted() {
        assert_eq!(format_offset(0), "UTC+00:00");
        assert_eq!(format_offset(3 * 3600), "UTC+03:00");
        assert_eq!(format_offset(19800), "UTC+05:30");
        assert_eq!(format_offset(-(12 * 3600)), "UTC-12:00");
        assert_eq!(format_offset(-(9 * 3600 - 1800)), "UTC-08:30");
        assert!(
            format_offset(-3600).is_ascii(),
            "the offset label must be plain ASCII to be renderable everywhere"
        );
    }

    #[test]
    fn daylight_saving_is_derived_from_the_database() {
        let tz = parse("Europe/Berlin").expect("known zone");

        let summer = resolve_wall_time_parts(tz, 2026, 7, 15, 12, 0).expect("resolvable");
        let winter = resolve_wall_time_parts(tz, 2026, 1, 15, 12, 0).expect("resolvable");

        assert!(is_daylight_saving(tz, summer), "July is DST in Berlin");
        assert!(!is_daylight_saving(tz, winter), "January is standard time");
    }

    #[test]
    fn a_zone_that_abolished_daylight_saving_reports_none() {
        // Turkey has been on a single UTC+3 offset since 2016; nothing here
        // may be inferred from a rule table.
        let tz = parse("Europe/Istanbul").expect("known zone");
        for month in 1..=12 {
            let instant = resolve_wall_time_parts(tz, 2026, month, 15, 12, 0).expect("resolvable");
            assert!(
                !is_daylight_saving(tz, instant),
                "month {month} reported DST"
            );
        }
    }

    #[test]
    fn zones_without_daylight_saving_are_never_in_dst() {
        let tz = parse("Asia/Kolkata").expect("known zone");
        for month in 1..=12 {
            let instant = resolve_wall_time_parts(tz, 2026, month, 15, 12, 0).expect("resolvable");
            assert!(
                !is_daylight_saving(tz, instant),
                "month {month} reported DST"
            );
        }
    }

    #[test]
    fn southern_hemisphere_daylight_saving_is_detected() {
        let tz = parse("Australia/Sydney").expect("known zone");
        let january = resolve_wall_time_parts(tz, 2026, 1, 15, 12, 0).expect("resolvable");
        let july = resolve_wall_time_parts(tz, 2026, 7, 15, 12, 0).expect("resolvable");

        assert!(is_daylight_saving(tz, january));
        assert!(!is_daylight_saving(tz, july));
    }

    #[test]
    fn impossible_dates_are_rejected() {
        let tz = parse("Europe/Istanbul").expect("known zone");
        assert!(resolve_wall_time_parts(tz, 2026, 2, 30, 10, 0).is_none());
        assert!(resolve_wall_time_parts(tz, 2026, 13, 1, 10, 0).is_none());
        assert!(resolve_wall_time_parts(tz, 2026, 1, 1, 25, 0).is_none());
    }

    #[test]
    fn abbreviation_prefers_a_real_name() {
        let tz = parse("Europe/Istanbul").expect("known zone");
        let instant = resolve_wall_time_parts(tz, 2026, 7, 15, 12, 0).expect("resolvable");
        // Istanbul's database entry has no alphabetic abbreviation, so the
        // numeric form is the correct fallback.
        assert!(abbreviation(tz, instant).starts_with("UTC"));

        let berlin = parse("Europe/Berlin").expect("known zone");
        let winter = resolve_wall_time_parts(berlin, 2026, 1, 15, 12, 0).expect("resolvable");
        assert_eq!(abbreviation(berlin, winter), "CET");
        let summer = resolve_wall_time_parts(berlin, 2026, 7, 15, 12, 0).expect("resolvable");
        assert_eq!(abbreviation(berlin, summer), "CEST");
    }

    #[test]
    fn monitor_reports_the_current_zone_and_ignores_early_ticks() {
        let mut monitor = TimeZoneMonitor::new();
        assert_eq!(monitor.current(), system_timezone());
        assert_eq!(monitor.check(), None, "the first tick must not probe");

        for _ in 0..PROBE_INTERVAL_TICKS - 1 {
            assert_eq!(monitor.check(), None);
        }
        // A real probe runs, and on an unchanged system finds no change.
        assert_eq!(monitor.check(), None);
    }
}
