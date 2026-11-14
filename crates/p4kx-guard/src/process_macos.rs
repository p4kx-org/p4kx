// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::process::is_game_process;

#[cfg(target_os = "macos")]
pub(super) fn game_process_status() -> io::Result<bool> {
    use std::process::{Command, Stdio};

    // The Darwin comm column reads only argv[0].
    let output = Command::new("/bin/ps")
        .args(["-axww", "-o", "pid=", "-o", "comm="])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("macOS process listing failed"));
    }
    inspect(&output.stdout)
}

fn inspect(output: &[u8]) -> io::Result<bool> {
    let mut complete = true;
    let mut observed = false;
    for row in output.split(|byte| *byte == b'\n') {
        let row = row.trim_ascii();
        if row.is_empty() {
            continue;
        }
        let Some(command) = command_name(row) else {
            complete = false;
            continue;
        };
        observed = true;
        let kernel_name = command
            .strip_prefix(b"(")
            .and_then(|name| name.strip_suffix(b")"));
        let image = kernel_name.unwrap_or(command);
        let name = image
            .rsplit(|byte| matches!(byte, b'/' | b'\\'))
            .next()
            .and_then(|part| std::str::from_utf8(part).ok());
        if name.is_some_and(is_game_process) {
            return Ok(true);
        }
        if kernel_name.is_some() && name.is_some_and(is_wine_loader) {
            complete = false;
        }
    }
    if !observed || !complete {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "macOS process listing could not establish game absence",
        ));
    }
    Ok(false)
}

fn command_name(row: &[u8]) -> Option<&[u8]> {
    let boundary = row.iter().position(u8::is_ascii_whitespace)?;
    let (pid, command) = row.split_at_checked(boundary)?;
    let command = command.trim_ascii_start();
    (pid.iter().all(u8::is_ascii_digit) && !command.is_empty()).then_some(command)
}

fn is_wine_loader(name: &str) -> bool {
    [
        "wine",
        "wine64",
        "wine-preloader",
        "wine64-preloader",
        "wine64-preloade",
    ]
    .iter()
    .any(|loader| name.eq_ignore_ascii_case(loader))
}

#[cfg(test)]
mod tests;
