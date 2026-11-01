// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The XDG resolution of the base folders, driven by environment input.

use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use p4kx_settings::{FolderKind, XdgEnvironment};

type TestResult = Result<(), Box<dyn Error>>;

fn os(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[test]
fn the_variables_win_when_they_are_absolute() -> TestResult {
    let root = tempfile::tempdir()?;
    let env = XdgEnvironment::new(
        Some(root.path().join("home")),
        Some(os(&root.path().join("c"))),
        Some(os(&root.path().join("k"))),
        Some(os(&root.path().join("d"))),
    );

    assert_eq!(env.base(FolderKind::Config), Some(root.path().join("c")));
    assert_eq!(env.base(FolderKind::Cache), Some(root.path().join("k")));
    assert_eq!(env.base(FolderKind::Data), Some(root.path().join("d")));
    Ok(())
}

#[test]
fn unset_variables_fall_back_to_the_home_folder() -> TestResult {
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let env = XdgEnvironment::new(Some(home.clone()), None, None, None);

    assert_eq!(env.base(FolderKind::Config), Some(home.join(".config")));
    assert_eq!(env.base(FolderKind::Cache), Some(home.join(".cache")));
    assert_eq!(
        env.base(FolderKind::Data),
        Some(home.join(".local").join("share"))
    );
    Ok(())
}

#[test]
fn an_empty_or_relative_variable_is_ignored() -> TestResult {
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let env = XdgEnvironment::new(
        Some(home.clone()),
        Some(OsString::new()),
        Some(OsString::from("relative/cache")),
        Some(OsString::from(".")),
    );

    assert_eq!(env.base(FolderKind::Config), Some(home.join(".config")));
    assert_eq!(env.base(FolderKind::Cache), Some(home.join(".cache")));
    assert_eq!(
        env.base(FolderKind::Data),
        Some(home.join(".local").join("share"))
    );
    Ok(())
}

#[test]
fn without_a_usable_home_or_variable_there_is_no_folder() {
    let env = XdgEnvironment::new(Some(PathBuf::from("relative-home")), None, None, None);
    assert_eq!(env.base(FolderKind::Config), None, "relative home");
    assert_eq!(XdgEnvironment::default().base(FolderKind::Cache), None);
}

#[test]
fn a_variable_alone_needs_no_home_folder() -> TestResult {
    let root = tempfile::tempdir()?;
    let env = XdgEnvironment::new(None, Some(os(root.path())), None, None);

    assert_eq!(
        env.base(FolderKind::Config),
        Some(root.path().to_path_buf())
    );
    assert_eq!(env.base(FolderKind::Data), None, "data has no variable");
    Ok(())
}

#[cfg(windows)]
#[test]
fn the_windows_config_folder_is_the_roaming_app_data_folder() {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from);
    assert_eq!(p4kx_settings::base_folder(FolderKind::Config), roaming);
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_the_platform_folders_follow_the_process_environment() {
    let env = XdgEnvironment::from_process();

    assert_eq!(
        p4kx_settings::base_folder(FolderKind::Config),
        env.base(FolderKind::Config)
    );
    assert_eq!(
        p4kx_settings::cache_folder(),
        env.base(FolderKind::Cache).map(|base| base.join("P4KX"))
    );
    assert_eq!(
        p4kx_settings::data_folder(),
        env.base(FolderKind::Data).map(|base| base.join("P4KX"))
    );
}
