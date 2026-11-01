// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Archive key stored in the user's profile and the precedence of its sources.

use std::error::Error;

use p4kx_foundation::ArchiveKey;
use p4kx_settings::{KeyChoiceError, KeySource, KeyStore, KeyStoreError, choose_key};

type TestResult = Result<(), Box<dyn Error>>;

const FIRST: &str = "000102030405060708090a0b0c0d0e0f";
const SECOND: &str = "f0e0d0c0b0a090807060504030201000";

fn key(text: &str) -> Result<ArchiveKey, Box<dyn Error>> {
    Ok(ArchiveKey::from_hex(text)?)
}

fn store(dir: &tempfile::TempDir) -> KeyStore {
    KeyStore::new(dir.path().join("config").join("archive.key"))
}

#[test]
fn nothing_is_stored_at_first() -> TestResult {
    let dir = tempfile::tempdir()?;

    assert_eq!(store(&dir).load()?, None, "no key");
    assert!(!store(&dir).remove()?, "nothing to remove");
    Ok(())
}

#[test]
fn a_saved_key_is_loaded_replaced_and_removed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);

    store.save(&key(FIRST)?)?;
    assert_eq!(store.load()?, Some(key(FIRST)?), "first key");

    store.save(&key(SECOND)?)?;
    assert_eq!(store.load()?, Some(key(SECOND)?), "replaced");

    assert!(store.remove()?, "was stored");
    assert_eq!(store.load()?, None, "removed");
    Ok(())
}

#[test]
fn saving_leaves_no_temporary_file_behind() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);

    store.save(&key(FIRST)?)?;

    let names: Vec<_> = std::fs::read_dir(dir.path().join("config"))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(names, vec!["archive.key"], "only the key file");
    Ok(())
}

#[cfg(unix)]
#[test]
fn the_key_file_is_readable_by_the_user_alone() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    store.save(&key(FIRST)?)?;

    let mode = std::fs::metadata(store.file())?.permissions().mode();

    assert_eq!(mode & 0o777, 0o600, "mode");
    Ok(())
}

#[cfg(unix)]
#[test]
fn replacing_a_key_file_with_wider_permissions_leaves_mode_0600() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    store.save(&key(FIRST)?)?;
    std::fs::set_permissions(store.file(), std::fs::Permissions::from_mode(0o644))?;

    store.save(&key(SECOND)?)?;

    let mode = std::fs::metadata(store.file())?.permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "mode");
    Ok(())
}

#[test]
fn a_file_that_is_no_key_is_malformed_and_its_error_hides_the_content() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    std::fs::create_dir_all(dir.path().join("config"))?;
    std::fs::write(store.file(), "hunter2-not-a-key-at-all-nope-nope")?;

    let error = store.load().err().ok_or("loaded")?;

    assert!(matches!(error, KeyStoreError::Malformed { .. }), "{error}");
    assert!(
        !format!("{error} {error:?}").contains("hunter2"),
        "the content is not in the error"
    );
    Ok(())
}

#[test]
fn the_flag_beats_the_variable_and_the_variable_beats_the_profile() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    store.save(&key("11111111111111111111111111111111")?)?;

    let chosen = choose_key(Some(FIRST), Some(SECOND), Some(&store))?.ok_or("none")?;
    assert_eq!(
        (chosen.key, chosen.source),
        (key(FIRST)?, KeySource::Flag),
        "flag"
    );

    let chosen = choose_key(None, Some(SECOND), Some(&store))?.ok_or("none")?;
    assert_eq!(
        (chosen.key, chosen.source),
        (key(SECOND)?, KeySource::Environment),
        "variable"
    );

    let chosen = choose_key(Some(" "), Some(""), Some(&store))?.ok_or("none")?;
    assert_eq!(chosen.source, KeySource::Profile, "empty values are unset");
    Ok(())
}

#[test]
fn without_any_source_there_is_no_key() -> TestResult {
    assert_eq!(choose_key(None, None, None)?, None, "none");
    Ok(())
}

#[test]
fn a_mistyped_key_is_reported_and_not_replaced_by_a_lower_source() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    store.save(&key(FIRST)?)?;

    let flag = choose_key(Some("abc"), None, Some(&store))
        .err()
        .ok_or("flag")?;
    let variable = choose_key(None, Some("zz"), Some(&store))
        .err()
        .ok_or("variable")?;

    assert!(matches!(flag, KeyChoiceError::Flag(_)), "{flag}");
    assert!(
        matches!(variable, KeyChoiceError::Environment(_)),
        "{variable}"
    );
    Ok(())
}

#[test]
fn a_leftover_temporary_file_is_neither_followed_nor_overwritten() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    let folder = store.file().parent().ok_or("a folder")?;
    std::fs::create_dir_all(folder)?;
    let stale = folder.join(format!("archive.key.{}.0.tmp", std::process::id()));
    std::fs::write(&stale, "left behind")?;

    store.save(&key(FIRST)?)?;

    assert_eq!(store.load()?, Some(key(FIRST)?), "saved");
    assert_eq!(std::fs::read_to_string(&stale)?, "left behind", "untouched");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_missing_key_folder_is_created_readable_by_the_user_alone() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir()?;
    let store = store(&dir);
    store.save(&key(FIRST)?)?;

    let folder = dir.path().join("config");
    let mode = std::fs::metadata(&folder)?.permissions().mode();
    assert_eq!(mode & 0o777, 0o700, "mode");
    Ok(())
}
