//! Robust local persistence.
//!
//! Three things matter for a clock application's state, and all three are
//! handled here rather than scattered through the call sites:
//!
//! * **Nothing valid is ever lost.** The file is read as a generic JSON
//!   document and each section is decoded on its own, so one malformed field
//!   falls back to its default while everything else in the file is kept.
//!   Whatever was recovered is reported back so the UI can say so.
//! * **Writes are atomic.** The new document is written to a temporary file in
//!   the same directory, flushed, and renamed over the old one, so a crash
//!   mid-write cannot leave a half-written configuration. The previous good
//!   file is kept as a backup and used if the primary is unreadable.
//! * **Missing is normal.** A first launch is not an error, and an absent file
//!   yields defaults.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// The file extension used for the configuration.
const EXTENSION: &str = "json";
/// Suffix of the temporary file written before a rename.
const TEMPORARY: &str = "tmp";
/// Suffix of the retained previous good file.
const BACKUP: &str = "bak";

/// Something that had to be recovered while loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    /// Where in the document the problem was, e.g. `alarms[2].snooze_minutes`.
    pub path: String,
    /// A short explanation, in the user's terms.
    pub detail: String,
}

impl std::fmt::Display for Recovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.path, self.detail)
    }
}

/// The outcome of loading a document.
///
/// Every variant carries the value to use, so a caller can never be left
/// without an answer; what differs is whether anything is worth reporting.
#[derive(Clone, Debug, PartialEq)]
pub enum Loaded<T> {
    /// Nothing on disk; defaults were used.
    Fresh { value: T },
    /// The document was read, possibly with some parts recovered.
    Restored {
        value: T,
        /// Non-empty when something had to be defaulted.
        recovered: Vec<Recovery>,
    },
    /// The document could not be read at all. Defaults were used.
    Failed { value: T, detail: String },
}

impl<T> Loaded<T> {
    /// The loaded value, whatever happened.
    pub fn value(self) -> T {
        match self {
            Loaded::Fresh { value }
            | Loaded::Restored { value, .. }
            | Loaded::Failed { value, .. } => value,
        }
    }

    /// Anything worth telling the user about.
    pub fn notices(&self) -> Vec<String> {
        match self {
            Loaded::Fresh { .. } => Vec::new(),
            Loaded::Restored { recovered, .. } => {
                recovered.iter().map(|entry| entry.to_string()).collect()
            }
            Loaded::Failed { detail, .. } => vec![detail.clone()],
        }
    }

    /// True when the value came from disk without any repair.
    pub fn is_clean(&self) -> bool {
        match self {
            Loaded::Fresh { .. } => true,
            Loaded::Restored { recovered, .. } => recovered.is_empty(),
            Loaded::Failed { .. } => false,
        }
    }

    /// True when nothing was on disk to begin with.
    pub fn is_fresh(&self) -> bool {
        matches!(self, Loaded::Fresh { .. })
    }
}

