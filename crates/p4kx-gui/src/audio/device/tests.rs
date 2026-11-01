// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Duration, Instant};

use rodio::DeviceSinkBuilder;

use super::DeviceOutput;
use crate::audio::{Pcm, Player};

/// How long the output thread may take to report that it found no device.
const REPORT_DEADLINE: Duration = Duration::from_secs(10);

/// Returns whether the host has an output device; a host with one plays instead of failing.
fn has_device() -> bool {
    DeviceSinkBuilder::from_default_device()
        .and_then(|builder| builder.open_sink_or_fallback())
        .is_ok()
}

#[test]
fn a_host_without_an_output_device_reports_the_failure_and_does_not_panic() {
    if has_device() {
        return;
    }
    let mut player = Player::new(Pcm::new(vec![0; 4_800], 1, 48_000), 1.0);
    player.play(&DeviceOutput);

    let started = Instant::now();
    while !player.poll() {
        assert!(
            started.elapsed() < REPORT_DEADLINE,
            "the missing device was not reported"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        player.failure().is_some_and(|reason| !reason.is_empty()),
        "the failure carries a reason"
    );
    assert!(!player.is_playing(), "a failed output does not play");
}
