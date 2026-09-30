//! The desktop integration, checked.
//!
//! The application ID is one string that has to be spelled the same way in
//! three files — the desktop entry, the AppStream metadata and the icon's
//! name — and an icon set that has to be present at every size the freedesktop
//! icon theme specification names. None of that is compiled into the binary, so
//! nothing else in the crate would notice it going wrong: a renamed icon leaves
//! a launcher showing a generic placeholder, and a stale PNG set leaves a
//! package that installs without complaint and then draws the wrong size.
//!
//! So it is checked here, against the files in the repository, with no
//! dependency beyond the standard library — parsing a `.desktop` entry and an
//! AppStream document is a few lines of string work, and a test that can say
//! precisely which field it meant is more use than a general XML parser would
//! be.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The freedesktop application ID. Spelled once here, and the files are
/// checked against it rather than against each other, so a rename is one edit.
const APP_ID: &str = "org.odsos.OClock";

/// The binary the desktop entry runs, and the one Cargo builds.
const BINARY: &str = "oclock";

/// The sizes the icon theme specification names for the `apps` context, plus
/// 512 because that is the size software stores an icon at.
const ICON_SIZES: [u32; 9] = [16, 22, 24, 32, 48, 64, 128, 256, 512];

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn data(file: &str) -> PathBuf {
    repository().join("data").join(file)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
}

/// A `.desktop` entry as its groups of keys.
///
/// A desktop entry is a flat list of `Key=Value` lines in one group; a
/// duplicate key is not something the specification allows, so the last one
/// wins rather than the first.
fn desktop_entry(path: &Path) -> BTreeMap<String, String> {
    let mut entry = BTreeMap::new();
    for line in read(path).lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            entry.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    entry
}

/// The text of the first `<tag>` in an AppStream document.
///
/// The opening tag may carry attributes — `<launchable type="desktop-id">`
/// does — so the search is for the bare name followed by a delimiter rather
/// than for a literal `<tag>`. Comments are stripped first: the metadata
/// carries a header that names the tags it is describing, and a search that
/// did not know about comments would find that header instead of the value.
fn metainfo_value(document: &str, tag: &str) -> String {
    let uncommented = strip_comments(document);

    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let after = uncommented
        .find(&open)
        .unwrap_or_else(|| panic!("the metadata should have a <{tag}> element"))
        + open.len();

    let content_start = match uncommented[after..].find('>') {
        Some(offset) => {
            let start = after + offset + 1;
            if uncommented[after..after + offset].trim_end().ends_with('/') {
                return String::new(); // A self-closing element has no content.
            }
            start
        }
        None => panic!("the <{tag}> element should be closed"),
    };

    let content_end = uncommented[content_start..]
        .find(&close)
        .unwrap_or_else(|| panic!("the <{tag}> element should be closed"))
        + content_start;
    uncommented[content_start..content_end].trim().to_string()
}

/// The name inside the `<developer>` element.
///
/// The component has a `<name>` of its own — the application's — so a document
/// with both cannot be asked for "the name" and answered with whichever came
/// first.
fn developer_name(document: &str) -> String {
    let uncommented = strip_comments(document);
    let after = uncommented
        .find("<developer")
        .expect("the metadata names a developer");
    metainfo_value(&uncommented[after..], "name")
}

/// An AppStream document with its comments removed.
///
/// The metadata explains itself in comments — what each tag is for, and why a
/// field is absent — and a check about what the document *declares* has to be
/// able to see past them.
fn strip_comments(document: &str) -> String {
    let mut uncommented = String::with_capacity(document.len());
    let mut rest = document;
    while let Some(start) = rest.find("<!--") {
        uncommented.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return String::new(),
        }
    }
    uncommented.push_str(rest);
    uncommented
}

