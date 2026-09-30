//! World clocks: which locations are shown, and how they are found.
//!
//! Locations are stored as IANA timezone identifiers, never as UTC offsets, so
//! daylight-saving transitions are whatever the tz database says they are.
//! The human-readable city names come from the platform's own zone table
//! (`zone1970.tab`), which is the same data `tzdata` ships — see
//! `services::tzdata` for how it is discovered.

use serde::{Deserialize, Serialize};

use chrono::Offset;

use crate::core::tz::{self, Zoned};

/// How the world-clock list is ordered.
///
/// Pinning is not a fourth order: a pinned city leads all three of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortMode {
    /// The order the user dragged them into.
    #[default]
    Manual,
    /// Alphabetical by city.
    City,
    /// By UTC offset, closest to the user's own zone first.
    Offset,
}

impl SortMode {
    /// The choices, in menu order.
    pub const ALL: [SortMode; 3] = [SortMode::Manual, SortMode::City, SortMode::Offset];

    /// Menu label.
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Manual => "Custom order",
            SortMode::City => "By city",
            SortMode::Offset => "By time zone",
        }
    }

    /// The next mode, for a single control that cycles.
    pub fn next(self) -> SortMode {
        match self {
            SortMode::Manual => SortMode::City,
            SortMode::City => SortMode::Offset,
            SortMode::Offset => SortMode::Manual,
        }
    }
}

/// A city or region the user can watch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Location {
    /// Stable identity.
    pub id: u64,
    /// Display name, e.g. `Tokyo`.
    pub city: String,
    /// The IANA identifier that defines its time, e.g. `Asia/Tokyo`.
    pub zone: String,
    /// Whether the user pinned this location to the front.
    pub pinned: bool,
}

impl Default for Location {
    fn default() -> Self {
        Location {
            id: 0,
            city: "Unknown".to_string(),
            zone: "UTC".to_string(),
            pinned: false,
        }
    }
}

impl Location {
    /// A location for a resolved timezone.
    pub fn new(id: u64, city: impl Into<String>, zone: chrono_tz::Tz) -> Location {
        Location {
            id,
            city: city.into(),
            zone: zone.name().to_string(),
            pinned: false,
        }
    }

    /// The timezone, if the stored identifier is still known.
    ///
    /// A zone can stop being valid when tzdata is updated; the caller keeps
    /// the card and shows an error rather than dropping the user's data.
    pub fn tz(&self) -> Option<chrono_tz::Tz> {
        tz::parse(&self.zone)
    }

    /// The current time at this location.
    pub fn now(&self) -> Option<Zoned> {
        self.tz().map(tz::now_in)
    }
}

/// Everything the world-clock page needs, computed once per tick.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldRow {
    /// The location this row describes.
    pub location: Location,
    /// Its current time, or `None` when the zone is no longer recognised.
    pub at: Option<Zoned>,
    /// `UTC+09:00`
    pub offset: String,
    /// `JST`
    pub abbreviation: String,
    /// Whether daylight saving is in effect there right now.
    pub daylight_saving: bool,
    /// Signed difference from the user's own zone, in whole minutes.
    ///
    /// Positive means the location is ahead. Derived from the two zones'
    /// actual offsets at this instant, so it accounts for whichever
    /// hemisphere's summer it is.
    pub offset_from_local: i32,
    /// `Today` / `Tomorrow` / `Yesterday` relative to the user's date.
    pub day_relation: DayRelation,
    /// A secondary line: the date there.
    pub date: String,
}

/// How a location's calendar day relates to the user's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayRelation {
    Yesterday,
    Today,
    Tomorrow,
}

impl DayRelation {
    /// The word shown on the card.
    pub fn label(self) -> &'static str {
        match self {
            DayRelation::Yesterday => "Yesterday",
            DayRelation::Today => "Today",
            DayRelation::Tomorrow => "Tomorrow",
        }
    }
}

/// The world-clock collection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldClocks {
    locations: Vec<Location>,
    sort: SortMode,
    /// The next id this collection will hand out.
    next_id: u64,
}

