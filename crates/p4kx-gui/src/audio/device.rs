// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZero;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use rodio::{
    ChannelCount, DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, SampleRate, Source,
};

use super::{AudioOutput, FailureReporter, SoundSource, StreamGuard};
use crate::model::describe;

/// The default audio device of the system, opened through rodio on a thread of its own for
/// every sound played; rodio converts the sound to the device's rate and channels.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceOutput;

impl AudioOutput for DeviceOutput {
    fn start(&self, source: SoundSource) -> StreamGuard {
        let (guard, closed) = mpsc::channel::<()>();
        let reporter = source.reporter();
        let thread_reporter = reporter.clone();
        let spawned = thread::Builder::new()
            .name("audio-output".to_owned())
            .spawn(move || play(source, &thread_reporter, &closed));
        if let Err(error) = spawned {
            reporter.report(describe(&error));
        }
        StreamGuard::new(guard)
    }
}

/// Opens the default output device; stream errors after the opening go to `reporter`. Fails
/// with [`DeviceSinkError::NoDevice`] when the system has none, as on a host without a sound
/// card.
fn open_default(reporter: &FailureReporter) -> Result<MixerDeviceSink, DeviceSinkError> {
    let runtime = reporter.clone();
    DeviceSinkBuilder::from_default_device().and_then(|builder| {
        builder
            .with_error_callback(move |error| runtime.report(describe(&error)))
            .open_sink_or_fallback()
    })
}

fn play(source: SoundSource, reporter: &FailureReporter, closed: &Receiver<()>) {
    let mut sink = match open_default(reporter) {
        Ok(sink) => sink,
        Err(error) => {
            reporter.report(describe(&error));
            return;
        }
    };
    sink.log_on_drop(false);
    sink.mixer().add(Feed(source));
    let _closed = closed.recv();
}

#[cfg(test)]
mod tests;

/// A [`SoundSource`] as a rodio source.
struct Feed(SoundSource);

impl Iterator for Feed {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_sample()
    }
}

impl Source for Feed {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(self.0.channels()).unwrap_or(NonZero::<u16>::MIN)
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.0.sample_rate()).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
