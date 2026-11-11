// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::time::Duration;

use crate::read_at::SliceSource;
use crate::riff::{Chunk, Chunks, RiffHeader, slice_range};
use crate::{AudioError, ByteOrder, Codec, Loop, WemInfoSource, WemMetadata};

const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// One RIFF chunk of a WEM: its tag and its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WemChunk<'a> {
    /// The four-character chunk tag, e.g. `*b"smpl"`.
    pub id: [u8; 4],
    /// The body after the 8-byte chunk header, without a pad byte.
    pub body: &'a [u8],
}

/// A Wwise WEM sound parsed from bytes the caller holds, e.g. a `&[u8]` or a `Vec<u8>`.
#[derive(Debug, Clone)]
pub struct Wem<B> {
    bytes: B,
    metadata: WemMetadata,
    data: Range<usize>,
}

impl<B: AsRef<[u8]>> Wem<B> {
    /// Parses the RIFF or RIFX structure and the `fmt` chunk of a WEM.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] when the bytes are not a WEM, are truncated, lack the `fmt` or
    /// `data` chunk, or declare a contradictory format.
    pub fn parse(bytes: B) -> Result<Self, AudioError> {
        let mut input = SliceSource(bytes.as_ref());
        let header = RiffHeader::read(&mut input)?;
        let metadata = WemMetadata::read(&mut input, header)?;
        let data = slice_range(metadata.data_chunk())?;
        Ok(Self {
            bytes,
            metadata,
            data,
        })
    }

    /// Returns every chunk of the RIFF in file order, those after `data` included, e.g. `smpl`,
    /// `cue ` and `LIST`; the walk ends at the declared RIFF size, or at the first chunk that runs
    /// past it.
    pub fn chunks(&self) -> impl Iterator<Item = WemChunk<'_>> {
        let bytes = self.bytes.as_ref();
        Chunks::of(bytes)
            .into_iter()
            .flat_map(|(_, chunks)| chunks)
            .map_while(Result::ok)
            .map(move |Chunk { id, body }| WemChunk {
                id,
                body: bytes.get(body).unwrap_or_default(),
            })
    }

    /// Returns the body of the first chunk tagged `id`, wherever it lies in the file.
    #[must_use]
    pub fn chunk(&self, id: [u8; 4]) -> Option<&[u8]> {
        self.chunks()
            .find(|chunk| chunk.id == id)
            .map(|chunk| chunk.body)
    }

    /// Returns the bytes of the `data` chunk, the encoded audio.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        self.bytes
            .as_ref()
            .get(self.data.clone())
            .unwrap_or_default()
    }

    /// Returns the number of frames, samples per channel, when the header states it or the codec
    /// allows computing it; for Switch Opus at a declared rate other than 48 kHz only the
    /// conversion counts them.
    #[must_use]
    pub const fn sample_count(&self) -> Option<u64> {
        self.metadata.sample_count()
    }

    /// Returns the loop of a looping sound: the first forward loop of its `smpl` chunk, when it
    /// is non-empty and ends within the known sample count. A raw end of 0 loops to the end of
    /// the sound. A `smpl` chunk after the `data` chunk is ignored.
    #[must_use]
    pub const fn loop_points(&self) -> Option<Loop> {
        self.metadata.loop_points()
    }

    /// Returns the playing time, when the sample count is known.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.metadata.duration()
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }

    /// Returns the bytes the WEM was parsed from.
    pub fn into_inner(self) -> B {
        self.bytes
    }
}

impl<B> Wem<B> {
    /// Returns the shared decoded WEM summary.
    #[must_use]
    pub const fn metadata(&self) -> &WemMetadata {
        &self.metadata
    }

    /// Returns the original decisions for this WEM's decoded summary.
    #[must_use]
    pub const fn info_source(&self) -> &WemInfoSource {
        self.metadata.info_source()
    }

    /// Returns the codec.
    #[must_use]
    pub const fn codec(&self) -> Codec {
        self.metadata.codec()
    }

    /// Returns the raw format tag of the `fmt` chunk.
    #[must_use]
    pub const fn format_tag(&self) -> u16 {
        self.metadata.format_tag()
    }

    /// Returns the byte order of the file.
    #[must_use]
    pub const fn byte_order(&self) -> ByteOrder {
        self.metadata.byte_order()
    }

    /// Returns the number of channels, at least 1.
    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.metadata.channels()
    }

    /// Returns the sample rate in Hz, at least 1; 48 000 for Opus, which always decodes at that
    /// rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.metadata.sample_rate()
    }

    /// Returns the bytes per block or frame the `fmt` chunk declares; 0 for variable-rate codecs.
    #[must_use]
    pub const fn block_align(&self) -> u16 {
        self.metadata.block_align()
    }

    /// Returns the speaker positions as a `WAVEFORMATEXTENSIBLE` channel mask, 0 when unknown.
    #[must_use]
    pub const fn channel_mask(&self) -> u32 {
        self.metadata.channel_mask()
    }
}

/// Returns the playing time of `frames` frames at `sample_rate` Hz; `None` for a rate of 0.
pub(crate) fn playing_time(frames: u64, sample_rate: u32) -> Option<Duration> {
    let rate = u64::from(sample_rate);
    let nanos = (frames.checked_rem(rate)?) * NANOS_PER_SECOND / rate;
    Some(Duration::new(frames / rate, u32::try_from(nanos).ok()?))
}
