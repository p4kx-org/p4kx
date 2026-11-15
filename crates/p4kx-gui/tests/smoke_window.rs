// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The smoke run of the real window: `p4kx-gui` starts, draws a few frames and exits.
//!
//! The test requires a display and a graphics driver. On macOS the application checks that its
//! actual window adapter uses Metal. Run with `--ignored`. The window uses an isolated profile.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The frames the run draws before the window closes itself.
const FRAMES: &str = "3";

/// How long the window may take to start, draw and exit.
const LIMIT: Duration = Duration::from_secs(120);

const POLL: Duration = Duration::from_millis(100);

/// Returns the built `p4kx-gui`: nextest sets the variable when it runs from an archive.
fn window_binary() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_p4kx-gui").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_p4kx-gui")),
        PathBuf::from,
    )
}

/// Builds the window launch with the profile folders below `profile`.
fn window_command(profile: &Path) -> Command {
    let mut window = Command::new(window_binary());
    window
        .env("P4KX_SMOKE_FRAMES", FRAMES)
        .env(p4kx_settings::FOLDERS_ROOT_VARIABLE, profile)
        .env("HOME", profile)
        .env("USERPROFILE", profile)
        .env("XDG_CONFIG_HOME", profile.join("config"))
        .env("XDG_CACHE_HOME", profile.join("cache"))
        .env("XDG_DATA_HOME", profile.join("data"))
        .env("APPDATA", profile.join("config"))
        .env("LOCALAPPDATA", profile.join("cache"))
        .env_remove("P4KX_KEY_FILE")
        .env_remove("P4KX_ARCHIVE_KEY");
    window
}

/// Runs the window until it exits or reaches the start and draw deadline.
fn run_window(mut command: Command) -> TestResult {
    let mut window = command.spawn()?;
    let deadline = Instant::now() + LIMIT;
    while Instant::now() < deadline {
        if let Some(status) = window.try_wait()? {
            assert!(
                status.success(),
                "the window ended unsuccessfully: {status}"
            );
            return Ok(());
        }
        sleep(POLL);
    }
    window.kill()?;
    window.wait()?;
    Err(format!("the window did not exit within {LIMIT:?}").into())
}

#[test]
#[ignore = "needs a display and a graphics driver: run with --ignored, under xvfb-run on a server"]
fn the_window_starts_draws_frames_and_exits() -> TestResult {
    let profile = tempfile::tempdir()?;

    run_window(window_command(profile.path()))
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "needs a private Weston compositor with output scale 2"]
fn the_wayland_window_draws_hidpi_frames_and_exits() -> TestResult {
    use std::fs::File;
    use std::process::Stdio;

    let runtime = std::env::var_os("P4KX_TEST_WAYLAND_RUNTIME_DIR")
        .ok_or("the Wayland smoke requires its private Weston runtime directory")?;
    let profile = tempfile::tempdir()?;
    let stdout = profile.path().join("window.stdout");
    let mut command = window_command(profile.path());
    command
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env("XDG_RUNTIME_DIR", runtime)
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("WGPU_BACKEND", "vulkan")
        .stdout(Stdio::from(File::create(&stdout)?));
    run_window(command)?;

    let output = std::fs::read_to_string(stdout)?;
    let scale = output
        .lines()
        .filter_map(|line| line.strip_prefix("P4KX_GUI_NATIVE_SCALE="))
        .last();
    assert_eq!(
        scale,
        Some("2"),
        "the native Wayland viewport must observe 2× scaling"
    );
    Ok(())
}
