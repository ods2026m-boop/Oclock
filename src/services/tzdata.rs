//! The list of cities a user can watch, built from the platform's own data.
//!
//! OClock does not ship a city list. A place is named by its timezone
//! identifier — `Asia/Tokyo` is Tokyo, `America/Argentina/Buenos_Aires` is
//! Buenos Aires — and the set of identifiers comes from the zone files the
//! system's tzdata package installs, read once at start-up from a directory
//! walk that costs nothing measurable.
//!
//! Naming from the identifier rather than from a table matters: it is always
//! right, it is always in step with the installed tzdata, and it costs nothing
//! to maintain. Where a `zone1970.tab` is present its coordinates are folded in
//! as a bonus, but they are not needed for anything the picker does.
//!
//! When the zone directory is missing entirely — a stripped container, not a
//! desktop — [`index`] is empty, and the world clock page says so rather than
//! offering a list of invented places.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::tz;
use crate::domain::world::City;

/// Where the optional coordinate tables are looked for.
const TABLE_PATHS: [&str; 4] = [
    "/usr/share/zoneinfo/zone1970.tab",
    "/usr/share/zoneinfo/zone.tab",
    "/usr/share/lib/zoneinfo/zone1970.tab",
    "/usr/share/lib/zoneinfo/zone.tab",
];

/// Directories that hold zone files, in order of preference.
const ZONE_DIRS: [&str; 3] = [
    "/usr/share/zoneinfo",
    "/usr/share/lib/zoneinfo",
    "/etc/zoneinfo",
];

/// Files inside a zone directory that are not zones.
const NOT_A_ZONE: [&str; 12] = [
    "posix",
    "right",
    "localtime",
    "+VERSION",
    "SECURITY",
    "README",
    "leapseconds",
    "tzdata.zi",
    "iso3166.tab",
    "zone.tab",
    "zone1970.tab",
    "leap-seconds.list",
];

/// The searchable city index, loaded once.
pub fn index() -> &'static [City] {
    static INDEX: OnceLock<Vec<City>> = OnceLock::new();
    INDEX.get_or_init(load)
}

/// Where the index's data came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A zone directory, optionally enriched with a coordinate table.
    ZoneDirectory {
        directory: String,
        coordinates: bool,
    },
    /// No zone data is installed on this system.
    Missing,
}

impl Source {
    /// A short description for the user interface.
    pub fn describe(&self) -> String {
        match self {
            Source::ZoneDirectory {
                directory,
                coordinates,
            } => {
                if *coordinates {
                    format!("{directory} and the tzdata zone table")
                } else {
                    directory.clone()
                }
            }
            Source::Missing => "no timezone database is installed".to_string(),
        }
    }

    /// True when the picker can offer places at all.
    pub fn is_usable(&self) -> bool {
        !matches!(self, Source::Missing)
    }
}

/// Where the index's data came from.
pub fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| match zone_directory() {
        None => Source::Missing,
        Some(directory) => Source::ZoneDirectory {
            directory: directory.display().to_string(),
            coordinates: coordinates().is_some(),
        },
    })
}

fn load() -> Vec<City> {
    let Some(directory) = zone_directory() else {
        return Vec::new();
    };

    let coordinates = coordinates().unwrap_or_default();
    let mut cities = Vec::new();

    for (identifier, zone) in zones(&directory) {
        let (region, leaf) = split_identifier(&identifier);
        if leaf.is_empty() {
            continue;
        }

        let (latitude, longitude) = coordinates.get(zone.name()).copied().unwrap_or((0.0, 0.0));

        cities.push(City {
            zone: zone.name().to_string(),
            name: leaf.replace('_', " "),
            country_code: country_code(&region),
            country: region.replace('_', " "),
            latitude,
            longitude,
        });
    }

    finish(cities)
}

/// The region and the place within it.
///
/// `America/Argentina/Buenos_Aires` is a place in the Americas called Buenos
/// Aires, so the leaf is the name and the first component is the region.
fn split_identifier(identifier: &str) -> (String, String) {
    // `America/Argentina/Buenos_Aires` is a place in the Americas called Buenos
    // Aires, so the region is the first component and the name the last. A
    // single component such as `UTC` is its own name and has no region.
    if identifier.contains('/') {
        let region = identifier.split('/').next().unwrap_or_default();
        let leaf = identifier.rsplit('/').next().unwrap_or_default();
        (region.to_string(), leaf.to_string())
    } else {
        // A single component such as `UTC` is its own name and has no region.
        (String::new(), identifier.to_string())
    }
}

/// A two-letter hint derived from the region, for display only.
///
/// The authoritative country code is not derivable from a zone name, and
/// keeping an ISO 3166 table in step with the standard would be a
/// maintenance burden for no user-visible gain.
fn country_code(region: &str) -> String {
    region.chars().take(2).collect::<String>().to_uppercase()
}