#[test]
fn the_desktop_entry_names_the_application() {
    let entry = desktop_entry(&data(&format!("{APP_ID}.desktop")));

    assert_eq!(
        entry.get("Type").map(String::as_str),
        Some("Application"),
        "it is an application, not a link or a directory"
    );
    assert_eq!(
        entry.get("Name").map(String::as_str),
        Some(oclock::APP_NAME),
        "the launcher's name is the application's name"
    );
    assert_eq!(
        entry.get("Exec").map(String::as_str),
        Some(BINARY),
        "and it runs the binary Cargo builds"
    );
    assert_eq!(
        entry.get("Terminal").map(String::as_str),
        Some("false"),
        "a clock is not a terminal application"
    );

    let icon = entry.get("Icon").expect("the entry names an icon");
    assert_eq!(
        icon, APP_ID,
        "the icon is named for the application ID, so it is found in the theme"
    );

    let categories = entry.get("Categories").expect("the entry is categorised");
    assert!(
        categories.split(';').any(|category| category == "Clock"),
        "Clock is the category a launcher searches for, in {categories}"
    );
}

#[test]
fn the_three_files_agree_on_the_application_id() {
    let metadata = read(&data(&format!("{APP_ID}.metainfo.xml")));

    assert_eq!(
        metainfo_value(&metadata, "id"),
        APP_ID,
        "the AppStream ID is the file's own name"
    );
    assert_eq!(
        metainfo_value(&metadata, "launchable"),
        format!("{APP_ID}.desktop"),
        "and it launches the desktop entry of the same name"
    );
    assert_eq!(
        metainfo_value(&metadata, "name"),
        oclock::APP_NAME,
        "a software centre shows the application's name"
    );
    assert_eq!(
        metainfo_value(&metadata, "metadata_license"),
        "CC0-1.0",
        "the metadata is itself licensed, and permissively"
    );

    // `Cargo.toml` is the authority on the licence; the packaging data is not
    // allowed to answer the question on its own.
    let manifest = read(&repository().join("Cargo.toml"));
    let declared = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("license = "))
        .map(|value| value.trim_matches('"'))
        .expect("the package declares a licence");
    assert_eq!(
        metainfo_value(&metadata, "project_license"),
        declared,
        "the metainfo's project licence is the crate's"
    );
}

/// `ODS` is the creator and `ODS-os` is the platform they belong to.
///
/// They are two different names that both appear in this project's files, and
/// the only way to keep them apart is to say which is which in each place: the
/// creator in the copyright and the developer fields, the ecosystem in the
/// descriptions that talk about what the application runs on.
#[test]
fn the_creator_is_ods_and_the_platform_is_ods_os() {
    let metadata = read(&data(&format!("{APP_ID}.metainfo.xml")));

    assert_eq!(
        developer_name(&metadata),
        "ODS",
        "the developer is the creator, whose name is ODS"
    );
    assert!(
        metadata.contains("<developer id=\"org.odsos\">"),
        "and is identified by the ecosystem, which is not the same thing as a name"
    );

    // The platform is named where it is a platform: the description says what
    // the application runs on, not who wrote it.
    let description = metainfo_value(&metadata, "description");
    assert!(
        description.contains("ODS-os"),
        "the description says what the application runs on"
    );
    assert!(
        !description.contains("ODS-os-native"),
        "which is not the same claim as being written for it"
    );
    assert!(
        !description.contains("Copyright"),
        "and a description is not where a copyright notice belongs"
    );

    // Nothing anywhere claims the ecosystem is the copyright holder, or the
    // creator is an operating system.
    for (file, contents) in [
        ("LICENSE", read(&repository().join("LICENSE"))),
        ("README.md", read(&repository().join("README.md"))),
        (APP_ID, metadata),
    ] {
        assert!(
            !contents.contains("Copyright (c) 2026 ODS-os"),
            "{file} must not credit the platform as the copyright holder"
        );
        assert!(
            !contents.contains("<name>ODS-os</name>"),
            "{file} must not name the platform as the developer"
        );
    }
}

