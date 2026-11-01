// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Channel detection inside synthetic Wine prefixes; runs on every platform.

use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use p4kx_channels::{ChannelSource, SearchPaths, WineEnvironment, detect, known_library_folders};
use p4kx_testkit::write_build_manifest;

type TestResult = Result<(), Box<dyn Error>>;

const LAUNCHER_IN_PREFIX: [&str; 6] = [
    "drive_c",
    "users",
    "steamuser",
    "AppData",
    "Roaming",
    "rsilauncher",
];

fn channel_folder(library: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let channel = library.join("LIVE");
    std::fs::create_dir_all(&channel)?;
    std::fs::write(channel.join("Data.p4k"), b"")?;
    write_build_manifest(&channel, "sc-alpha-4.7.0", "4.7.0.1", "11674325", "id")?;
    Ok(channel)
}

fn game_library(prefix: &Path) -> PathBuf {
    prefix
        .join("drive_c")
        .join("Program Files")
        .join("Roberts Space Industries")
        .join("StarCitizen")
}

fn write_launcher_log(prefix: &Path, line_folder: &str) -> TestResult {
    let launcher = LAUNCHER_IN_PREFIX
        .iter()
        .fold(prefix.to_path_buf(), |path, part| path.join(part));
    let logs = launcher.join("logs");
    std::fs::create_dir_all(&logs)?;
    let escaped = serde_json::to_string(line_folder)?;
    let inner = escaped.trim_matches('"');
    std::fs::write(
        logs.join("log.log"),
        format!("{{ \"msg\": \"Launching Star Citizen LIVE from ({inner})\" }}\n"),
    )?;
    Ok(())
}

fn with_prefix(prefix: &Path) -> SearchPaths {
    SearchPaths {
        wine_prefixes: vec![prefix.to_path_buf()],
        ..SearchPaths::default()
    }
}

fn environment(home: Option<&str>, prefix: Option<&str>) -> WineEnvironment {
    WineEnvironment::new(
        home.map(|home| Path::new(home).to_path_buf()),
        prefix.map(OsString::from),
    )
}

#[cfg(unix)]
const ABSOLUTE: &str = "/home/pilot";
#[cfg(windows)]
const ABSOLUTE: &str = r"C:\Users\pilot";

#[test]
fn the_default_prefix_is_below_the_home_folder() {
    let candidates = environment(Some(ABSOLUTE), None).candidate_prefixes();
    assert_eq!(
        candidates,
        vec![Path::new(ABSOLUTE).join("Games").join("star-citizen")]
    );
}

#[test]
fn the_wineprefix_variable_comes_first_and_a_prefix_is_listed_once() {
    let custom = Path::new(ABSOLUTE).join("wine");
    let candidates = environment(Some(ABSOLUTE), custom.to_str()).candidate_prefixes();
    assert_eq!(
        candidates,
        vec![
            custom,
            Path::new(ABSOLUTE).join("Games").join("star-citizen")
        ]
    );
    let default = Path::new(ABSOLUTE).join("Games").join("star-citizen");
    let same = environment(Some(ABSOLUTE), default.to_str()).candidate_prefixes();
    assert_eq!(same, vec![default]);
}

#[test]
fn an_empty_or_relative_variable_and_a_relative_home_are_ignored() {
    assert_eq!(
        environment(Some(ABSOLUTE), Some(""))
            .candidate_prefixes()
            .len(),
        1
    );
    assert_eq!(
        environment(Some(ABSOLUTE), Some("wine"))
            .candidate_prefixes()
            .len(),
        1
    );
    assert_eq!(
        environment(Some("home"), None).candidate_prefixes().len(),
        0
    );
    assert_eq!(environment(None, None).candidate_prefixes().len(), 0);
}

#[test]
fn only_existing_prefixes_are_kept() -> TestResult {
    let root = tempfile::tempdir()?;
    let present = root.path().join("present");
    std::fs::create_dir(&present)?;
    let missing = root.path().join("missing");
    let environment = WineEnvironment::new(None, Some(missing.into_os_string()));
    assert_eq!(environment.existing_prefixes().len(), 0);
    let environment = WineEnvironment::new(None, Some(present.clone().into_os_string()));
    assert_eq!(environment.existing_prefixes(), vec![present]);
    Ok(())
}

