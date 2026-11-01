// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The write guard on the layout of a Wine prefix: `dosdevices` links that name the same folder.

#![cfg(unix)]

use std::error::Error;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use p4kx_guard::Installation;

type TestResult = Result<(), Box<dyn Error>>;

/// Builds `prefix/drive_c/.../StarCitizen/LIVE/Bin64` with `dosdevices/c:` and `dosdevices/z:`.
fn prefix_in(root: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let prefix = root.join("prefix");
    let game = prefix
        .join("drive_c")
        .join("Program Files")
        .join("Roberts Space Industries")
        .join("StarCitizen");
    fs::create_dir_all(game.join("LIVE").join("Bin64"))?;
    fs::create_dir_all(prefix.join("dosdevices"))?;
    symlink("../drive_c", prefix.join("dosdevices").join("c:"))?;
    symlink("/", prefix.join("dosdevices").join("z:"))?;
    Ok((prefix, game))
}

#[test]
fn the_c_drive_link_does_not_hide_the_installation() -> TestResult {
    let root = tempfile::tempdir()?;
    let (prefix, game) = prefix_in(root.path())?;
    let mut installation = Installation::new();
    installation.add(&game);
    let through_c = prefix
        .join("dosdevices")
        .join("c:")
        .join("Program Files")
        .join("Roberts Space Industries")
        .join("StarCitizen")
        .join("LIVE")
        .join("new");
    assert!(installation.check(&through_c).is_err(), "through c:");
    Ok(())
}

#[test]
fn the_root_drive_link_does_not_hide_the_installation() -> TestResult {
    let root = tempfile::tempdir()?;
    let (prefix, game) = prefix_in(root.path())?;
    let mut installation = Installation::new();
    installation.add(&game);
    let absolute = fs::canonicalize(&game)?;
    let through_z = prefix
        .join("dosdevices")
        .join("z:")
        .join(absolute.strip_prefix("/")?)
        .join("LIVE")
        .join("new");
    assert!(installation.check(&through_z).is_err(), "through z:");
    Ok(())
}

#[test]
fn a_protected_folder_entered_through_a_link_is_protected_by_its_real_place() -> TestResult {
    let root = tempfile::tempdir()?;
    let (prefix, game) = prefix_in(root.path())?;
    let mut installation = Installation::new();
    installation.add(
        &prefix
            .join("dosdevices")
            .join("c:")
            .join("Program Files")
            .join("Roberts Space Industries")
            .join("StarCitizen"),
    );
    assert!(installation.check(&game.join("LIVE").join("x")).is_err());
    Ok(())
}

#[test]
fn the_rest_of_the_prefix_stays_writable() -> TestResult {
    let root = tempfile::tempdir()?;
    let (prefix, game) = prefix_in(root.path())?;
    let mut installation = Installation::new();
    installation.add(&game);
    let users = prefix
        .join("dosdevices")
        .join("c:")
        .join("users")
        .join("steamuser");
    assert!(installation.check(&users).is_ok(), "another folder of C:");
    assert!(
        installation
            .check(&root.path().join("export").join("StarCitizen"))
            .is_ok(),
        "an export folder named like the game"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn another_letter_case_is_another_folder_on_a_case_sensitive_file_system() -> TestResult {
    let root = tempfile::tempdir()?;
    let (_, game) = prefix_in(root.path())?;
    let mut installation = Installation::new();
    installation.add(&game);
    let shouted = game.with_file_name("STARCITIZEN").join("x");
    assert!(installation.check(&shouted).is_ok());
    Ok(())
}