/// A JSON document on disk.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// A store backed by `path`, which need not exist yet.
    pub fn new(path: impl Into<PathBuf>) -> Store {
        Store { path: path.into() }
    }

    /// A store at `<dir>/<name>.json`, creating `dir` if needed.
    pub fn in_dir(dir: impl AsRef<Path>, name: &str) -> Result<Store, StoreError> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir).map_err(|source| StoreError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        Ok(Store::new(dir.join(format!("{name}.{EXTENSION}"))))
    }

    /// Where the document lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// True when a document exists.
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// Reads and decodes the document, section by section.
    ///
    /// `sections` is a list of `(key, default)`. Each is decoded on its own so
    /// that a single malformed section cannot cost the user the rest of their
    /// configuration.
    pub fn load_sections<T, F>(&self, mut sections: F) -> Loaded<T>
    where
        T: DeserializeOwned + Default,
        F: FnMut(&mut Loader) -> T,
    {
        let Some(text) = self.read_primary() else {
            return Loaded::Fresh {
                value: sections(&mut Loader::new(&Value::Null)),
            };
        };

        let document: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                // The primary file is unreadable. The backup is the whole
                // point of keeping one.
                if let Some(backup) = self.read_backup() {
                    if let Ok(value) = serde_json::from_str::<Value>(&backup) {
                        let mut loader = Loader::new(&value);
                        let decoded = sections(&mut loader);
                        let mut recovered = loader.recoveries;
                        recovered.insert(
                            0,
                            Recovery {
                                path: "configuration".to_string(),
                                detail: format!("recovered from the backup copy: {error}"),
                            },
                        );
                        return Loaded::Restored {
                            value: decoded,
                            recovered,
                        };
                    }
                }

                return Loaded::Failed {
                    value: sections(&mut Loader::new(&Value::Null)),
                    detail: format!("the configuration file could not be read: {error}"),
                };
            }
        };

        let mut loader = Loader::new(&document);
        let decoded = sections(&mut loader);
        Loaded::Restored {
            value: decoded,
            recovered: loader.recoveries,
        }
    }

    /// Reads and decodes a single document, falling back to `default`.
    pub fn load<T: DeserializeOwned + Default>(&self) -> Loaded<T> {
        self.load_sections(|loader| loader.root())
    }

    fn read_primary(&self) -> Option<String> {
        fs::read_to_string(&self.path).ok()
    }

    fn read_backup(&self) -> Option<String> {
        fs::read_to_string(self.backup_path()).ok()
    }

    fn backup_path(&self) -> PathBuf {
        sibling(&self.path, BACKUP)
    }

    fn temporary_path(&self) -> PathBuf {
        sibling(&self.path, TEMPORARY)
    }

    /// Writes the document atomically.
    pub fn save<T: Serialize>(&self, value: &T) -> Result<(), StoreError> {
        let text = serde_json::to_string_pretty(value).map_err(|source| StoreError::Format {
            source: source.to_string(),
        })?;
        self.write_atomically(&text)
    }

    /// Writes pre-rendered text atomically.
    pub fn write_atomically(&self, text: &str) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        // Retain the previous good file before touching the real one.
        if self.path.is_file() {
            let _ = fs::copy(&self.path, self.backup_path());
        }

        let temporary = self.temporary_path();
        {
            let mut file = fs::File::create(&temporary).map_err(|source| StoreError::Io {
                path: temporary.clone(),
                source,
            })?;
            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|source| StoreError::Io {
                    path: temporary.clone(),
                    source,
                })?;
        }

        fs::rename(&temporary, &self.path).map_err(|source| StoreError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "oclock".to_string());
    name.push('.');
    name.push_str(suffix);
    path.with_file_name(name)
}

/// Decodes individual fields out of a parsed document, recording anything it
/// has to default.
#[derive(Debug)]
pub struct Loader<'a> {
    root: &'a Value,
    recoveries: Vec<Recovery>,
}

