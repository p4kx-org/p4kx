// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::transport::{SoundSource, Transport};
use super::{AudioOutput, Pcm, StreamGuard};

/// The playback of one decoded sound: play, pause, seek and volume, played through an
/// [`AudioOutput`] once it is started. Dropping it stops the sound and closes the output.
pub struct Player {
    pcm: Arc<Pcm>,
    transport: Arc<Transport>,
    stream: Option<StreamGuard>,
    failure: Option<String>,
}

impl fmt::Debug for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Player")
            .field("frames", &self.pcm.frames())
            .field("transport", &self.transport)
            .field("started", &self.stream.is_some())
            .field("failure", &self.failure)
            .finish()
    }
}

impl Player {
    /// Prepares the playback of `pcm` at `volume`, paused at the start.
    #[must_use]
    pub fn new(pcm: Pcm, volume: f32) -> Self {
        Self {
            pcm: Arc::new(pcm),
            transport: Arc::new(Transport::new(volume.clamp(0.0, 1.0))),
            stream: None,
            failure: None,
        }
    }

    /// Returns the decoded sound.
    #[must_use]
    pub fn pcm(&self) -> &Pcm {
        &self.pcm
    }

    /// Plays from the current position, or from the start once the end was reached; starts the
    /// output on first use and after it failed.
    pub fn play(&mut self, output: &dyn AudioOutput) {
        if self.failure.take().is_some() {
            self.stream = None;
        }
        if self.transport.position() >= self.pcm.frames() {
            self.transport.seek(0);
        }
        self.transport.set_playing(true);
        if self.stream.is_none() {
            let source = SoundSource::new(Arc::clone(&self.pcm), Arc::clone(&self.transport));
            self.stream = Some(output.start(source));
        }
    }

    /// Pauses at the current position.
    pub fn pause(&self) {
        self.transport.set_playing(false);
    }

    /// Returns whether the sound plays: started, not paused, failed or at its end.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.failure.is_none()
            && self.transport.is_playing()
            && self.transport.position() < self.pcm.frames()
    }

    /// Moves to `fraction` of the sound, clamped to `0.0..=1.0`.
    pub fn seek(&self, fraction: f32) {
        let frames = self.pcm.frames();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a position needs no more than f64 precision and the fraction is clamped"
        )]
        let frame = (f64::from(fraction.clamp(0.0, 1.0)) * frames as f64).round() as u64;
        self.transport.seek(frame.min(frames));
    }

    /// Returns the position as a fraction of the sound.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        let frames = self.pcm.frames();
        if frames == 0 {
            return 0.0;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            reason = "the fraction is drawn as a bar a few hundred points wide"
        )]
        let fraction = (self.transport.position().min(frames) as f64 / frames as f64) as f32;
        fraction
    }

    /// Returns the playing time at the current position.
    #[must_use]
    pub fn position(&self) -> Duration {
        self.pcm
            .time_of(self.transport.position().min(self.pcm.frames()))
    }

    /// Returns the playing time of the whole sound.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.pcm.duration()
    }

    /// Sets the volume, clamped to `0.0..=1.0`.
    pub fn set_volume(&self, volume: f32) {
        self.transport.set_volume(volume.clamp(0.0, 1.0));
    }

    /// Returns why the output failed, or `None` while it works or was not started.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Takes a failure the output reported and closes it; returns whether one arrived.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(reason) = self.transport.take_failure() else {
            return false;
        };
        self.transport.set_playing(false);
        self.stream = None;
        self.failure = Some(reason);
        true
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.transport.stop();
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "player_native_macos_tests.rs"]
mod native_macos_tests;