#[test]
fn the_default_library_of_a_prefix_is_searched() -> TestResult {
    let root = tempfile::tempdir()?;
    let channel = channel_folder(&game_library(root.path()))?;
    let channels = detect(&with_prefix(root.path()));
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].archive(), channel.join("Data.p4k"));
    assert_eq!(channels[0].source(), ChannelSource::DefaultPath);
    Ok(())
}

#[test]
fn a_missing_prefix_is_skipped_silently() -> TestResult {
    let root = tempfile::tempdir()?;
    let missing = root.path().join("does-not-exist");
    let paths = with_prefix(&missing);
    assert_eq!(detect(&paths).len(), 0);
    assert_eq!(known_library_folders(&paths), vec![game_library(&missing)]);
    Ok(())
}

#[test]
fn a_prefix_without_the_game_finds_nothing() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::create_dir_all(root.path().join("drive_c").join("windows"))?;
    assert_eq!(detect(&with_prefix(root.path())).len(), 0);
    Ok(())
}

#[test]
fn the_launcher_log_in_a_prefix_names_a_library_on_the_c_drive() -> TestResult {
    let root = tempfile::tempdir()?;
    let library = root.path().join("drive_c").join("Games").join("SC");
    let channel = channel_folder(&library)?;
    write_launcher_log(root.path(), r"C:\Games\SC\LIVE")?;
    let paths = with_prefix(root.path());
    let channels = detect(&paths);
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].archive(), channel.join("Data.p4k"));
    assert_eq!(channels[0].source(), ChannelSource::Launcher);
    assert!(known_library_folders(&paths).contains(&library));
    Ok(())
}

#[test]
fn a_library_named_twice_is_listed_once() -> TestResult {
    let root = tempfile::tempdir()?;
    channel_folder(&game_library(root.path()))?;
    write_launcher_log(
        root.path(),
        r"C:\Program Files\Roberts Space Industries\StarCitizen\LIVE",
    )?;
    let paths = with_prefix(root.path());
    assert_eq!(detect(&paths).len(), 1);
    let known = known_library_folders(&paths);
    assert_eq!(
        known
            .iter()
            .filter(|folder| **folder == game_library(root.path()))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn a_log_path_leaving_the_prefix_is_not_followed() -> TestResult {
    let root = tempfile::tempdir()?;
    write_launcher_log(root.path(), r"C:\Games\..\..\..\LIVE")?;
    assert_eq!(detect(&with_prefix(root.path())).len(), 0);
    Ok(())
}

#[cfg(unix)]
#[test]
fn another_drive_of_the_prefix_is_followed_through_dosdevices() -> TestResult {
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("prefix");
    let disk = root.path().join("disk");
    let channel = channel_folder(&disk.join("SC"))?;
    std::fs::create_dir_all(prefix.join("dosdevices"))?;
    std::os::unix::fs::symlink(&disk, prefix.join("dosdevices").join("d:"))?;
    write_launcher_log(&prefix, r"D:\SC\LIVE")?;
    let channels = detect(&with_prefix(&prefix));
    assert_eq!(channels.len(), 1);
    assert_eq!(
        std::fs::canonicalize(channels[0].archive())?,
        std::fs::canonicalize(channel.join("Data.p4k"))?
    );
    Ok(())
}

#[test]
fn a_manual_folder_and_archive_are_found_again() -> TestResult {
    let root = tempfile::tempdir()?;
    let folder = channel_folder(&root.path().join("elsewhere"))?;
    let other = root.path().join("other");
    std::fs::create_dir_all(&other)?;
    std::fs::write(other.join("Data.p4k"), b"")?;
    let paths = SearchPaths {
        manual_folders: vec![folder],
        manual_archives: vec![other.join("Data.p4k")],
        ..SearchPaths::default()
    };
    let channels = detect(&paths);
    assert_eq!(channels.len(), 2);
    assert!(
        channels
            .iter()
            .all(|channel| channel.source() == ChannelSource::Manual)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_windows_path_in_the_native_launcher_log_is_not_listed() -> TestResult {
    let root = tempfile::tempdir()?;
    let logs = root.path().join("rsilauncher").join("logs");
    std::fs::create_dir_all(&logs)?;
    std::fs::write(
        logs.join("log.log"),
        "{ \"msg\": \"Launching Star Citizen LIVE from (C:\\Games\\SC\\LIVE)\" }\n",
    )?;
    let paths = SearchPaths {
        launcher: Some(root.path().join("rsilauncher")),
        ..SearchPaths::default()
    };
    assert_eq!(known_library_folders(&paths).len(), 0);
    Ok(())
}