impl<'a> Loader<'a> {
    /// Wraps a parsed document.
    pub fn new(root: &'a Value) -> Loader<'a> {
        Loader {
            root,
            recoveries: Vec::new(),
        }
    }

    /// The whole document, decoded as `T`.
    pub fn root<T: DeserializeOwned + Default>(&mut self) -> T {
        decode(self.root, "configuration", &mut self.recoveries)
    }

    /// One named section of the document.
    pub fn section<T: DeserializeOwned + Default>(&mut self, key: &str) -> T {
        let value = self.root.get(key).unwrap_or(&Value::Null);
        decode(value, key, &mut self.recoveries)
    }

    /// One field, addressed by a path of object keys.
    ///
    /// A path that does not exist yields the type's default *without*
    /// recording a recovery: absent is the normal case, not damage.
    ///
    /// Note the fallback is [`Default::default`] of the *field's* type, which
    /// for a scalar is almost never the model's default. Use [`Loader::patch`]
    /// for a field inside a model that has its own defaults.
    pub fn field<T: DeserializeOwned + Default>(&mut self, path: &[&str]) -> T {
        let Some(value) = self.lookup(path).cloned() else {
            return T::default();
        };
        decode(&value, &path.join("."), &mut self.recoveries)
    }

    /// Overrides part of an already-decoded model from a path, if the document
    /// has one.
    ///
    /// This is how a group with its own defaults is read. The group's `Default`
    /// supplies the fallback — so an absent `clock.show_seconds` leaves the
    /// model's `true` in place rather than the `false` a bare `bool` would give
    /// — and an unreadable value leaves it alone too, having recorded a
    /// recovery.
    pub fn patch<T: Default + DeserializeOwned + std::fmt::Debug>(
        &mut self,
        target: &mut T,
        path: &[&str],
    ) {
        let Some(value) = self.lookup(path).cloned() else {
            return;
        };
        match serde_json::from_value::<T>(value) {
            Ok(decoded) => *target = decoded,
            Err(error) => self.recoveries.push(Recovery {
                path: path.join("."),
                detail: format!(
                    "kept the default ({}) because {error}",
                    describe_default(target)
                ),
            }),
        }
    }

    /// The value at a path, if the document has one.
    fn lookup(&self, path: &[&str]) -> Option<&Value> {
        let mut cursor = self.root;
        for key in &path[..path.len() - 1] {
            cursor = cursor.get(key)?;
        }
        cursor.get(path[path.len() - 1])
    }

    /// Records a problem the caller discovered itself.
    pub fn note(&mut self, path: impl Into<String>, detail: impl Into<String>) {
        self.recoveries.push(Recovery {
            path: path.into(),
            detail: detail.into(),
        });
    }

    /// The recoveries collected so far.
    pub fn into_recoveries(self) -> Vec<Recovery> {
        self.recoveries
    }
}

/// A short description of what a value currently holds, for a recovery note.
fn describe_default<T: std::fmt::Debug>(value: &T) -> String {
    let plain = format!("{value:?}");
    let trimmed: String = plain.chars().take(40).collect();
    if plain.chars().count() > 40 {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

fn decode<T: DeserializeOwned + Default>(value: &Value, path: &str, into: &mut Vec<Recovery>) -> T {
    if value.is_null() {
        return T::default();
    }
    match serde_json::from_value(value.clone()) {
        Ok(decoded) => decoded,
        Err(error) => {
            into.push(Recovery {
                path: path.to_string(),
                detail: format!("reset to the default value: {error}"),
            });
            T::default()
        }
    }
}

/// A failure while reading or writing a document.
#[derive(Debug)]
pub enum StoreError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Format {
        source: String,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io { path, source } => {
                write!(f, "could not access {}: {source}", path.display())
            }
            StoreError::Format { source } => {
                write!(f, "could not encode the configuration: {source}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// A scratch directory that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "oclock-store-{label}-{}",
                u64::from(std::process::id()) * 1000
                    + u64::from(COUNTER.fetch_add(1, Ordering::SeqCst))
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Scratch(path)
        }

        fn file(&self, name: &str) -> PathBuf {
            self.0.join(format!("{name}.json"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    use std::sync::atomic::Ordering;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Section {
        label: String,
        count: u32,
        ratio: f64,
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Document {
        appearance: Section,
        clock: Section,
    }

    fn sections(document: &mut Loader) -> Document {
        Document {
            appearance: document.section("appearance"),
            clock: document.section("clock"),
        }
    }

    #[test]
    fn a_missing_file_yields_defaults_without_complaint() {
        let scratch = Scratch::new("missing");
        let store = Store::new(scratch.file("nothing"));
        assert!(!store.exists());

        let loaded = store.load_sections(sections);
        assert!(loaded.is_fresh());
        assert!(loaded.notices().is_empty());
        assert_eq!(loaded.value(), Document::default());
    }

    #[test]
    fn a_saved_document_round_trips() {
        let scratch = Scratch::new("roundtrip");
        let store = Store::new(scratch.file("config"));

        let document = Document {
            appearance: Section {
                label: "dark".into(),
                count: 3,
                ratio: 0.25,
            },
            clock: Section {
                label: "twelve".into(),
                count: 1,
                ratio: 1.0,
            },
        };
        store.save(&document).expect("saves");
        assert!(store.exists());

        let loaded = store.load_sections(sections);
        assert!(loaded.is_clean());
        assert_eq!(loaded.value(), document);
    }

    #[test]
    fn saving_twice_keeps_a_backup() {
        let scratch = Scratch::new("backup");
        let store = Store::new(scratch.file("config"));

        store
            .save(&Document {
                appearance: Section {
                    label: "first".into(),
                    count: 1,
                    ratio: 1.0,
                },
                clock: Section::default(),
            })
            .expect("first save");

        store
            .save(&Document {
                appearance: Section {
                    label: "second".into(),
                    count: 2,
                    ratio: 0.5,
                },
                clock: Section::default(),
            })
            .expect("second save");

        assert!(store.backup_path().is_file());
        let backup: Document =
            serde_json::from_str(&store.read_backup().expect("backup")).expect("parses");
        assert_eq!(backup.appearance.label, "first");
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_and_says_so() {
        let scratch = Scratch::new("corrupt");
        let store = Store::new(scratch.file("config"));
        fs::write(store.path(), "{ this is not json").expect("writes garbage");

        let loaded = store.load_sections(sections);
        assert!(matches!(loaded, Loaded::Failed { .. }));

        let notices = loaded.notices();
        assert_eq!(notices.len(), 1);
        assert!(
            notices[0].contains("could not be read"),
            "unexpected notice: {}",
            notices[0]
        );
        assert_eq!(loaded.value(), Document::default());
    }

    #[test]
    fn a_corrupt_file_is_repaired_from_the_backup() {
        let scratch = Scratch::new("repair");
        let store = Store::new(scratch.file("config"));

        let good = Document {
            appearance: Section {
                label: "kept".into(),
                count: 7,
                ratio: 0.75,
            },
            clock: Section {
                label: "kept too".into(),
                count: 8,
                ratio: 0.25,
            },
        };
        store.save(&good).expect("saves");
        // A second save promotes the good file to the backup slot...
        store.save(&good).expect("saves again");
        // ...and then the primary is destroyed.
        fs::write(store.path(), "not json at all").expect("corrupts");

        let loaded = store.load_sections(sections);
        assert!(
            !loaded.is_clean(),
            "recovering from a backup is still worth reporting"
        );
        assert!(loaded.notices()[0].contains("backup"));
        assert_eq!(loaded.value(), good, "the backup should have been used");
    }

    #[test]
    fn one_bad_section_does_not_cost_the_others() {
        let scratch = Scratch::new("partial");
        let store = Store::new(scratch.file("config"));
        fs::write(
            store.path(),
            r#"{"appearance": {"label": "dark", "count": 2}, "clock": "not an object"}"#,
        )
        .expect("writes");

        let loaded = store.load_sections(sections);
        let notices = loaded.notices();
        let document = loaded.value();

        assert_eq!(document.appearance.label, "dark");
        assert_eq!(document.appearance.count, 2);
        // The broken section falls back...
        assert_eq!(document.clock, Section::default());
        // ...and is reported, so the user is not left wondering.
        assert_eq!(notices.len(), 1, "unexpected notices: {notices:?}");
        assert!(
            notices[0].starts_with("clock"),
            "unexpected: {}",
            notices[0]
        );
    }

    #[test]
    fn one_bad_field_does_not_cost_its_siblings() {
        let scratch = Scratch::new("field");
        let store = Store::new(scratch.file("config"));
        fs::write(
            store.path(),
            r#"{"clock": {"label": "twelve", "count": "not a number"}}"#,
        )
        .expect("writes");

        let loaded = store.load_sections(|fields| {
            let label: String = fields.field(&["clock", "label"]);
            let count: u32 = fields.field(&["clock", "count"]);
            (label, count)
        });

        let notices = loaded.notices();
        let (label, count) = loaded.value();
        assert_eq!(label, "twelve", "a good field is preserved");
        assert_eq!(count, 0, "the bad field is defaulted");
        assert_eq!(notices.len(), 1);
    }

    #[test]
    fn a_field_that_is_absent_is_not_a_recovery() {
        let scratch = Scratch::new("absent");
        let store = Store::new(scratch.file("config"));
        fs::write(store.path(), r#"{"clock": {"label": "twelve"}}"#).expect("writes");

        let loaded = store.load::<Document>();
        assert!(loaded.is_clean(), "absent is normal, not damage");
        assert_eq!(loaded.value().clock.label, "twelve");
    }

    #[test]
    fn an_entirely_empty_document_is_clean() {
        let scratch = Scratch::new("empty");
        let store = Store::new(scratch.file("config"));
        fs::write(store.path(), "{}").expect("writes");

        let loaded = store.load_sections(sections);
        assert!(loaded.is_clean());
        assert_eq!(loaded.value(), Document::default());
    }

    #[test]
    fn a_json_array_where_an_object_belongs_recovers() {
        let scratch = Scratch::new("array");
        let store = Store::new(scratch.file("config"));
        fs::write(store.path(), r#"{"appearance": [1, 2, 3]}"#).expect("writes");

        let loaded = store.load_sections(sections);
        let notices = loaded.notices();
        assert_eq!(notices.len(), 1);
        assert_eq!(loaded.value(), Document::default());
    }

    #[test]
    fn saving_creates_missing_directories() {
        let scratch = Scratch::new("mkdir");
        let nested = scratch.0.join("deep/deeper");
        let store = Store::new(nested.join("config.json"));

        store.save(&Document::default()).expect("saves");
        assert!(store.path().is_file());
    }

    #[test]
    fn in_dir_creates_the_directory() {
        let scratch = Scratch::new("indir");
        let dir = scratch.0.join("state");
        let store = Store::in_dir(&dir, "oclock").expect("creates");
        assert!(dir.is_dir());
        assert!(store.path().ends_with("oclock.json"));
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let scratch = Scratch::new("cleanup");
        let store = Store::new(scratch.file("config"));
        store.save(&Document::default()).expect("saves");
        assert!(!store.temporary_path().exists());
    }

    #[test]
    fn direct_field_lookup_by_path() {
        let scratch = Scratch::new("paths");
        let store = Store::new(scratch.file("config"));
        fs::write(
            store.path(),
            r#"{"a": {"b": {"c": 5}}, "broken": {"b": {"c": "x"}}}"#,
        )
        .expect("writes");

        let document: Value =
            serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).expect("parses");
        let mut loader = Loader::new(&document);

        assert_eq!(loader.field::<u32>(&["a", "b", "c"]), 5);
        assert_eq!(loader.field::<u32>(&["a", "missing", "c"]), 0);
        assert_eq!(loader.field::<u32>(&["missing"]), 0);
        assert_eq!(loader.field::<u32>(&["broken", "b", "c"]), 0);
        assert_eq!(loader.into_recoveries().len(), 1);
    }

    #[test]
    fn the_loader_can_note_a_problem_it_found_itself() {
        let document = Value::Null;
        let mut loader = Loader::new(&document);
        loader.note("alarms[0].sound", "unknown sound name");
        let recoveries = loader.into_recoveries();
        assert_eq!(recoveries.len(), 1);
        assert_eq!(recoveries[0].path, "alarms[0].sound");
        assert!(recoveries[0].to_string().contains("unknown sound name"));
    }

    #[test]
    fn error_messages_name_the_path() {
        let error = StoreError::Io {
            path: PathBuf::from("/tmp/oclock.json"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(error.to_string().contains("/tmp/oclock.json"));
        assert!(error.to_string().contains("denied"));
    }

    #[test]
    fn sibling_paths_are_derived_correctly() {
        let path = Path::new("/home/user/.config/oclock/settings.json");
        assert_eq!(
            sibling(path, "bak"),
            Path::new("/home/user/.config/oclock/settings.bak")
        );
        assert_eq!(
            sibling(path, "tmp"),
            Path::new("/home/user/.config/oclock/settings.tmp")
        );
    }
}
