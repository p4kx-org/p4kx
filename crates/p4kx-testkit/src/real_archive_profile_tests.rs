// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use p4kx_foundation::{ArchiveKey, FOLDERS_ROOT_VARIABLE};

use super::{REAL_ARCHIVE_KEY_FILE_VAR, REAL_ARCHIVE_KEY_VAR, real_archive_key};
use crate::{other_test_key, test_key};

type TestResult = Result<(), Box<dyn Error>>;

const CHILD_CASE: &str = "P4KX_TESTKIT_PROFILE_CHILD";

#[test]
fn an_explicit_profile_uses_the_current_key_then_the_legacy_key() -> TestResult {
    let root = tempfile::tempdir()?;
    let config = root.path().join("config");
    let current = store_key(&config, "P4KX", &test_key())?;
    store_key(&config, "P4K-Extractor", &other_test_key())?;
    check_child(&mut child(root.path(), "valid")?)?;

    fs::remove_file(current)?;
    store_key(&config, "P4K-Extractor", &test_key())?;
    check_child(&mut child(root.path(), "valid")?)
}

#[test]
fn a_relative_profile_override_blocks_the_default_profile() -> TestResult {
    let root = tempfile::tempdir()?;
    check_child(child(root.path(), "missing")?.env(FOLDERS_ROOT_VARIABLE, "relative"))
}

#[cfg(target_os = "linux")]
#[test]
fn the_default_linux_profile_reads_the_key_from_absolute_xdg_config() -> TestResult {
    let root = tempfile::tempdir()?;
    let config = root.path().join("xdg-config");
    store_key(&config, "P4KX", &test_key())?;
    check_child(
        child(root.path(), "valid")?
            .env_remove(FOLDERS_ROOT_VARIABLE)
            .env("XDG_CONFIG_HOME", config),
    )
}

fn store_key(config: &Path, folder: &str, key: &ArchiveKey) -> io::Result<PathBuf> {
    let folder = config.join(folder);
    fs::create_dir_all(&folder)?;
    let path = folder.join("archive.key");
    fs::write(&path, key.to_hex())?;
    Ok(path)
}

fn child(root: &Path, case: &str) -> io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", "real_archive::profile_tests::read_test_profile"])
        .current_dir(root)
        .env(CHILD_CASE, case)
        .env(FOLDERS_ROOT_VARIABLE, root)
        .env_remove(REAL_ARCHIVE_KEY_VAR)
        .env_remove(REAL_ARCHIVE_KEY_FILE_VAR);
    Ok(command)
}

fn check_child(command: &mut Command) -> TestResult {
    let output = command.output()?;
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && report.contains("1 passed; 0 failed"),
        "the isolated key lookup must run once and pass: {report}"
    );
    Ok(())
}

#[test]
fn read_test_profile() -> TestResult {
    match std::env::var(CHILD_CASE).ok().as_deref() {
        None => Ok(()),
        Some("valid") => {
            assert_eq!(
                real_archive_key()?,
                test_key(),
                "the selected synthetic profile key"
            );
            Ok(())
        }
        Some("missing") => {
            let missing = real_archive_key().expect_err("an unusable override has no profile");
            assert!(
                missing.to_string().contains("the stored key (not set)"),
                "the typed error reports an unavailable profile"
            );
            Ok(())
        }
        Some(_) => Err("unknown profile test case".into()),
    }
}
