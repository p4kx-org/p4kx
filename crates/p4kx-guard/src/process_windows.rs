// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::process::qprocess_lists_game;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
const COUNT_SCRIPT: &str = "$ErrorActionPreference = 'Stop'; @(Get-Process | Where-Object { $_.ProcessName -eq 'StarCitizen' }).Count";

struct ProbeOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[cfg(windows)]
impl From<std::process::Output> for ProbeOutput {
    fn from(output: std::process::Output) -> Self {
        Self {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

#[cfg(windows)]
pub(super) fn game_process_status() -> io::Result<bool> {
    use std::path::PathBuf;
    use std::process::Command;

    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let system = root.join("System32");
    let mut qprocess = Command::new(system.join("qprocess.exe"));
    qprocess.arg("StarCitizen.exe");
    check(run(&mut qprocess), || {
        let mut powershell = Command::new(
            system
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
        );
        powershell.args(["-NoProfile", "-NonInteractive", "-Command", COUNT_SCRIPT]);
        run(&mut powershell)
    })
}

#[cfg(windows)]
fn run(command: &mut std::process::Command) -> io::Result<ProbeOutput> {
    use std::os::windows::process::CommandExt;

    command
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(ProbeOutput::from)
}

fn check(
    qprocess: io::Result<ProbeOutput>,
    powershell: impl FnOnce() -> io::Result<ProbeOutput>,
) -> io::Result<bool> {
    let first_error = match qprocess {
        Ok(output) if output.success => {
            return Ok(qprocess_lists_game(&String::from_utf8_lossy(
                &output.stdout,
            )));
        }
        Ok(output) => command_failure("qprocess", &output),
        Err(error) => error,
    };
    let result = match powershell() {
        Ok(output) if output.success => parse_count(&output.stdout),
        Ok(output) => Err(command_failure("PowerShell", &output)),
        Err(error) => Err(error),
    };
    result.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("qprocess: {first_error}; PowerShell: {error}"),
        )
    })
}

fn parse_count(output: &[u8]) -> io::Result<bool> {
    let text = std::str::from_utf8(output)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    text.trim()
        .parse::<u32>()
        .map(|count| count > 0)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn command_failure(program: &str, output: &ProbeOutput) -> io::Error {
    io::Error::other(format!(
        "{program} exited unsuccessfully: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
mod tests;