/// Sorts and de-duplicates, so the picker behaves predictably however the
/// source happened to be ordered.
fn finish(cities: Vec<City>) -> Vec<City> {
    let mut cities = cities;
    cities.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.zone.cmp(&b.zone))
    });
    cities.dedup_by(|a, b| a.zone == b.zone && a.name == b.name);
    cities
}

/// Coordinates by zone, from whichever table is installed.
fn coordinates() -> Option<HashMap<String, (f32, f32)>> {
    let mut table = HashMap::new();
    for path in TABLE_PATHS {
        if read_coordinates(Path::new(path), &mut table) {
            return Some(table);
        }
    }
    None
}

/// Parses a tzdata zone table, keeping only the coordinates.
///
/// The format is a comment header then tab-separated rows of country codes,
/// coordinates, zone, and a free-text comment. Only the second and third
/// columns are wanted, and rows naming a zone this build cannot resolve are
/// skipped so a stale table cannot add a name that will not work.
fn read_coordinates(path: &Path, into: &mut HashMap<String, (f32, f32)>) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };

    let mut found = false;
    for line in contents.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let mut fields = line.split('\t');
        let (Some(_countries), Some(raw), Some(zone)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(parsed) = tz::parse(zone) else {
            continue;
        };

        into.insert(parsed.name().to_string(), parse_coordinates(raw));
        found = true;
    }

    found
}

/// `+DDMMSS+DDDMMSS` or `+DDMM+DDDMM`: two signed values with no separator.
fn parse_coordinates(raw: &str) -> (f32, f32) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (0.0, 0.0);
    }

    // The second sign starts the longitude.
    let split = trimmed[1..]
        .find(['+', '-'])
        .map(|index| index + 1)
        .unwrap_or(trimmed.len());

    (coordinate(&trimmed[..split]), coordinate(&trimmed[split..]))
}

/// `+DDMMSS` as signed decimal degrees.
fn coordinate(value: &str) -> f32 {
    if value.is_empty() {
        return 0.0;
    }

    let sign = if value.starts_with('-') { -1.0 } else { 1.0 };
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();

    // A latitude or longitude is written as degrees, minutes and optionally
    // seconds with no separators, so the last four digits are minutes and
    // seconds and whatever precedes them is whole degrees.
    // At least two digits are whole degrees, so a `DDMM` reading still splits.
    let split = digits.len().saturating_sub(4).max(2).min(digits.len());
    let degrees: f32 = digits[..split].parse().unwrap_or(0.0);
    let minutes: f32 = digits
        .get(split..split + 2)
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0);

    sign * (degrees + minutes / 60.0)
}

/// The first zone directory that exists.
pub fn zone_directory() -> Option<PathBuf> {
    ZONE_DIRS
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_dir())
}

