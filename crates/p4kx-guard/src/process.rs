// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The process names of the game, compared ignoring case.
const GAME_PROCESSES: [&str; 2] = ["StarCitizen.exe", "StarCitizen"];

/// Returns whether `name` is the name of the Star Citizen game process, ignoring case.
#[must_use]
pub fn is_game_process(name: &str) -> bool {
    GAME_PROCESSES
        .iter()
        .any(|game| name.eq_ignore_ascii_case(game))
}

/// Returns whether any of `names` is the game process.
pub fn any_game_process<I, S>(names: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    names.into_iter().any(|name| is_game_process(name.as_ref()))
}

/// Returns whether the output of `qprocess` lists the game process.
///
/// A process line ends in the process id and the image name; the header and the message for no
/// match, which in English also ends in the image name, have no process id before it.
#[must_use]
pub fn qprocess_lists_game(output: &str) -> bool {
    output.lines().any(|line| {
        let mut columns = line.split_whitespace().rev();
        matches!(
            (columns.next(), columns.next()),
            (Some(image), Some(id))
                if is_game_process(image) && id.bytes().all(|byte| byte.is_ascii_digit())
        )
    })
}

/// Returns whether the count that the PowerShell fallback prints is above zero.
#[must_use]
pub fn process_count_lists_game(output: &str) -> bool {
    output.trim().parse::<u32>().is_ok_and(|count| count > 0)
}

/// Returns whether `StarCitizen.exe` of any Channel is running.
///
/// Only process names are read: `qprocess` on Windows (a PowerShell count of the matching
/// processes where `qprocess` is missing), `comm` and the first `cmdline` argument of each process
/// below `/proc` elsewhere, which finds the game under Wine and Proton (see
/// [`game_in_proc_tree`](crate::game_in_proc_tree)). No process is opened, attached to or
/// inspected, so the game's memory and handles are never touched. The check takes a few hundred
/// milliseconds on Windows.
///
/// When the process list cannot be read, Windows answers `false` and Linux answers `true`. The
/// result only drives a non-blocking warning; the write guard does not depend on it. Other
/// platforms without `/proc` answer `false`.
#[must_use]
pub fn game_is_running() -> bool {
    platform::game_is_running()
}

#[cfg(windows)]
mod platform {
    use std::io::ErrorKind;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const COUNT_SCRIPT: &str = "@(Get-Process -Name StarCitizen -ErrorAction Ignore).Count";

    /// Asks `qprocess`; Windows Home has none, so it falls back to a PowerShell script that
    /// prints only the number of matching processes.
    pub(super) fn game_is_running() -> bool {
        let root = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let system = root.join("System32");
        let qprocess = Command::new(system.join("qprocess.exe"))
            .arg("StarCitizen.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match qprocess {
            Ok(output) => {
                output.status.success()
                    && super::qprocess_lists_game(&String::from_utf8_lossy(&output.stdout))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Command::new(
                system
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            )
            .args(["-NoProfile", "-NonInteractive", "-Command", COUNT_SCRIPT])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && super::process_count_lists_game(&String::from_utf8_lossy(&output.stdout))
            }),
            Err(_) => false,
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    /// Scans `/proc`; where it cannot be listed the answer is unknown, which on Linux counts as
    /// "the game may be running".
    pub(super) fn game_is_running() -> bool {
        crate::game_in_proc_tree(Path::new("/proc")).unwrap_or(cfg!(target_os = "linux"))
    }
}