impl WorldClocks {
    /// Builds a collection from restored state, dropping entries whose id is
    /// already taken by an earlier one.
    pub fn new(locations: Vec<Location>, sort: SortMode) -> WorldClocks {
        let mut seen = std::collections::HashSet::new();
        let locations: Vec<Location> = locations
            .into_iter()
            .filter(|location| seen.insert(location.id))
            .collect();

        // Ids are this collection's own: a location id and an alarm id live in
        // different namespaces, and sharing one counter between them would let a
        // newly added city land on the id of an existing alarm.
        //
        // `saturating_add` rather than `+ 1`: a stored document is untrusted
        // input, and a city saved with the largest identifier there can be would
        // otherwise overflow a debug build on the way to being rejected.
        // Saturating leaves the allocator refusing to go past it, which is the
        // answer that cannot collide with anything already in the file.
        let next_id = locations
            .iter()
            .map(|location| location.id)
            .max()
            .map_or(1, |highest| highest.saturating_add(1));

        WorldClocks {
            locations,
            sort,
            next_id,
        }
    }

    /// Adds a city, giving it the next free id in this collection.
    ///
    /// `None` when there is no identifier left to give: a stored document is
    /// untrusted input, and one carrying every identifier there is has left the
    /// collection nothing to allocate. It is reported rather than papered over,
    /// because an id handed out twice is a city that cannot be pinned, removed
    /// or distinguished from the one it collides with.
    pub fn add_city(&mut self, city: impl Into<String>, zone: chrono_tz::Tz) -> Option<u64> {
        let id = self.next_free_id()?;
        self.next_id = id.saturating_add(1);
        self.add(Location::new(id, city, zone));
        Some(id)
    }

    /// The next identifier this collection can hand out.
    ///
    /// Sequential in the ordinary case. A stored document may already hold the
    /// identifier the counter was about to reach — a hand-edited one can hold
    /// the largest there is — so a taken one is stepped over rather than reused,
    /// and exhaustion is `None` rather than a second city sharing an identity.
    fn next_free_id(&self) -> Option<u64> {
        if self.next_id > 0 && self.get(self.next_id).is_none() {
            return Some(self.next_id);
        }
        (1..=self.next_id).find(|id| self.get(*id).is_none())
    }

    /// The locations, in their current display order.
    pub fn locations(&self) -> &[Location] {
        &self.locations
    }

    /// The chosen ordering.
    pub fn sort(&self) -> SortMode {
        self.sort
    }

    /// Sets the ordering.
    pub fn set_sort(&mut self, sort: SortMode) {
        self.sort = sort;
    }

    /// How many locations are shown.
    pub fn len(&self) -> usize {
        self.locations.len()
    }

    /// True when no locations are shown.
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    /// Looks a location up.
    pub fn get(&self, id: u64) -> Option<&Location> {
        self.locations.iter().find(|location| location.id == id)
    }

    /// Adds a location, ignoring an exact duplicate of an existing one.
    ///
    /// Returns the id of the existing entry if the same city is already shown,
    /// so adding a city twice is a harmless no-op rather than a duplicate row.
    pub fn add(&mut self, location: Location) -> u64 {
        if let Some(existing) = self
            .locations
            .iter()
            .find(|candidate| candidate.zone == location.zone && candidate.city == location.city)
        {
            return existing.id;
        }
        let id = location.id;
        self.locations.push(location);
        id
    }