/// Every compiled zone name, by walking the zone directory once.
fn zones(root: &Path) -> Vec<(String, chrono_tz::Tz)> {
    fn walk(root: &Path, prefix: &str, out: &mut Vec<(String, chrono_tz::Tz)>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if NOT_A_ZONE.contains(&name.as_str()) {
                continue;
            }

            let path = entry.path();
            let qualified = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };

            if path.is_dir() {
                walk(&path, &qualified, out);
            } else if !name.contains('.') {
                if let Some(zone) = tz::parse(&qualified) {
                    out.push((qualified, zone));
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(root, "", &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_into_a_region_and_a_place() {
        assert_eq!(
            split_identifier("Asia/Tokyo"),
            ("Asia".to_string(), "Tokyo".to_string())
        );
        assert_eq!(
            split_identifier("America/Argentina/Buenos_Aires"),
            ("America".to_string(), "Buenos_Aires".to_string())
        );
        assert_eq!(split_identifier("UTC"), (String::new(), "UTC".to_string()));
    }

    #[test]
    fn coordinates_parse_into_signed_degrees() {
        fn close(a: f32, b: f32) {
            assert!((a - b).abs() < 0.01, "expected {b}, got {a}");
        }

        let (latitude, longitude) = parse_coordinates("+340308-0841433");
        close(latitude, 34.05);
        close(longitude, -84.23);

        let (latitude, longitude) = parse_coordinates("+513030-0000731");
        close(latitude, 51.5);
        close(longitude, -0.12);

        // Southern and western hemispheres.
        let (latitude, longitude) = parse_coordinates("-335258+1511207");
        close(latitude, -33.87);
        close(longitude, 151.2);

        // The minute-only form.
        close(coordinate("-1230"), -12.5);
        close(coordinate("+4530"), 45.5);

        assert_eq!(parse_coordinates("+000000+0000000"), (0.0, 0.0));
        assert_eq!(parse_coordinates(""), (0.0, 0.0));
        assert_eq!(parse_coordinates("garbage").0, 0.0);
        assert_eq!(coordinate(""), 0.0);
    }

    #[test]
    fn the_index_is_usable_on_a_machine_with_tzdata() {
        let index = index();
        if zone_directory().is_none() {
            assert!(index.is_empty());
            assert!(!source().is_usable());
            assert!(source().describe().contains("no timezone database"));
            return;
        }

        assert!(!index.is_empty(), "the city index came back empty");
        assert!(source().is_usable());
        assert!(!source().describe().is_empty());
    }

    #[test]
    fn every_entry_names_a_zone_this_build_resolves() {
        // A picker entry that cannot place the clock is worse than no entry.
        for city in index() {
            assert!(
                tz::parse(&city.zone).is_some(),
                "{} does not resolve",
                city.zone
            );
            assert!(!city.name.trim().is_empty(), "unnamed entry: {city:?}");
        }
    }

    #[test]
    fn the_index_is_sorted_and_free_of_duplicates() {
        for pair in index().windows(2) {
            assert!(
                pair[0].name.to_lowercase() <= pair[1].name.to_lowercase(),
                "{} sorts before {}",
                pair[0].name,
                pair[1].name
            );
        }

        let mut keys: Vec<(String, String)> = index()
            .iter()
            .map(|city| (city.name.clone(), city.zone.clone()))
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), before, "the index contains duplicates");
    }

    #[test]
    fn place_names_read_as_places() {
        if index().is_empty() {
            return;
        }
        let names: Vec<&str> = index().iter().map(|city| city.name.as_str()).collect();

        // Spot-check the shape of the prettified names, which is the whole
        // premise of deriving them from the identifier.
        assert!(names.contains(&"Tokyo"), "Tokyo missing from {names:?}");
        assert!(names.contains(&"London"), "London missing");
        assert!(names.contains(&"New York"), "New York missing");
        assert!(
            names.iter().all(|name| !name.contains('_')),
            "underscores should have become spaces"
        );
    }

    #[test]
    fn a_real_table_contributes_coordinates() {
        let Some(path) = TABLE_PATHS.iter().map(PathBuf::from).find(|p| p.is_file()) else {
            return;
        };

        let mut table = HashMap::new();
        assert!(read_coordinates(&path, &mut table));
        assert!(table.len() > 100, "only {} rows parsed", table.len());

        let (latitude, longitude) = table
            .get("Asia/Tokyo")
            .copied()
            .expect("Tokyo should be in the table");
        // 35°39'16" N, 139°44'41" E.
        assert!((latitude - 35.65).abs() < 0.05, "latitude was {latitude}");
        assert!(
            (longitude - 139.74).abs() < 0.05,
            "longitude was {longitude}"
        );

        // Every entry must name a zone this build can resolve.
        for zone in table.keys() {
            assert!(tz::parse(zone).is_some(), "{zone} does not resolve");
        }
    }

    #[test]
    fn a_missing_table_is_reported_rather_than_guessed() {
        let mut table = HashMap::new();
        assert!(!read_coordinates(
            Path::new("/nonexistent/zone.tab"),
            &mut table
        ));
        assert!(table.is_empty());
    }

    #[test]
    fn rows_naming_an_unknown_zone_are_skipped() {
        let path =
            std::env::temp_dir().join(format!("oclock-bad-table-{}.tab", std::process::id()));
        std::fs::write(
            &path,
            "# comment\n\
             short\trow\n\
             ZZ\t+0000+0000\tMiddle/Earth\tNowhere\n\
             JP\t+353916+1394441\tAsia/Tokyo\tTokyo\n",
        )
        .expect("writes");

        let mut table = HashMap::new();
        assert!(read_coordinates(&path, &mut table));
        assert_eq!(table.len(), 1, "the unresolvable zone should be dropped");
        assert!(table.contains_key("Asia/Tokyo"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn country_codes_are_two_letters_of_the_region() {
        assert_eq!(country_code("Europe"), "EU");
        assert_eq!(country_code("America"), "AM");
        assert_eq!(country_code("UTC"), "UT");
        assert_eq!(country_code(""), "");
    }

    #[test]
    fn the_zone_walk_skips_duplicates_and_files() {
        let Some(directory) = zone_directory() else {
            return;
        };

        let found = zones(&directory);
        assert!(!found.is_empty());
        for (identifier, zone) in &found {
            assert_eq!(identifier, zone.name(), "{identifier} and {zone} disagree");
            // `posix/` and `right/` hold duplicate copies of every zone.
            assert!(!identifier.starts_with("posix/"));
            assert!(!identifier.starts_with("right/"));
            // The tables are not zones.
            assert!(!identifier.ends_with(".tab"));
            assert!(!identifier.contains('.'), "{identifier} is a data file");
        }
    }
}
