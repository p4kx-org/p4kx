// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

/// The process names of the game, compared ignoring case.
const GAME_PROCESSES: [&str; 2] = ["StarCitizen.exe", "StarCitizen"];

/// Returns whether name is the Star Citizen game process, ignoring case.
#[must_use]
pub fn is_game_process(name: &str) -> bool {
    GAME_PROCESSES
        .iter()
        .any(|game| name.eq_ignore_ascii_case(game))
}

/// Returns whether any of names is the game process.
pub fn any_game_process<I, S>(names: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    names.into_iter().any(|name| is_game_process(name.as_ref()))
}

/// Returns whether qprocess output contains a game process row.
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

/// Returns whether the PowerShell process count is a valid positive integer.
#[must_use]
pub fn process_count_lists_game(output: &str) -> bool {
    output.trim().parse::<u32>().is_ok_and(|count| count > 0)
}

/// Returns whether the process list identifies a Star Citizen game process.
///
/// Reads process image names through the platform process list.
///
/// # Errors
///
/// Returns the detection error when the process list cannot establish absence or presence.
pub fn game_process_status() -> io::Result<bool> {
    #[cfg(windows)]
    {
        crate::process_windows::game_process_status()
    }
    #[cfg(target_os = "linux")]
    {
        visible_process_status(
            crate::game_in_proc_tree(std::path::Path::new("/proc")),
            std::path::Path::new("/.flatpak-info").try_exists(),
        )
    }
    #[cfg(target_os = "macos")]
    {
        crate::process_macos::game_process_status()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Ok(false)
    }
}

/// Returns whether the game is running or its process status cannot be determined.
///
/// This conservative result drives a non-blocking warning, independently of the write guard.
#[must_use]
pub fn game_is_running() -> bool {
    warning_for(game_process_status())
}

fn warning_for(status: io::Result<bool>) -> bool {
    status.unwrap_or(true)
}

#[cfg(any(test, target_os = "linux"))]
fn visible_process_status(
    status: io::Result<bool>,
    flatpak_marker: io::Result<bool>,
) -> io::Result<bool> {
    match (status, flatpak_marker) {
        (Ok(false), Ok(true)) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Flatpak process isolation cannot establish host game absence",
        )),
        (Ok(false), Err(error)) => Err(error),
        (status, _) => status,
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{visible_process_status, warning_for};

    #[test]
    fn a_flatpak_negative_scan_cannot_establish_host_absence() {
        let result = visible_process_status(Ok(false), Ok(true));
        assert!(
            matches!(result, Err(ref error) if error.kind() == io::ErrorKind::Unsupported),
            "a readable sandbox process tree does not include host processes"
        );
    }

    #[test]
    fn an_observed_process_is_conclusive_inside_a_sandbox() -> io::Result<()> {
        assert!(
            visible_process_status(Ok(true), Ok(true))?,
            "observed presence remains conclusive in Flatpak"
        );
        assert!(
            visible_process_status(
                Ok(true),
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "marker denied"
                )),
            )?,
            "an observed process does not depend on marker readability"
        );
        assert!(
            !visible_process_status(Ok(false), Ok(false))?,
            "a complete host scan can establish absence"
        );
        Ok(())
    }

    #[test]
    fn an_unreadable_sandbox_marker_leaves_absence_unknown() {
        let result = visible_process_status(
            Ok(false),
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "marker denied",
            )),
        );
        assert!(
            matches!(result, Err(ref error) if error.kind() == io::ErrorKind::PermissionDenied),
            "marker failure cannot certify host process visibility"
        );
    }

    #[test]
    fn a_known_absence_is_the_only_result_without_a_warning() {
        assert!(
            !warning_for(Ok(false)),
            "known absence suppresses the warning"
        );
        assert!(
            warning_for(Ok(true)),
            "observed presence warrants a warning"
        );
        assert!(
            warning_for(Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "process list unavailable",
            ))),
            "unknown process status warrants a warning"
        );
    }
}