    /// Removes a location.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.locations.len();
        self.locations.retain(|location| location.id != id);
        self.locations.len() != before
    }

    /// Pins or unpins a location.
    pub fn set_pinned(&mut self, id: u64, pinned: bool) {
        if let Some(location) = self.locations.iter_mut().find(|l| l.id == id) {
            location.pinned = pinned;
        }
    }

    /// Moves a location to a new index, for drag-free reordering.
    pub fn move_to(&mut self, id: u64, index: usize) {
        let Some(from) = self.locations.iter().position(|l| l.id == id) else {
            return;
        };
        let location = self.locations.remove(from);
        self.locations
            .insert(index.min(self.locations.len()), location);
    }

    /// The locations in display order, measured at `at`.
    ///
    /// Passing the instant keeps offset ordering reproducible and keeps the
    /// ordering consistent with the rows the caller is about to build.
    ///
    /// A pinned city leads in every order, because pinning is the one thing the
    /// user said about that row that outranks the sort they chose: it is what
    /// "pinned" means, and a card drawn with an accent saying it is pinned has to
    /// be the card at the top. The sort mode still decides the order of
    /// everything else, and pinned cities keep their own relative order, so
    /// switching modes never reshuffles the part the user arranged by hand.
    pub fn ordered(&self, at: Zoned) -> Vec<&Location> {
        let mut ordered: Vec<&Location> = self.locations.iter().collect();
        match self.sort {
            SortMode::Manual => {}
            SortMode::City => {
                ordered
                    .sort_by_key(|location| (location.city.to_lowercase(), location.zone.clone()));
            }
            SortMode::Offset => {
                // The zone furthest ahead of the viewer's own first; an
                // unrecognised zone sinks to the bottom rather than jumping
                // around.
                ordered.sort_by_key(|location| {
                    (
                        location
                            .tz()
                            .map(|zone| at.with_timezone(&zone).offset().fix().local_minus_utc())
                            .unwrap_or(i32::MAX),
                        location.city.to_lowercase(),
                    )
                });
            }
        }

        // Stable, so this moves the pinned group to the front without touching
        // the order inside either group.
        if ordered.iter().any(|location| location.pinned) {
            ordered.sort_by_key(|location| !location.pinned);
        }

        ordered
    }

    /// Builds the per-tick rows for the world-clock page.
    ///
    /// `at` is the instant every location is measured against, so all the
    /// cards on screen agree with each other and the whole result is a pure
    /// function of its arguments.
    pub fn rows(&self, at: Zoned, local_zone: chrono_tz::Tz) -> Vec<WorldRow> {
        let local_offset = tz::utc_offset_at(local_zone, at);
        let local_date = at.date_naive();

        self.ordered(at)
            .into_iter()
            .map(|location| {
                let Some(zone) = location.tz() else {
                    return WorldRow {
                        location: location.clone(),
                        at: None,
                        offset: "—".to_string(),
                        abbreviation: "Unknown zone".to_string(),
                        daylight_saving: false,
                        offset_from_local: 0,
                        day_relation: DayRelation::Today,
                        date: location.zone.clone(),
                    };
                };

                let at = at.with_timezone(&zone);
                let difference = tz::utc_offset_at(zone, at) - local_offset;
                let day_relation = match at.date_naive().cmp(&local_date) {
                    std::cmp::Ordering::Less => DayRelation::Yesterday,
                    std::cmp::Ordering::Equal => DayRelation::Today,
                    std::cmp::Ordering::Greater => DayRelation::Tomorrow,
                };

                WorldRow {
                    location: location.clone(),
                    offset: tz::format_offset(tz::utc_offset_at(zone, at)),
                    abbreviation: tz::abbreviation(zone, at),
                    daylight_saving: tz::is_daylight_saving(zone, at),
                    offset_from_local: difference / 60,
                    day_relation,
                    date: at.format("%-d %b").to_string(),
                    at: Some(at),
                }
            })
            .collect()
    }
}

/// A place the user can add, as loaded from the platform's zone table.
#[derive(Clone, Debug, PartialEq)]
pub struct City {
    /// IANA identifier.
    pub zone: String,
    /// The city's name, e.g. `Tokyo`.
    pub name: String,
    /// ISO country code, when the table provides one.
    pub country_code: String,
    /// Country name, when it can be determined.
    pub country: String,
    /// Rough latitude, for the "near me" ordering.
    pub latitude: f32,
    /// Rough longitude.
    pub longitude: f32,
}

impl City {
    /// A short description for a search result row.
    pub fn subtitle(&self) -> String {
        if self.country.is_empty() {
            self.zone.clone()
        } else {
            format!("{} · {}", self.zone, self.country)
        }
    }

    /// Everything a search should match against, lower-cased.
    pub fn haystack(&self) -> String {
        format!(
            "{} {} {} {}",
            self.name, self.zone, self.country, self.country_code
        )
        .to_lowercase()
    }
}

/// Ranks [`City`] entries against a query.
///
/// Exact and prefix matches on the city name always outrank a fuzzy match
/// somewhere in the country name, because someone typing "tok" wants Tokyo,
/// not Bolivia. Blank queries return the input order untouched so an empty
/// search box shows the whole list in its natural order.
pub fn search<'a>(query: &str, cities: &'a [City]) -> Vec<&'a City> {
    let needle: String = query
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();

    if needle.is_empty() {
        return cities.iter().collect();
    }

    let mut scored: Vec<(u8, usize, &'a City)> = Vec::new();

    for (index, city) in cities.iter().enumerate() {
        let Some(score) = score(needle.as_str(), city) else {
            continue;
        };
        scored.push((score, index, city));
    }

    scored.sort_by_key(|(score, index, _)| (*score, *index));
    scored.into_iter().map(|(_, _, city)| city).collect()
}

