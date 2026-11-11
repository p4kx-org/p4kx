// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::panic::{self, AssertUnwindSafe};

use lewton::audio::{PreviousWindowRight, read_audio_packet_generic};
use lewton::samples::InterleavedSamples;

use super::{RebuiltSound, VorbisError};
use crate::{AudioError, WavReport, Wem};

const PANICKED: &str = "the decoder failed";

impl<B: AsRef<[u8]>> Wem<B> {
    /// Rebuilds a Wwise Vorbis sound, decodes it and writes it as a 16-bit PCM WAV to `out`.
    pub(crate) fn write_vorbis_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        let sound = self.rebuild_vorbis()?;
        let (header, report) = self.decoded_wav_header(sound.headers.sample_count)?;
        out.write_all(&header).map_err(AudioError::Write)?;
        sound.write_pcm(usize::from(self.channels()).max(1), out)?;
        Ok(report)
    }
}

impl RebuiltSound<'_> {
    /// Decodes the audio packets and writes the sample count's frames of interleaved
    /// little-endian 16-bit samples.
    fn write_pcm(&self, channels: usize, out: &mut impl Write) -> Result<(), AudioError> {
        let decoder = &self.headers.decoder;
        let mut audio = self.stream.audio(&self.headers.setup);
        let mut window = PreviousWindowRight::new();
        let mut packet = Vec::new();
        let mut bytes = Vec::new();
        let mut remaining = self.headers.sample_count;
        while remaining > 0
            && let Some(rebuilt) = audio.next_into(&mut packet)
        {
            let offset = rebuilt.map_err(AudioError::InvalidVorbis)?.offset as u64;
            let undecodable = |reason: String| {
                AudioError::InvalidVorbis(VorbisError::Undecodable { offset, reason })
            };
            let pcm: InterleavedSamples<i16> = panic::catch_unwind(AssertUnwindSafe(|| {
                read_audio_packet_generic(&decoder.ident, &decoder.setup, &packet, &mut window)
                    .map_err(|error| error.to_string())
            }))
            .unwrap_or_else(|_| Err(PANICKED.to_owned()))
            .map_err(undecodable)?;
            let available = pcm.samples.len().checked_div(channels).unwrap_or_default();
            let take = usize::try_from(remaining).map_or(available, |left| available.min(left));
            bytes.clear();
            for frame in pcm.samples.chunks_exact(channels).take(take) {
                for sample in frame {
                    bytes.extend_from_slice(&sample.to_le_bytes());
                }
            }
            out.write_all(&bytes).map_err(AudioError::Write)?;
            remaining -= take as u64;
        }
        if remaining > 0 {
            return Err(AudioError::InvalidVorbis(VorbisError::MissingSamples {
                expected: self.headers.sample_count,
                decoded: self.headers.sample_count - remaining,
            }));
        }
        Ok(())
    }
}
