// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The smoke run of the real window: `p4kx-gui` starts, draws a few frames and exits.
//!
//! The test needs a display (a desktop session, or `xvfb-run` on a server) and a Vulkan or GL
//! driver (lavapipe on a machine without a GPU), so it runs only when asked for with
//! `--ignored`. The window gets an empty profile and no network access to the update check.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
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

/// Runs the window with the profile folders below `profile` and returns how it ended, or `None`
/// when it was still running after [`LIMIT`] and has been killed.
fn run_window(profile: &Path) -> std::io::Result<Option<ExitStatus>> {
    let mut window = Command::new(window_binary())
        .env("P4KX_SMOKE_FRAMES", FRAMES)
        .env("HOME", profile)
        .env("USERPROFILE", profile)
        .env("XDG_CONFIG_HOME", profile.join("config"))
        .env("XDG_CACHE_HOME", profile.join("cache"))
        .env("XDG_DATA_HOME", profile.join("data"))
        .env("APPDATA", profile.join("config"))
        .env("LOCALAPPDATA", profile.join("cache"))
        .env_remove("P4KX_KEY_FILE")
        .env_remove("P4KX_ARCHIVE_KEY")
        .spawn()?;
    let deadline = Instant::now() + LIMIT;
    while Instant::now() < deadline {
        if let Some(status) = window.try_wait()? {
            return Ok(Some(status));
        }
        sleep(POLL);
    }
    window.kill()?;
    window.wait()?;
    Ok(None)
}

#[test]
#[ignore = "needs a display and a graphics driver: run with --ignored, under xvfb-run on a server"]
fn the_window_starts_draws_frames_and_exits() -> TestResult {
    let profile = tempfile::tempdir()?;

    let status = run_window(profile.path())?;

    assert!(
        status.is_some_and(|status| status.success()),
        "the window ended successfully within {LIMIT:?}: {status:?}"
    );
    Ok(())
}
