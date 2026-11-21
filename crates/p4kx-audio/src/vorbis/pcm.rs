// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::panic::{self, AssertUnwindSafe};

use lewton::audio::{PreviousWindowRight, read_audio_packet_generic};
use lewton::samples::InterleavedSamples;

use super::VorbisError;
use super::header::Header;
use super::stream::{AudioRead, AudioSource, Headers, StreamError};
use crate::{AudioError, WavReport, Wem};

const PANICKED: &str = "the decoder failed";

impl<B: AsRef<[u8]>> Wem<B> {
    /// Rebuilds a Wwise Vorbis sound, decodes it and writes it as a 16-bit PCM WAV to `out`.
    pub(crate) fn write_vorbis_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        let sound = self.rebuild_vorbis()?;
        let (header, report) = self.decoded_wav_header(sound.headers.sample_count)?;
        out.write_all(&header).map_err(AudioError::Write)?;
        sound
            .headers
            .write_pcm(&mut sound.stream.source(), &sound.stream.header, out)
            .map_err(StreamError::into_audio)?;
        Ok(report)
    }
}

impl Headers {
    /// Decodes the audio packets and writes the sample count's frames of interleaved
    /// little-endian 16-bit samples.
    pub(super) fn write_pcm<S: AudioSource>(
        &self,
        source: &mut S,
        header: &Header,
        out: &mut (impl Write + ?Sized),
    ) -> Result<usize, StreamError<S::InputError>> {
        let channels = usize::from(header.channels).max(1);
        let decoder = &self.decoder;
        let mut audio = source.audio(header, &self.setup);
        let mut window = PreviousWindowRight::new();
        let mut packet = Vec::new();
        let mut bytes = Vec::new();
        let mut remaining = self.sample_count;
        let mut decoded_end = header.audio_offset;
        while remaining > 0
            && let Some(rebuilt) = audio.next_packet(&mut packet)?
        {
            let offset = rebuilt.offset as u64;
            let undecodable = |reason| VorbisError::Undecodable { offset, reason };
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
            out.write_all(&bytes)
                .map_err(|error| StreamError::Audio(AudioError::Write(error)))?;
            remaining -= take as u64;
            decoded_end = rebuilt.end;
        }
        if remaining > 0 {
            return Err(VorbisError::MissingSamples {
                expected: self.sample_count,
                decoded: self.sample_count - remaining,
            }
            .into());
        }
        Ok(decoded_end)
    }
}