/// No URL is invented.
///
/// The metadata's `homepage`, `bugtracker` and `repository` fields all want the
/// project's real location, and this project does not carry one: nothing in the
/// repository, the Cargo metadata or the packaging data says where OClock is
/// hosted or who to report a bug to. Each of those fields is optional, so the
/// honest answer is to leave it out rather than to write a URL assembled from
/// the application's ID or the ecosystem's name — which is exactly the kind of
/// address that looks authoritative and resolves to somebody else's page.
#[test]
fn no_project_url_is_invented_anywhere() {
    let metadata = read(&data(&format!("{APP_ID}.metainfo.xml")));
    let uncommented = strip_comments(&metadata);

    for kind in ["homepage", "bugtracker", "repository", "help"] {
        assert!(
            !uncommented.contains(&format!("<url type=\"{kind}\">")),
            "the metadata declares no {kind}: there is no verified one to declare"
        );
    }
    assert!(
        !uncommented.contains("<url"),
        "no URL of any kind is declared"
    );

    // And nothing in the crate's own metadata has grown one either.
    let manifest = read(&repository().join("Cargo.toml"));
    assert!(
        !manifest.contains("repository ="),
        "Cargo.toml declares no repository: there is no verified one to declare"
    );
    assert!(!manifest.contains("homepage ="), "and no homepage");
    assert!(
        !manifest.contains("documentation ="),
        "and no documentation URL"
    );

    // A repository URL that cannot be checked is worse than none, and this one
    // would be the obvious guess. It is named here so that writing it is a
    // decision somebody makes rather than a typo somebody makes.
    for (file, contents) in [
        ("Cargo.toml", manifest),
        ("metainfo", metadata),
        ("desktop", read(&data(&format!("{APP_ID}.desktop")))),
    ] {
        assert!(
            !contents.contains("github.com"),
            "{file} must not name a repository host it has not been told about"
        );
    }
}

/// The metadata's `id`, and the version it releases, are checked against the
/// crate rather than against each other.
#[test]
fn the_release_names_the_version_the_crate_declares() {
    let metadata = read(&data(&format!("{APP_ID}.metainfo.xml")));
    let manifest = read(&repository().join("Cargo.toml"));
    let version = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim_matches('"'))
        .expect("the crate declares a version");

    assert!(
        metadata.contains(&format!("<release version=\"{version}\"")),
        "the release is {version}, the version the crate builds"
    );
    assert!(
        metadata.contains("<description>"),
        "and a release says what it is, which is what a software centre shows"
    );
}

#[test]
fn the_icon_is_authored_as_vector_and_shipped_at_every_size() {
    let svg = data(&format!("icons/hicolor/scalable/apps/{APP_ID}.svg"));
    let artwork = read(&svg);

    assert!(
        artwork.starts_with("<?xml"),
        "the artwork is an XML document"
    );
    assert!(
        artwork.contains("viewBox=\"0 0 512 512\""),
        "and it is a square, so every size below is a straight scale"
    );

    // The artwork is the design system's, not a lookalike: an icon drawn by
    // hand in a different indigo is exactly the drift this catches.
    let accent = oclock::design::palette::Palette::LIGHT.accent;
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let accent_hex = format!(
        "#{:02X}{:02X}{:02X}",
        channel(accent.r),
        channel(accent.g),
        channel(accent.b)
    );
    assert!(
        artwork.contains(&accent_hex),
        "the icon's face is the palette's accent, {accent_hex}"
    );

    for size in ICON_SIZES {
        let path = data(&format!("icons/hicolor/{size}x{size}/apps/{APP_ID}.png"));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{} should exist: {error}", path.display()));

        assert_eq!(
            &bytes[..8],
            b"\x89PNG\r\n\x1a\n",
            "{} is a PNG",
            path.display()
        );

        // The width and height are big-endian `u32`s at offsets 16 and 20.
        let dimension = |offset: usize| {
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
        };
        assert_eq!(
            (dimension(16), dimension(20)),
            (size, size),
            "{} is {size} pixels square, as its directory says",
            path.display()
        );
    }
}

#[test]
fn there_is_a_symbolic_icon_for_the_places_that_recolour() {
    // GLib appends `-symbolic` to every themed icon name as a fallback, so a
    // title bar or a panel asks for this one whether or not it exists. It is
    // vector only, which is what the symbolic icon convention asks for.
    let path = data(&format!(
        "icons/hicolor/symbolic/apps/{APP_ID}-symbolic.svg"
    ));
    let artwork = read(&path);

    assert!(
        artwork.starts_with("<?xml"),
        "the artwork is an XML document"
    );
    assert!(
        artwork.contains("viewBox=\"0 0 16 16\""),
        "a symbolic icon is drawn on the sixteen pixel grid"
    );
    assert!(
        !artwork.contains("viewBox=\"0 0 512 512\""),
        "and is not the full-colour artwork under another name"
    );
    assert!(
        !artwork.contains("#4F46E5"),
        "a symbolic icon names no colour of its own"
    );
}