/// How good a match is: lower is better. `None` means "not a match".
fn score(needle: &str, city: &City) -> Option<u8> {
    let name = city.name.to_lowercase();
    let zone = city.zone.to_lowercase();
    let country = city.country.to_lowercase();

    if name == needle {
        Some(0)
    } else if name.starts_with(needle) {
        Some(1)
    } else if name.contains(needle) {
        Some(2)
    } else if zone.contains(needle) {
        Some(3)
    } else if country.starts_with(needle) {
        Some(4)
    } else if let Some(hit) = sublime_fuzzy::best_match(needle, &city.haystack()) {
        // Fuzzy hits are the weakest signal there is, and a loose one is worse
        // than a tight one, but neither is ever allowed to outrank a real
        // substring match: someone typing "tor" wants Toronto, not a city that
        // happens to contain the letters.
        let needle_len = needle.len().max(1);
        let coverage = hit.matched_indices().len() * 8 / needle_len;
        Some(if coverage >= 4 { 5 } else { 6 })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;
    use chrono_tz::Tz;

    fn zone(name: &str) -> Tz {
        name.parse().expect("known zone")
    }

    /// A fixed instant, so nothing here depends on when the suite runs.
    fn at(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Zoned {
        tz::resolve_wall_time_parts(tz, y, mo, d, h, mi).expect("resolvable")
    }

    fn sample_cities() -> Vec<City> {
        vec![
            City {
                zone: "Asia/Tokyo".into(),
                name: "Tokyo".into(),
                country_code: "JP".into(),
                country: "Japan".into(),
                latitude: 35.68,
                longitude: 139.69,
            },
            City {
                zone: "Europe/London".into(),
                name: "London".into(),
                country_code: "GB".into(),
                country: "United Kingdom".into(),
                latitude: 51.51,
                longitude: -0.12,
            },
            City {
                zone: "America/Toronto".into(),
                name: "Toronto".into(),
                country_code: "CA".into(),
                country: "Canada".into(),
                latitude: 43.65,
                longitude: -79.38,
            },
            City {
                zone: "America/Toronto".into(),
                name: "Mississauga".into(),
                country_code: "CA".into(),
                country: "Canada".into(),
                latitude: 43.59,
                longitude: -79.64,
            },
            City {
                zone: "Europe/Rome".into(),
                name: "Turin".into(),
                country_code: "IT".into(),
                country: "Italy".into(),
                latitude: 45.07,
                longitude: 7.69,
            },
        ]
    }

    #[test]
    fn locations_round_trip_and_resolve_their_zone() {
        let location = Location::new(1, "Tokyo", zone("Asia/Tokyo"));
        assert_eq!(location.zone, "Asia/Tokyo");
        assert_eq!(location.tz(), Some(zone("Asia/Tokyo")));

        let json = serde_json::to_string(&location).expect("serializes");
        let restored: Location = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(location, restored);
    }

    #[test]
    fn a_zone_alias_still_resolves() {
        // A city stored under an alias name must not become a broken card:
        // whatever the name resolves to, it has to place the clock somewhere
        // real, and for a link it must be the same place as its target.
        let location = Location {
            id: 1,
            city: "Toronto".into(),
            zone: "Canada/Eastern".into(),
            pinned: false,
        };
        let resolved = location.tz().expect("alias resolves");

        let reference = Location::new(2, "Toronto", zone("America/Toronto"));
        let now = at(zone("UTC"), 2026, 6, 15, 12, 0);
        assert_eq!(
            now.with_timezone(&resolved).naive_local(),
            now.with_timezone(&reference.tz().expect("known zone"))
                .naive_local(),
            "an alias must land on the same wall time as its target"
        );
    }

    #[test]
    fn an_unknown_zone_does_not_panic() {
        let location = Location {
            zone: "Middle/Earth".into(),
            ..Location::default()
        };
        assert_eq!(location.tz(), None);
        assert!(location.now().is_none());
    }

    #[test]
    fn adding_the_same_city_twice_is_a_no_op() {
        let mut world = WorldClocks::default();
        let first = world.add(Location::new(1, "Tokyo", zone("Asia/Tokyo")));
        let second = world.add(Location::new(2, "Tokyo", zone("Asia/Tokyo")));

        assert_eq!(first, second);
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn different_cities_sharing_a_zone_are_kept() {
        let mut world = WorldClocks::default();
        world.add(Location::new(1, "Toronto", zone("America/Toronto")));
        world.add(Location::new(2, "Mississauga", zone("America/Toronto")));
        assert_eq!(world.len(), 2);
    }

    #[test]
    fn locations_can_be_removed() {
        let mut world = WorldClocks::default();
        let id = world.add(Location::new(1, "Tokyo", zone("Asia/Tokyo")));
        assert!(world.remove(id));
        assert!(!world.remove(id));
        assert!(world.is_empty());
    }

    #[test]
    fn manual_order_is_preserved_and_reorderable() {
        let mut world = WorldClocks::default();
        let a = world.add(Location::new(1, "Tokyo", zone("Asia/Tokyo")));
        let b = world.add(Location::new(2, "London", zone("Europe/London")));
        let c = world.add(Location::new(3, "Turin", zone("Europe/Rome")));
        let now = at(zone("Europe/London"), 2026, 6, 15, 12, 0);

        let order =
            |world: &WorldClocks| world.ordered(now).iter().map(|l| l.id).collect::<Vec<_>>();
        assert_eq!(order(&world), vec![a, b, c]);

        world.move_to(c, 0);
        assert_eq!(order(&world), vec![c, a, b]);

        // Out-of-range indices clamp rather than panic.
        world.move_to(a, 99);
        assert_eq!(order(&world), vec![c, b, a]);

        // Moving something that is not there is a no-op.
        world.move_to(999, 0);
        assert_eq!(order(&world), vec![c, b, a]);
    }

    #[test]
    fn city_sort_is_alphabetical() {
        let world = WorldClocks::new(
            vec![
                Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                Location::new(2, "London", zone("Europe/London")),
                Location::new(3, "Turin", zone("Europe/Rome")),
            ],
            SortMode::City,
        );
        let now = at(zone("UTC"), 2026, 6, 15, 12, 0);
        let order: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        assert_eq!(order, vec!["London", "Tokyo", "Turin"]);
    }

    #[test]
    fn offset_sort_puts_pinned_first_then_by_offset() {
        let world = WorldClocks::new(
            vec![
                Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                Location::new(2, "London", zone("Europe/London")),
                Location::new(3, "New York", zone("America/New_York")),
            ],
            SortMode::Offset,
        );
        let now = at(zone("Europe/London"), 2026, 6, 15, 12, 0);
        let order: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        // Unpinned, by UTC offset ascending: New York −4, London +1, Tokyo +9.
        assert_eq!(order, vec!["New York", "London", "Tokyo"]);

        let mut pinned = world;
        pinned.set_pinned(1, true);
        let order: Vec<&str> = pinned
            .ordered(now)
            .iter()
            .map(|l| l.city.as_str())
            .collect();
        assert_eq!(order[0], "Tokyo", "pinned locations lead");
    }

    #[test]
    fn a_pinned_city_leads_in_every_order() {
        // Pinning used to be honoured under one sort mode out of three, so the
        // same city sat at the top of the list or in the middle of it depending
        // on an unrelated choice, and its card said "pinned" either way.
        let build = |mode| {
            let mut world = WorldClocks::new(
                vec![
                    Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                    Location::new(2, "London", zone("Europe/London")),
                    Location::new(3, "New York", zone("America/New_York")),
                ],
                mode,
            );
            world.set_pinned(1, true);
            world
        };
        let now = at(zone("Europe/London"), 2026, 6, 15, 12, 0);

        for mode in [SortMode::Manual, SortMode::City, SortMode::Offset] {
            let world = build(mode);
            let order: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
            assert_eq!(order[0], "Tokyo", "pinned leads under {mode:?}");
        }

        // And the sort still decides the order of everything else.
        let world = build(SortMode::City);
        let cities: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        assert_eq!(cities, vec!["Tokyo", "London", "New York"]);

        let world = build(SortMode::Offset);
        let offsets: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        assert_eq!(offsets, vec!["Tokyo", "New York", "London"]);
    }

    #[test]
    fn pinning_never_reshuffles_the_order_it_did_not_touch() {
        // Two pinned cities keep the order the user put them in, whatever the
        // sort mode says about either of them: a pin says "these two matter",
        // not "these two are in this order".
        let mut world = WorldClocks::new(
            vec![
                Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                Location::new(2, "London", zone("Europe/London")),
                Location::new(3, "New York", zone("America/New_York")),
            ],
            SortMode::City,
        );
        world.set_pinned(3, true);
        world.set_pinned(1, true);

        let now = at(zone("Europe/London"), 2026, 6, 15, 12, 0);
        let order: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        assert_eq!(
            order,
            vec!["New York", "Tokyo", "London"],
            "the pinned pair keeps its configured order, the rest is sorted"
        );
    }

    #[test]
    fn a_configuration_full_of_the_largest_identifier_loads() {
        // A stored document is untrusted input. An identifier one short of the
        // ceiling used to be turned into `id + 1` on the way in, which overflows
        // a debug build rather than being rejected.
        let mut world = WorldClocks::new(
            vec![Location::new(u64::MAX, "Edge", zone("UTC"))],
            SortMode::Manual,
        );
        assert_eq!(
            world.add_city("Also edge", zone("UTC")),
            Some(1),
            "a taken identifier is stepped over, not handed out twice"
        );
        assert_eq!(
            world.add_city("Third", zone("Europe/London")),
            Some(2),
            "and the counter carries on from there"
        );
    }

    #[test]
    fn a_collection_with_no_identifier_left_says_so() {
        // Every identifier reachable is taken, which a hand-edited document can
        // arrange. Adding a city then reports that it could not, rather than
        // giving two cities one identity.
        let mut world = WorldClocks::new(Vec::new(), SortMode::Manual);
        world.next_id = 2;
        world.add(Location::new(1, "First", zone("UTC")));
        world.add(Location::new(2, "Second", zone("UTC")));

        assert_eq!(
            world.add_city("One too many", zone("Europe/London")),
            None,
            "no identifier left to give"
        );
        assert_eq!(world.len(), 2, "and nothing was added");
    }

    #[test]
    fn offset_sort_sinks_an_unrecognised_zone_to_the_bottom() {
        let world = WorldClocks::new(
            vec![
                Location {
                    id: 1,
                    city: "Nowhere".into(),
                    zone: "Middle/Earth".into(),
                    pinned: false,
                },
                Location::new(2, "Tokyo", zone("Asia/Tokyo")),
            ],
            SortMode::Offset,
        );
        let now = at(zone("UTC"), 2026, 6, 15, 12, 0);
        let order: Vec<&str> = world.ordered(now).iter().map(|l| l.city.as_str()).collect();
        assert_eq!(order, vec!["Tokyo", "Nowhere"]);
    }

    #[test]
    fn sort_mode_cycles_and_labels() {
        assert_eq!(SortMode::Manual.next(), SortMode::City);
        assert_eq!(SortMode::City.next(), SortMode::Offset);
        assert_eq!(SortMode::Offset.next(), SortMode::Manual);
        assert_eq!(SortMode::Manual.label(), "Custom order");
        assert_eq!(SortMode::City.label(), "By city");
        assert_eq!(SortMode::Offset.label(), "By time zone");
    }

    #[test]
    fn rows_carry_offsets_and_day_relations() {
        let home = zone("Europe/Istanbul");
        let world = WorldClocks::new(
            vec![Location::new(1, "Tokyo", zone("Asia/Tokyo"))],
            SortMode::Manual,
        );

        let rows = world.rows(at(home, 2026, 6, 15, 12, 0), home);
        assert_eq!(rows.len(), 1);

        let row = &rows[0];
        assert_eq!(row.offset, "UTC+09:00");
        assert_eq!(row.abbreviation, "JST");
        assert!(!row.daylight_saving);
        // Istanbul is UTC+3 in June, Tokyo UTC+9: six hours ahead.
        assert_eq!(row.offset_from_local, 360);
        assert_eq!(row.day_relation, DayRelation::Today);
        assert_eq!(row.date, "15 Jun");
        // Istanbul is on a fixed UTC+3, so noon there is 18:00 in Tokyo.
        assert_eq!(row.at.unwrap().naive_local().hour(), 18);
    }

    #[test]
    fn every_row_is_measured_from_the_same_instant() {
        let home = zone("Europe/London");
        let world = WorldClocks::new(
            vec![
                Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                Location::new(2, "New York", zone("America/New_York")),
            ],
            SortMode::Manual,
        );

        let instant = at(home, 2026, 6, 15, 12, 0);
        let rows = world.rows(instant, home);
        for row in &rows {
            assert_eq!(
                row.at.map(|at| at.timestamp()),
                Some(instant.timestamp()),
                "{} was not measured from the supplied instant",
                row.location.city
            );
        }
    }

    #[test]
    fn day_relation_reflects_the_local_date() {
        let auckland = zone("Pacific/Auckland");
        let world = WorldClocks::new(
            vec![Location::new(1, "Los Angeles", zone("America/Los_Angeles"))],
            SortMode::Manual,
        );

        // 08:00 in Auckland on the 16th is still the 15th in Los Angeles.
        let rows = world.rows(at(auckland, 2026, 6, 16, 8, 0), auckland);
        assert_eq!(rows[0].day_relation, DayRelation::Yesterday);

        // 22:00 in Auckland on the 16th has already reached Los Angeles.
        let rows = world.rows(at(auckland, 2026, 6, 16, 22, 0), auckland);
        assert_eq!(rows[0].day_relation, DayRelation::Today);
    }

    #[test]
    fn day_relation_reaches_tomorrow() {
        let tokyo = zone("Asia/Tokyo");
        let world = WorldClocks::new(
            vec![Location::new(1, "Auckland", zone("Pacific/Auckland"))],
            SortMode::Manual,
        );
        let rows = world.rows(at(tokyo, 2026, 6, 15, 23, 0), tokyo);
        assert_eq!(rows[0].day_relation, DayRelation::Tomorrow);
    }

    #[test]
    fn an_unrecognised_zone_produces_an_error_row_rather_than_a_panic() {
        let mut world = WorldClocks::default();
        world.add(Location {
            id: 1,
            city: "Nowhere".into(),
            zone: "Middle/Earth".into(),
            pinned: false,
        });

        let zone = zone("Europe/Istanbul");
        let rows = world.rows(at(zone, 2026, 6, 15, 12, 0), zone);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].at.is_none());
        assert_eq!(rows[0].abbreviation, "Unknown zone");
        assert_eq!(rows[0].offset, "—");
    }

    #[test]
    fn offset_difference_is_measured_in_minutes() {
        let home = zone("Europe/London");
        let world = WorldClocks::new(
            vec![Location::new(1, "Kolkata", zone("Asia/Kolkata"))],
            SortMode::Manual,
        );
        // India is UTC+5:30, London UTC+1 in June, so 4h30m = 270 minutes.
        let rows = world.rows(at(home, 2026, 6, 15, 12, 0), home);
        assert_eq!(rows[0].offset_from_local, 270);
        assert_eq!(rows[0].offset, "UTC+05:30");
    }

    #[test]
    fn offset_difference_accounts_for_which_hemester_is_summer() {
        // In January London is on GMT and Sydney on summer time, eleven hours
        // ahead. In July London is on summer time and Sydney is not, so the gap
        // narrows to nine. Neither is a constant, and a hard-coded difference
        // would be wrong for half the year.
        let home = zone("Europe/London");
        let world = WorldClocks::new(
            vec![Location::new(1, "Sydney", zone("Australia/Sydney"))],
            SortMode::Manual,
        );

        let winter = world.rows(at(home, 2026, 1, 15, 12, 0), home);
        assert_eq!(winter[0].offset_from_local, 11 * 60);
        assert!(winter[0].daylight_saving, "Sydney is on AEDT in January");

        let summer = world.rows(at(home, 2026, 7, 15, 12, 0), home);
        assert_eq!(summer[0].offset_from_local, 9 * 60);
        assert!(!summer[0].daylight_saving, "Sydney is on AEST in July");
    }

    #[test]
    fn daylight_saving_is_reported_for_the_location() {
        let home = zone("Europe/London");
        let world = WorldClocks::new(
            vec![Location::new(1, "London", zone("Europe/London"))],
            SortMode::Manual,
        );
        let winter = world.rows(at(home, 2026, 1, 15, 12, 0), home);
        let summer = world.rows(at(home, 2026, 7, 15, 12, 0), home);
        assert!(!winter[0].daylight_saving);
        assert!(summer[0].daylight_saving);
    }

    #[test]
    fn restoring_keeps_the_first_of_any_duplicate_ids() {
        let world = WorldClocks::new(
            vec![
                Location::new(1, "Tokyo", zone("Asia/Tokyo")),
                Location::new(1, "London", zone("Europe/London")),
            ],
            SortMode::Manual,
        );
        assert_eq!(world.len(), 1);
        assert_eq!(world.get(1).unwrap().city, "Tokyo");
    }

    #[test]
    fn pin_and_sort_are_persisted() {
        let mut world = WorldClocks::default();
        world.add(Location::new(1, "Tokyo", zone("Asia/Tokyo")));
        world.set_pinned(1, true);
        world.set_sort(SortMode::Offset);
        assert!(world.get(1).unwrap().pinned);
        assert_eq!(world.sort(), SortMode::Offset);
    }

    // ---- search ----

    #[test]
    fn blank_query_returns_everything_in_order() {
        let all = sample_cities();
        for query in ["", "   "] {
            let results = search(query, &all);
            assert_eq!(results.len(), all.len());
            assert_eq!(results[0].name, "Tokyo");
        }
    }

    #[test]
    fn exact_match_ranks_first() {
        let all = sample_cities();
        assert_eq!(search("tokyo", &all)[0].name, "Tokyo");
    }

    #[test]
    fn a_prefix_outranks_a_name_that_only_contains_the_query() {
        let all = sample_cities();
        let names: Vec<&str> = search("tor", &all)
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        // "Toronto" is a prefix match. "Mississauga" only matches through its
        // shared zone name, which is a weaker signal, and it comes after.
        assert_eq!(names[0], "Toronto");
        assert!(names.contains(&"Mississauga"));
    }

    #[test]
    fn a_city_name_outranks_a_country_name() {
        let all = sample_cities();
        // "Turin" starts with "tur"; "Turkey" does not appear at all, so this
        // checks the ordering is name-first rather than insertion order.
        let names: Vec<&str> = search("t", &all).iter().map(|c| c.name.as_str()).collect();
        // The three names starting with "t" lead, in list order. Then the two
        // that only match elsewhere: Toronto's zone for Mississauga, a fuzzy
        // hit inside "United Kingdom" for London.
        assert_eq!(
            names,
            vec!["Tokyo", "Toronto", "Turin", "Mississauga", "London"]
        );
    }

    #[test]
    fn whitespace_and_case_are_ignored() {
        let all = sample_cities();
        assert_eq!(search("  To Kyo ", &all)[0].name, "Tokyo");
    }

    #[test]
    fn searching_by_zone_finds_the_city() {
        let all = sample_cities();
        assert_eq!(search("Asia/Tokyo", &all)[0].name, "Tokyo");
    }

    #[test]
    fn searching_by_country_finds_its_cities() {
        let all = sample_cities();
        let names: Vec<&str> = search("canada", &all)
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["Toronto", "Mississauga"]);
    }

    #[test]
    fn a_query_matching_nothing_returns_nothing() {
        assert!(search("zzzzqqq", &sample_cities()).is_empty());
    }

    #[test]
    fn diacritics_are_reachable() {
        let all = vec![City {
            zone: "America/Sao_Paulo".into(),
            name: "São Paulo".into(),
            country_code: "BR".into(),
            country: "Brazil".into(),
            latitude: 0.0,
            longitude: 0.0,
        }];
        assert_eq!(search("sao", &all).len(), 1);
        assert_eq!(search("são", &all).len(), 1);
    }

    #[test]
    fn results_are_stable_for_the_same_query() {
        let all = sample_cities();
        let first: Vec<String> = search("to", &all).iter().map(|c| c.name.clone()).collect();
        let second: Vec<String> = search("to", &all).iter().map(|c| c.name.clone()).collect();
        assert_eq!(first, second, "ranking must not depend on map ordering");
    }

    #[test]
    fn city_subtitle_prefers_zone_and_country() {
        let known = City {
            zone: "Asia/Tokyo".into(),
            name: "Tokyo".into(),
            country_code: "JP".into(),
            country: "Japan".into(),
            latitude: 0.0,
            longitude: 0.0,
        };
        assert_eq!(known.subtitle(), "Asia/Tokyo · Japan");

        let unknown_country = City {
            country: String::new(),
            ..known
        };
        assert_eq!(unknown_country.subtitle(), "Asia/Tokyo");
    }
}
