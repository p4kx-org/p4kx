// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The game is found in a fake `/proc` tree the way Wine and Proton present it.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_guard::game_in_proc_tree;

type TestResult = Result<(), Box<dyn Error>>;

fn process(root: &Path, id: &str, comm: &str, cmdline: &[u8]) -> TestResult {
    let folder = root.join(id);
    fs::create_dir_all(&folder)?;
    fs::write(folder.join("comm"), format!("{comm}\n"))?;
    fs::write(folder.join("cmdline"), cmdline)?;
    Ok(())
}

#[test]
fn finds_the_game_by_its_comm() -> TestResult {
    let root = tempfile::tempdir()?;
    process(root.path(), "100", "bash", b"bash\0")?;
    process(root.path(), "4242", "StarCitizen.exe", b"")?;
    assert!(game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn finds_the_game_by_its_windows_image_path_when_comm_is_the_loader() -> TestResult {
    let root = tempfile::tempdir()?;
    process(
        root.path(),
        "4242",
        "wine64-preloader",
        b"Z:\\home\\lucas\\Games\\StarCitizen\\LIVE\\Bin64\\StarCitizen.exe\0-no_login_dialog\0",
    )?;
    assert!(game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn finds_the_game_by_a_unix_image_path_in_any_letter_case() -> TestResult {
    let root = tempfile::tempdir()?;
    process(
        root.path(),
        "7",
        "wine",
        b"/prefix/drive_c/StarCitizen/LIVE/Bin64/starcitizen.EXE\0",
    )?;
    assert!(game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn ignores_other_programs_and_lookalikes() -> TestResult {
    let root = tempfile::tempdir()?;
    process(
        root.path(),
        "1",
        "RSI Launcher.ex",
        b"C:\\Program Files\\Roberts Space Industries\\RSI Launcher\\RSI Launcher.exe\0",
    )?;
    process(
        root.path(),
        "2",
        "StarCitizenLaun",
        b"Z:\\x\\StarCitizenLauncher.exe\0",
    )?;
    process(root.path(), "3", "less", b"less\0StarCitizen.exe\0")?;
    process(
        root.path(),
        "4",
        "vim",
        b"vim\0/home/u/StarCitizen.exe.log\0",
    )?;
    assert!(!game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn ignores_folders_that_are_not_process_ids() -> TestResult {
    let root = tempfile::tempdir()?;
    process(root.path(), "self", "StarCitizen.exe", b"StarCitizen.exe\0")?;
    process(root.path(), "thread-self", "StarCitizen.exe", b"")?;
    process(root.path(), "12a", "StarCitizen.exe", b"")?;
    fs::write(root.path().join("99"), "StarCitizen.exe")?;
    assert!(!game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn skips_a_process_that_ended_during_the_scan() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("5"))?;
    assert!(!game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn reads_only_a_bounded_prefix_of_the_command_line() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut cmdline = vec![b'x'; 1 << 20];
    cmdline.extend_from_slice(b"/StarCitizen.exe\0");
    process(root.path(), "9", "tool", &cmdline)?;
    assert!(!game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn an_empty_process_list_has_no_game() -> TestResult {
    let root = tempfile::tempdir()?;
    assert!(!game_in_proc_tree(root.path())?);
    Ok(())
}

#[test]
fn an_unlistable_root_is_an_error_not_an_answer() -> TestResult {
    let root = tempfile::tempdir()?;
    assert!(game_in_proc_tree(&root.path().join("missing")).is_err());
    Ok(())
}
