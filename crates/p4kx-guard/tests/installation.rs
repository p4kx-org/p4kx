// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical comparison of write targets with the protected folders.

use std::error::Error;
use std::fs;

use p4kx_guard::{Installation, any_game_process, game_is_running, is_game_process};

type TestResult = Result<(), Box<dyn Error>>;

fn installation_in(root: &std::path::Path) -> Result<Installation, Box<dyn Error>> {
    let folder = root.join("StarCitizen");
    fs::create_dir_all(folder.join("LIVE"))?;
    let mut installation = Installation::new();
    installation.add(&folder);
    Ok(installation)
}

#[test]
fn refuses_a_folder_inside_and_the_folder_itself() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let folder = root.path().join("StarCitizen");
    assert!(installation.check(&folder).is_err(), "the folder itself");
    assert!(installation.check(&folder.join("LIVE")).is_err(), "a child");
    assert!(
        installation
            .check(&folder.join("LIVE").join("new").join("deeper"))
            .is_err(),
        "a target that does not exist yet"
    );
    Ok(())
}

#[test]
fn allows_siblings_and_prefix_lookalikes() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    assert!(
        installation.check(&root.path().join("out")).is_ok(),
        "sibling"
    );
    assert!(
        installation
            .check(&root.path().join("StarCitizen-export"))
            .is_ok(),
        "a name that only starts like the folder"
    );
    assert!(installation.check(root.path()).is_ok(), "the parent");
    Ok(())
}

#[test]
fn dot_dot_does_not_hide_the_installation() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let sneaky = root
        .path()
        .join("out")
        .join("..")
        .join("StarCitizen")
        .join("LIVE");
    assert!(
        installation.check(&sneaky).is_err(),
        "through a sibling and .."
    );
    let missing = root
        .path()
        .join("missing")
        .join("..")
        .join("StarCitizen")
        .join("x");
    assert!(
        installation.check(&missing).is_err(),
        "through a missing folder and .."
    );
    let back_out = root
        .path()
        .join("StarCitizen")
        .join("LIVE")
        .join("..")
        .join("..")
        .join("out");
    assert!(
        installation.check(&back_out).is_ok(),
        "leaving the installation with .."
    );
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn letter_case_does_not_hide_the_installation() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let shouted = root.path().join("STARCITIZEN").join("live").join("x");
    assert!(installation.check(&shouted).is_err(), "other letter case");
    Ok(())
}

