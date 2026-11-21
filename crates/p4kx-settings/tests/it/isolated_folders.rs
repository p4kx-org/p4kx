// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

use p4kx_settings::{
    FOLDERS_ROOT_VARIABLE, FolderKind, KeyStore, Settings, base_folder, cache_folder, data_folder,
    default_settings_file,
};

type TestResult = Result<(), Box<dyn Error>>;
const CHILD_MODE: &str = "P4KX_FOLDERS_TEST_MODE";

#[test]
fn a_child_checks_the_actual_process_folders() -> TestResult {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        return Ok(());
    };
    if mode == "invalid" {
        for kind in [FolderKind::Config, FolderKind::Cache, FolderKind::Data] {
            assert_eq!(base_folder(kind), None, "no default fallback for {kind:?}");
        }
        assert_eq!(default_settings_file(), None, "no profile file fallback");
        assert!(KeyStore::in_profile().is_none(), "no profile key fallback");
        return Ok(());
    }
    let root = PathBuf::from(std::env::var_os(FOLDERS_ROOT_VARIABLE).ok_or("a root")?);
    assert_eq!(
        base_folder(FolderKind::Config),
        Some(root.join("config")),
        "explicit root wins over Windows known folders and XDG"
    );
    assert_eq!(
        cache_folder(),
        Some(root.join("cache/P4KX")),
        "isolated cache"
    );
    assert_eq!(data_folder(), Some(root.join("data/P4KX")), "isolated data");
    let file = default_settings_file().ok_or("an isolated settings path")?;
    assert_eq!(
        file,
        root.join("config/P4KX/settings.toml"),
        "isolated settings"
    );
    Settings::default().save(&file)?;
    assert!(
        file.is_file(),
        "the settings write stays under the explicit root"
    );
    Ok(())
}

#[test]
fn explicit_roots_are_checked_in_fresh_processes() -> TestResult {
    let root = tempfile::tempdir()?;
    let isolated = root.path().join("Unicode Ä 中 profile");
    for (mode, folder) in [
        ("valid", isolated.as_os_str()),
        ("invalid", std::ffi::OsStr::new("relative-profile")),
        ("invalid", std::ffi::OsStr::new("")),
    ] {
        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "isolated_folders::a_child_checks_the_actual_process_folders",
                "--nocapture",
            ])
            .env(CHILD_MODE, mode)
            .env(FOLDERS_ROOT_VARIABLE, folder)
            .env("XDG_CONFIG_HOME", root.path().join("unwanted-config"))
            .env("APPDATA", root.path().join("unwanted-appdata"))
            .output()?;
        assert!(
            output.status.success(),
            "{mode} root: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        isolated.join("config/P4KX/settings.toml").is_file(),
        "a native child wrote its settings under the isolated root"
    );
    assert!(
        !root.path().join("unwanted-config").exists(),
        "the platform fallback was not written"
    );
    assert!(
        !root.path().join("unwanted-appdata").exists(),
        "the environment fallback was not written"
    );
    Ok(())
}
