// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod bits;
mod codebook;
mod error;
mod header;
mod packets;
mod pcm;
mod seek;
mod seek_packets;
mod setup;
mod stream;
mod validate;

use std::convert::Infallible;
use std::io::Write;
use std::time::Duration;

pub use codebook::Codebooks;
pub use error::VorbisError;
pub use seek::{WemVorbisOgg, WemVorbisRanges, WemVorbisWav, WemVorbisWriteError};
pub(crate) use seek::{prepare_ogg, prepare_wav};

use self::header::Header;
use self::packets::{AudioPackets, Packets, Rebuilt};
use self::setup::Setup;
use self::stream::{AudioRead, AudioSource, Headers, StreamError};
use crate::riff::{Chunk, Chunks};
use crate::wem::playing_time;
use crate::{AudioError, ByteOrder, Codec, Loop, Wem};

pub(crate) const HEADER_IDENT: u8 = 1;
pub(crate) const HEADER_COMMENT: u8 = 3;
pub(crate) const HEADER_SETUP: u8 = 5;

/// What [`Wem::write_ogg`] wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OggReport {
    /// The codec of the stream: Wwise Vorbis becomes Ogg Vorbis, Opus becomes Ogg Opus.
    pub codec: Codec,
    /// Channels of the stream.
    pub channels: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Frames the stream plays, samples per channel: the granule position of its last page, less
    /// the pre-skip for Opus.
    pub sample_count: u64,
    /// Size of the whole Ogg file in bytes.
    pub size: u64,
    /// The codebooks the Vorbis setup header was rebuilt with; `None` for Opus.
    pub codebooks: Option<Codebooks>,
    /// The loop the `LOOPSTART` and `LOOPLENGTH` comments hold, `None` for a sound without a
    /// loop that fits the stream.
    pub loop_points: Option<Loop>,
}

impl OggReport {
    /// Returns the playing time of the stream.
    #[must_use]
    pub fn duration(&self) -> Duration {
        playing_time(self.sample_count, self.sample_rate).unwrap_or_default()
    }
}

/// The parts of a Wwise Vorbis WEM that rebuilding reads.
struct Stream<'a> {
    header: Header,
    order: ByteOrder,
    data: &'a [u8],
    setup: &'a [u8],
}

/// A Wwise Vorbis WEM with its Vorbis headers rebuilt and every audio packet checked.
pub(crate) struct RebuiltSound<'a> {
    stream: Stream<'a>,
    headers: Headers,
}

impl<B: AsRef<[u8]>> Wem<B> {
    /// Rebuilds a Wwise Vorbis sound as a standard Ogg Vorbis file, or repackages an Opus sound
    /// as a standard Ogg Opus file, and writes it to `out`.
    ///
    /// An Opus sound keeps its packets: they get an `OpusHead` with the pre-skip and the channel
    /// mapping of the WEM, an `OpusTags` packet and granule positions that count every packet,
    /// the last one trimmed to the pre-skip plus the sample count of the WEM.
    ///
    /// The setup header is rebuilt with the default codebook library first; when a Vorbis
    /// decoder rejects the result, the aoTuV library and then inline codebooks are tried. When
    /// every candidate that decodes gives mostly clipped samples, the first of them is taken.
    /// Granule positions count the samples of every packet, the first audio packet ends its own
    /// page at granule 0, and the last page is trimmed to the sample count of the WEM.
    ///
    /// The loop of a looping sound that fits the stream becomes the `LOOPSTART` and `LOOPLENGTH`
    /// comments, in frames after the Opus pre-skip; the sound is written once.
    ///
    /// # Errors
    ///
    /// Returns, before writing anything, [`AudioError::UnsupportedOggCodec`] for a codec other
    /// than Wwise Vorbis and Opus, [`AudioError::InvalidVorbis`] for a malformed header, packet
    /// layout or audio packet, [`AudioError::CodebooksRejected`] when no codebooks give a
    /// decodable stream and [`AudioError::InvalidOpus`] for a malformed Opus header or packet;
    /// [`AudioError::Write`] when `out` fails.
    pub fn write_ogg(&self, out: &mut impl Write) -> Result<OggReport, AudioError> {
        match self.codec() {
            Codec::Vorbis => self.write_vorbis_ogg(out),
            Codec::Opus => self.write_opus_ogg(out),
            codec => Err(AudioError::UnsupportedOggCodec(codec)),
        }
    }

    fn write_vorbis_ogg(&self, out: &mut impl Write) -> Result<OggReport, AudioError> {
        let sound = self.rebuild_vorbis()?;
        let stream = &sound.stream;
        sound
            .headers
            .write(&mut stream.source(), &stream.header, out)
            .map_err(StreamError::into_audio)
    }

    /// Rebuilds the Vorbis headers of a Wwise Vorbis WEM and checks every audio packet.
    pub(crate) fn rebuild_vorbis(&self) -> Result<RebuiltSound<'_>, AudioError> {
        let stream = self.vorbis_stream()?;
        let headers = Headers::rebuild(
            &mut stream.source(),
            &stream.header,
            stream.setup,
            self.loop_points(),
        )
        .map_err(StreamError::into_audio)?;
        Ok(RebuiltSound { stream, headers })
    }

    fn vorbis_stream(&self) -> Result<Stream<'_>, AudioError> {
        let bytes = self.bytes();
        let (order, chunks) = Chunks::of(bytes)?;
        let mut fmt = None;
        let mut vorb = None;
        for chunk in chunks {
            let Chunk { id, body } = chunk?;
            match &id {
                b"fmt " => fmt = bytes.get(body),
                b"vorb" => vorb = bytes.get(body),
                b"data" => break,
                _ => {}
            }
        }
        let fmt = fmt.ok_or(AudioError::MissingChunk("fmt "))?;
        let header = Header::read(order, fmt, vorb, self.channels(), self.sample_rate())
            .map_err(AudioError::InvalidVorbis)?;
        let data = self.data();
        let out_of_bounds = |offset: usize| {
            AudioError::InvalidVorbis(VorbisError::PacketOutOfBounds {
                offset: offset as u64,
            })
        };
        let setup = Packets::new(data, order, header.setup_offset)
            .next()
            .ok_or_else(|| out_of_bounds(header.setup_offset))?
            .map_err(AudioError::InvalidVorbis)?
            .payload;
        if header.audio_offset > data.len() {
            return Err(out_of_bounds(header.audio_offset));
        }
        for packet in Packets::new(data, order, header.audio_offset) {
            packet.map_err(AudioError::InvalidVorbis)?;
        }
        Ok(Stream {
            header,
            order,
            data,
            setup,
        })
    }
}

impl<'a> Stream<'a> {
    const fn source(&self) -> SliceAudio<'a> {
        SliceAudio {
            data: self.data,
            order: self.order,
        }
    }
}

struct SliceAudio<'a> {
    data: &'a [u8],
    order: ByteOrder,
}

impl AudioSource for SliceAudio<'_> {
    type InputError = Infallible;

    fn audio<'a>(
        &'a mut self,
        header: &'a Header,
        setup: &'a Setup,
    ) -> impl AudioRead<Self::InputError> + 'a {
        AudioPackets::new(self.data, self.order, header, &setup.long_modes)
    }
}

impl AudioRead<Infallible> for AudioPackets<'_> {
    fn next_packet(
        &mut self,
        out: &mut Vec<u8>,
    ) -> Result<Option<Rebuilt>, StreamError<Infallible>> {
        Ok(self.next_into(out).transpose()?)
    }
}
