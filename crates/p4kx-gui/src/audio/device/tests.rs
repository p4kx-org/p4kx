// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Duration, Instant};

use rodio::DeviceSinkError;

use super::Opened;
use crate::audio::{Pcm, Player};

/// How long the output thread may take to report its failure.
const REPORT_DEADLINE: Duration = Duration::from_secs(10);

fn player() -> Player {
    Player::new(Pcm::new(vec![0; 4_800], 1, 48_000), 1.0)
}

#[test]
fn a_missing_device_is_reported_to_the_player_and_does_not_panic() {
    let mut player = player();
    player.play(&Opened(|_| Err(DeviceSinkError::NoDevice)));

    let started = Instant::now();
    while !player.poll() {
        assert!(
            started.elapsed() < REPORT_DEADLINE,
            "the failure was not reported"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        player.failure().is_some_and(|reason| !reason.is_empty()),
        "the failure carries a reason"
    );
    assert!(!player.is_playing(), "a failed output does not play");
}

#[cfg(target_os = "linux")]
#[test]
fn the_default_output_survives_a_host_with_or_without_a_device() {
    let mut player = player();
    player.play(&super::DeviceOutput);
    std::thread::sleep(Duration::from_millis(200));
    player.poll();
}
