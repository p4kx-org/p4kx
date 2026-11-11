// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use p4kx_testkit::test_signal;

use super::Player;
use crate::audio::{DeviceOutput, Pcm};

type TestResult = Result<(), Box<dyn Error>>;

const RATE: u32 = 48_000;
const SAMPLE_SECONDS: u32 = 8;
const PROGRESS: Duration = Duration::from_millis(50);
const POLL: Duration = Duration::from_millis(10);
const SETTLE: Duration = Duration::from_millis(80);
const DEADLINE: Duration = Duration::from_secs(8);

#[test]
#[ignore = "requires an Apple Silicon Mac with a usable CoreAudio output device"]
fn coreaudio_consumes_samples_after_start_and_seek_and_stops_when_closed() -> TestResult {
    let pcm = Pcm::new(test_signal(RATE * SAMPLE_SECONDS, 1), 1, RATE);
    let mut player = Player::new(pcm, 0.04);
    let output = DeviceOutput;
    player.play(&output);
    wait_for_position(&mut player, PROGRESS)?;

    player.pause();
    sleep(SETTLE);
    let paused = player.position();
    sleep(SETTLE);
    assert_eq!(
        player.position(),
        paused,
        "paused CoreAudio stream holds the source position"
    );

    player.seek(0.5);
    let destination = Duration::from_secs(u64::from(SAMPLE_SECONDS / 2));
    assert_eq!(
        player.position(),
        destination,
        "seek was delivered to the shared transport"
    );
    player.play(&output);
    wait_for_position(&mut player, destination + PROGRESS)?;

    let transport = Arc::clone(&player.transport);
    drop(player);
    sleep(SETTLE);
    let stopped = transport.position();
    sleep(SETTLE);
    assert_eq!(
        transport.position(),
        stopped,
        "closed device no longer consumes the original stream"
    );
    Ok(())
}

fn wait_for_position(player: &mut Player, minimum: Duration) -> TestResult {
    let start = Instant::now();
    while start.elapsed() < DEADLINE {
        if player.poll() {
            return Err(format!(
                "CoreAudio playback failed: {}",
                player.failure().unwrap_or("unspecified device error")
            )
            .into());
        }
        if player.position() >= minimum {
            return Ok(());
        }
        sleep(POLL);
    }
    Err("CoreAudio did not consume the expected source frames".into())
}
