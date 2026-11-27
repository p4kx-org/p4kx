// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx channels` end to end, against synthetic install, launcher and config folders.

use std::error::Error;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

struct Install {
    root: tempfile::TempDir,
}

impl Install {
    fn new() -> Result<Self, Box<dyn Error>> {
        let install = Self {
            root: tempfile::tempdir()?,
        };
        std::fs::create_dir_all(install.default_library())?;
        std::fs::create_dir_all(install.launcher().join("logs"))?;
        Ok(install)
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn default_library(&self) -> PathBuf {
        self.path().join("default")
    }

    fn launcher(&self) -> PathBuf {
        self.path().join("rsilauncher")
    }

    fn config(&self) -> PathBuf {
        self.path().join("config")
    }

    fn p4kx(&self) -> Result<Command, Box<dyn Error>> {
        let mut command = p4kx_testkit::p4kx()?;
        command
            .env("P4KX_CONFIG_DIR", self.config())
            .env("P4KX_LAUNCHER_DIR", self.launcher())
            .env("P4KX_DEFAULT_LIBRARY", self.default_library())
            .arg("channels");
        Ok(command)
    }

    fn rows(&self) -> Result<Vec<[String; 4]>, Box<dyn Error>> {
        let output = self.p4kx()?.output()?;
        assert!(output.status.success(), "channels succeeds: {output:?}");
        let stdout = String::from_utf8(output.stdout)?;
        let mut lines = stdout.lines();
        let header = lines.next().unwrap_or_default();
        assert!(header.starts_with("CHANNEL"), "header: {header}");
        Ok(lines.map(parse_row).collect())
    }
}

fn parse_row(line: &str) -> [String; 4] {
    let mut rest = line;
    let mut word = || {
        let trimmed = rest.trim_start();
        let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let (word, tail) = trimmed.split_at(end);
        rest = tail;
        word.to_owned()
    };
    let name = word();
    let version = word();
    let source = word();
    [name, version, source, rest.trim().to_owned()]
}

fn row(name: &str, version: &str, source: &str, archive: &Path) -> [String; 4] {
    [
        name.to_owned(),
        version.to_owned(),
        source.to_owned(),
        archive.display().to_string(),
    ]
}

fn write_channel(folder: &Path, version: Option<&str>) -> Result<PathBuf, Box<dyn Error>> {
    std::fs::create_dir_all(folder)?;
    let archive = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\readme.txt", b"hello".to_vec()))
        .write_to(&archive)?;
    if let Some(version) = version {
        write_build_manifest(
            folder,
            "sc-alpha-4.10.2",
            version,
            "12913470",
            "2cae0dfc-a941-4dcc-b82a-543b08b52d97",
        )?;
    }
    Ok(archive)
}

fn json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn launch_line(channel_folder: &Path) -> String {
    format!(
        "{{ \"t\":\"2026-10-12 18:56:35.976\", \"[main][info] \": \"[Launcher::launch] Launching Star Citizen LIVE from ({})\"  }},\n",
        json_path(channel_folder)
    )
}

fn install_line(library: &Path) -> String {
    format!(
        "{{ \"t\":\"2026-10-12 18:50:00.000\", \"[main][info] \": \"[Installer] Installing Star Citizen TECH-PREVIEW 4.2-tp.1 at {} (force DP: false)\"  }},\n",
        json_path(library)
    )
}

#[test]
fn channels_lists_the_channels_below_the_default_install_path_with_their_build() -> TestResult {
    let install = Install::new()?;
    let live = write_channel(
        &install.default_library().join("LIVE"),
        Some("4.10.197.2878"),
    )?;
    let ptu = write_channel(&install.default_library().join("PTU"), None)?;
    std::fs::create_dir_all(install.default_library().join("Tools"))?;

    assert_eq!(
        install.rows()?,
        [
            row("LIVE", "4.10.197.2878", "default", &live),
            row("PTU", "unknown", "default", &ptu),
        ],
        "Channel folders with a Data.p4k, missing manifest as unknown Build"
    );
    Ok(())
}

#[test]
fn channels_finds_library_folders_named_in_the_launcher_log() -> TestResult {
    let install = Install::new()?;
    let library = install.path().join("Games").join("StarCitizen (copy)");
    let hotfix = write_channel(&library.join("HOTFIX"), Some("4.10.196.1"))?;
    let eptu = write_channel(&library.join("EPTU"), None)?;
    let other = install.path().join("Other");
    let tech = write_channel(&other.join("TECH-PREVIEW"), Some("4.2.0.1"))?;
    let logs = install.launcher().join("logs");
    std::fs::write(logs.join("log.log"), launch_line(&library.join("HOTFIX")))?;
    std::fs::write(logs.join("log.old.log"), install_line(&other))?;

    assert_eq!(
        install.rows()?,
        [
            row("EPTU", "unknown", "launcher", &eptu),
            row("HOTFIX", "4.10.196.1", "launcher", &hotfix),
            row("TECH-PREVIEW", "4.2.0.1", "launcher", &tech),
        ],
        "every Channel in each library folder the log names"
    );
    Ok(())
}

#[test]
fn channel_detection_never_reads_the_launcher_credential_and_store_files() -> TestResult {
    let install = Install::new()?;
    let decoy = install.path().join("Decoy");
    write_channel(&decoy.join("LIVE"), None)?;
    for name in ["loginData.json", "launcher store.json"] {
        std::fs::write(
            install.launcher().join(name),
            launch_line(&decoy.join("LIVE")),
        )?;
        std::fs::write(
            install.launcher().join("logs").join(name),
            launch_line(&decoy.join("LIVE")),
        )?;
    }

    assert!(
        install.rows()?.is_empty(),
        "nothing from files other than the launcher log"
    );
    Ok(())
}

#[test]
fn manually_added_folders_and_archives_are_stored_in_the_settings_and_listed() -> TestResult {
    let install = Install::new()?;
    let channel_folder = install.path().join("Copies").join("PTU");
    let in_folder = write_channel(&channel_folder, Some("4.11.0.1"))?;
    let library = install.path().join("Library");
    let in_library = write_channel(&library.join("LIVE"), None)?;
    let loose = install.path().join("Loose").join("old.p4k");
    std::fs::create_dir_all(install.path().join("Loose"))?;
    std::fs::copy(&in_folder, &loose)?;

    for path in [&channel_folder, &library, &loose] {
        install
            .p4kx()?
            .arg("add")
            .arg(path)
            .assert()
            .code(0)
            .stdout(predicate::str::contains("added"));
    }

    let settings = std::fs::read_to_string(install.config().join("settings.toml"))?;
    let stored: toml::Table = toml::from_str(&settings)?;
    let channels = stored.get("channels").ok_or("a [channels] table")?;
    let strings = |key: &str| -> Vec<String> {
        channels
            .get(key)
            .and_then(toml::Value::as_array)
            .map_or_default(|values| {
                values
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
    };
    assert_eq!(
        strings("folders"),
        [
            channel_folder.display().to_string(),
            library.display().to_string()
        ],
        "folders in the settings TOML"
    );
    assert_eq!(
        strings("archives"),
        [loose.display().to_string()],
        "Archives in the settings TOML"
    );

    assert_eq!(
        install.rows()?,
        [
            row("PTU", "4.11.0.1", "manual", &in_folder),
            row("LIVE", "unknown", "manual", &in_library),
            row("Loose", "unknown", "manual", &loose),
        ],
        "manual entries are listed"
    );
    Ok(())
}

#[test]
fn a_removed_manual_entry_is_no_longer_listed() -> TestResult {
    let install = Install::new()?;
    let folder = install.path().join("Copies").join("PTU");
    write_channel(&folder, None)?;
    install.p4kx()?.arg("add").arg(&folder).assert().code(0);

    install
        .p4kx()?
        .arg("remove")
        .arg(&folder)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("removed"));

    assert!(install.rows()?.is_empty(), "nothing is listed");
    install.p4kx()?.arg("remove").arg(&folder).assert().code(2);
    Ok(())
}

#[test]
fn adding_a_path_that_does_not_exist_is_invalid_input_with_exit_code_2() -> TestResult {
    let install = Install::new()?;

    install
        .p4kx()?
        .arg("add")
        .arg(install.path().join("missing"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("does not exist"));
    assert!(
        !install.config().join("settings.toml").exists(),
        "the settings stay unwritten"
    );
    Ok(())
}