#[cfg(windows)]
#[test]
fn verbatim_prefix_does_not_hide_the_installation() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let verbatim = fs::canonicalize(root.path())?.join("StarCitizen").join("x");
    assert!(
        verbatim.to_string_lossy().starts_with(r"\\?\"),
        "canonical form is verbatim"
    );
    assert!(installation.check(&verbatim).is_err(), "verbatim spelling");
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_into_the_installation_is_refused() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let link = root.path().join("out-link");
    std::os::unix::fs::symlink(root.path().join("StarCitizen").join("LIVE"), &link)?;
    assert!(
        installation.check(&link.join("x")).is_err(),
        "through a symlink"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn junction_into_the_installation_is_refused() -> TestResult {
    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let link = root.path().join("out-link");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(root.path().join("StarCitizen").join("LIVE"))
        .output()?
        .status;
    assert!(status.success(), "mklink /J");
    assert!(
        installation.check(&link.join("x")).is_err(),
        "through a junction"
    );
    Ok(())
}

#[test]
fn an_empty_installation_allows_everything() -> TestResult {
    let root = tempfile::tempdir()?;
    assert!(
        Installation::new().check(root.path()).is_ok(),
        "nothing protected"
    );
    Ok(())
}

#[test]
fn recognises_the_game_process_by_name_only() {
    assert!(is_game_process("StarCitizen.exe"));
    assert!(is_game_process("starcitizen.EXE"));
    assert!(!is_game_process("StarCitizenLauncher.exe"));
    assert!(!is_game_process("RSI Launcher.exe"));
    assert!(any_game_process(["explorer.exe", "StarCitizen.exe"]));
    assert!(!any_game_process(["explorer.exe"]));
}

#[test]
fn reads_the_game_from_qprocess_output() {
    let running = " BENUTZERNAME          SITZUNGSNAME        ID    PID  ABBILD\r\n>lucas                 console              1   4242  StarCitizen.exe\r\n";
    assert!(p4kx_guard::qprocess_lists_game(running));
    assert!(!p4kx_guard::qprocess_lists_game(
        "Es wurde kein Prozess gefunden.\r\n"
    ));
    assert!(!p4kx_guard::qprocess_lists_game(
        ">lucas console 1 13356 explorer.exe\r\n"
    ));
    assert!(!p4kx_guard::qprocess_lists_game(
        "No Process exists for starcitizen.exe\r\n"
    ));
    assert!(!p4kx_guard::qprocess_lists_game(
        "Für starcitizen.exe ist kein Prozess vorhanden\r\n"
    ));
}

#[test]
fn reads_the_game_from_the_powershell_count() {
    assert!(p4kx_guard::process_count_lists_game("1\r\n"));
    assert!(p4kx_guard::process_count_lists_game("2"));
    assert!(!p4kx_guard::process_count_lists_game("0\r\n"));
    assert!(!p4kx_guard::process_count_lists_game(""));
    assert!(!p4kx_guard::process_count_lists_game("error"));
}

#[cfg(windows)]
#[test]
fn a_loopback_share_does_not_hide_the_installation() -> TestResult {
    use std::path::{Component, Prefix};

    let root = tempfile::tempdir()?;
    let installation = installation_in(root.path())?;
    let canonical = fs::canonicalize(root.path())?;
    let mut components = canonical.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err("no drive prefix".into());
    };
    let (Prefix::VerbatimDisk(letter) | Prefix::Disk(letter)) = prefix.kind() else {
        return Ok(());
    };
    let rest: std::path::PathBuf = components
        .filter(|component| matches!(component, Component::Normal(_)))
        .collect();
    let share = format!(r"\\localhost\{}$", char::from(letter));
    if fs::metadata(&share).is_err() {
        return Ok(());
    }
    let target = std::path::PathBuf::from(&share)
        .join(&rest)
        .join("StarCitizen")
        .join("LIVE")
        .join("x");
    let verbatim = std::path::PathBuf::from(format!(r"\\?\UNC\localhost\{}$", char::from(letter)))
        .join(&rest)
        .join("StarCitizen")
        .join("LIVE")
        .join("x");
    assert!(installation.check(&target).is_err(), "loopback share");
    assert!(
        installation.check(&verbatim).is_err(),
        "verbatim loopback share"
    );
    let outside = std::path::PathBuf::from(&share).join(&rest).join("out");
    assert!(
        installation.check(&outside).is_ok(),
        "a sibling over the share"
    );
    Ok(())
}

#[test]
fn notices_a_running_game_from_the_process_list() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (source, name, args): (std::path::PathBuf, &str, &[&str]) = if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").ok_or("no SystemRoot")?;
        (
            std::path::PathBuf::from(root)
                .join("System32")
                .join("ping.exe"),
            "StarCitizen.exe",
            &["-n", "30", "127.0.0.1"],
        )
    } else {
        (
            std::path::PathBuf::from("/bin/sleep"),
            "StarCitizen.exe",
            &["30"],
        )
    };
    let copy = dir.path().join(name);
    fs::copy(source, &copy)?;
    let game = KillOnDrop(
        std::process::Command::new(&copy)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut seen = game_is_running();
    while !seen && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        seen = game_is_running();
    }
    drop(game);
    assert!(seen, "the fake game is in the process list");
    Ok(())
}

/// Kills and reaps the child when dropped, so a failing test leaves no process behind.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
