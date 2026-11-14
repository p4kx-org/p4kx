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
fn the_windows_config_folder_is_the_known_folder() -> TestResult {
    let output = location_child()?.output()?;
    let text = String::from_utf8(output.stdout)?;
    let expected = dirs::config_dir().map_or_else(String::new, |path| path.display().to_string());
    assert!(output.status.success(), "profile child failed: {text}");
    assert!(
        text.lines()
            .any(|line| line.strip_prefix("config=") == Some(expected.as_str())),
        "Windows known config folder: {text}"
    );
    Ok(())
}

const CHILD_MARKER: &str = "P4KX_LOCATIONS_CHILD";

fn location_child() -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .args([
            "--exact",
            "locations::child_prints_the_folders",
            "--nocapture",
        ])
        .env(CHILD_MARKER, "1")
        .env_remove(p4kx_settings::FOLDERS_ROOT_VARIABLE);
    Ok(command)
}

/// Prints the folders of this process for the parent test; does nothing outside the child.
#[test]
#[expect(
    clippy::print_stdout,
    reason = "the parent test reads the child's stdout"
)]
fn child_prints_the_folders() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }
    let show =
        |folder: Option<PathBuf>| folder.map_or_else(String::new, |p| p.display().to_string());
    println!(
        "config={}",
        show(p4kx_settings::base_folder(FolderKind::Config))
    );
    println!("cache={}", show(p4kx_settings::cache_folder()));
    println!("data={}", show(p4kx_settings::data_folder()));
}

#[cfg(target_os = "linux")]
#[test]
fn on_linux_the_variables_of_the_process_decide_the_folders() -> TestResult {
    let root = tempfile::tempdir()?;
    let output = location_child()?
        .env("XDG_CONFIG_HOME", root.path().join("c"))
        .env("XDG_CACHE_HOME", root.path().join("k"))
        .env("XDG_DATA_HOME", root.path().join("d"))
        .output()?;
    let text = String::from_utf8(output.stdout)?;

    assert!(output.status.success(), "child failed: {text}");
    for (name, folder) in [("config", "c"), ("cache", "k"), ("data", "d")] {
        let expected = root.path().join(folder);
        let expected = if name == "config" {
            expected
        } else {
            expected.join("P4KX")
        };
        assert!(
            text.contains(&format!(
                "{name}={}
",
                expected.display()
            )),
            "{name} in {text}"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn macos_uses_the_standard_library_directories_with_a_unicode_home() -> TestResult {
    let root = tempfile::tempdir()?;
    let home = root.path().join("P4KX ü Ω");
    std::fs::create_dir(&home)?;
    let unwanted = root.path().join("unwanted-xdg");
    let output = location_child()?
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &unwanted)
        .env("XDG_CACHE_HOME", &unwanted)
        .env("XDG_DATA_HOME", &unwanted)
        .output()?;
    let text = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "profile child failed: {text}");
    for (name, path) in [
        ("config", "Library/Application Support"),
        ("cache", "Library/Caches/P4KX"),
        ("data", "Library/Application Support/P4KX"),
    ] {
        let expected = format!("{name}={}", home.join(path).display());
        assert!(text.lines().any(|line| line == expected), "{name}: {text}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn saving_the_settings_creates_a_missing_folder_readable_by_the_user_alone() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir()?;
    let file = root.path().join("profile").join("settings.toml");
    p4kx_settings::Settings::default().save(&file)?;

    let mode = std::fs::metadata(root.path().join("profile"))?
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700, "mode");
    Ok(())
}
