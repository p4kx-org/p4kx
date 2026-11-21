// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::io::Write;

use super::header::{self, Header};
use super::packets::Rebuilt;
use super::setup::{self, Setup};
use super::validate::{self, Decoder, Fit};
use super::{Codebooks, OggReport, VorbisError};
use crate::{AudioError, Codec, Loop, OggWriter};

const SERIAL: u32 = 0x5034_4B58;
const ATTEMPTS: [Codebooks; 3] = [Codebooks::Default, Codebooks::AoTuV, Codebooks::Inline];

pub(super) enum StreamError<E> {
    Audio(AudioError),
    Input(E),
}

impl<E> From<VorbisError> for StreamError<E> {
    fn from(error: VorbisError) -> Self {
        Self::Audio(AudioError::InvalidVorbis(error))
    }
}

impl StreamError<Infallible> {
    pub(super) fn into_audio(self) -> AudioError {
        match self {
            Self::Audio(error) => error,
            Self::Input(never) => match never {},
        }
    }
}

pub(super) trait AudioRead<E> {
    fn next_packet(&mut self, out: &mut Vec<u8>) -> Result<Option<Rebuilt>, StreamError<E>>;
}

pub(super) trait AudioSource {
    type InputError;

    fn audio<'a>(
        &'a mut self,
        header: &'a Header,
        setup: &'a Setup,
    ) -> impl AudioRead<Self::InputError> + 'a;
}

pub(super) struct Headers {
    ident: [u8; 30],
    comment: Vec<u8>,
    loop_points: Option<Loop>,
    setup_header: Vec<u8>,
    pub(super) setup: Setup,
    pub(super) decoder: Decoder,
    codebooks: Codebooks,
    pub(super) sample_count: u64,
    pub(super) decoded_end: usize,
}

impl Headers {
    pub(super) fn rebuild<S: AudioSource>(
        source: &mut S,
        header: &Header,
        setup_packet: &[u8],
        loop_points: Option<Loop>,
    ) -> Result<Self, StreamError<S::InputError>> {
        let ident = header.ident_packet();
        let mut comment = Vec::new();
        header::comment_packet(None, &mut comment);
        let mut setup_header = Vec::new();
        let (codebooks, setup, decoder) = choose_codebooks(|codebooks| {
            let setup = setup::rebuild(
                setup_packet,
                u32::from(header.channels),
                codebooks,
                &mut setup_header,
            )?;
            let (fit, decoder) = validate::validate(
                &ident,
                &comment,
                &setup_header,
                source.audio(header, &setup),
            )?;
            Ok((setup, fit, decoder))
        })?;
        let extent = audio_extent(header, source.audio(header, &setup))?;
        let sample_count = extent.sample_count;
        let loop_points = loop_points.filter(|points| points.fits(sample_count));
        if loop_points.is_some() {
            header::comment_packet(loop_points, &mut comment);
        }
        Ok(Self {
            ident,
            comment,
            loop_points,
            setup_header,
            setup,
            decoder,
            codebooks,
            sample_count,
            decoded_end: extent.decoded_end,
        })
    }

    pub(super) fn write<S: AudioSource>(
        &self,
        source: &mut S,
        header: &Header,
        out: &mut (impl Write + ?Sized),
    ) -> Result<OggReport, StreamError<S::InputError>> {
        let mut ogg = OggWriter::new(out, SERIAL);
        let output = |error| StreamError::Audio(AudioError::Write(error));
        ogg.write_packet(&self.ident, 0).map_err(output)?;
        ogg.end_page().map_err(output)?;
        ogg.write_packet(&self.comment, 0).map_err(output)?;
        ogg.write_packet(&self.setup_header, 0).map_err(output)?;
        ogg.end_page().map_err(output)?;

        let mut audio = source.audio(header, &self.setup);
        let mut packet = Vec::new();
        let mut previous = 0;
        let mut granule = 0;
        let mut first = true;
        while let Some(rebuilt) = audio.next_packet(&mut packet)? {
            granule = trimmed(header, previous, rebuilt);
            ogg.write_packet(&packet, granule).map_err(output)?;
            if first {
                ogg.end_page().map_err(output)?;
                first = false;
            }
            previous = rebuilt.granule;
        }
        let size = ogg.finish().map_err(output)?;
        Ok(OggReport {
            codec: Codec::Vorbis,
            channels: u16::from(header.channels),
            sample_rate: header.sample_rate,
            sample_count: granule,
            size,
            codebooks: Some(self.codebooks),
            loop_points: self.loop_points,
        })
    }
}

fn choose_codebooks<E>(
    mut rebuild: impl FnMut(Codebooks) -> Result<(Setup, Fit, Decoder), StreamError<E>>,
) -> Result<(Codebooks, Setup, Decoder), StreamError<E>> {
    let mut clipped = None;
    let (mut default, mut aotuv, mut inline) = (None, None, None);
    for codebooks in ATTEMPTS {
        let reason = match codebooks {
            Codebooks::Default => &mut default,
            Codebooks::AoTuV => &mut aotuv,
            Codebooks::Inline => &mut inline,
        };
        match rebuild(codebooks) {
            Ok((setup, Fit::Clean, decoder)) => return Ok((codebooks, setup, decoder)),
            Ok((_, Fit::Clipped, _)) => {
                clipped.get_or_insert(codebooks);
            }
            Err(StreamError::Audio(AudioError::InvalidVorbis(error))) => *reason = Some(error),
            Err(error) => return Err(error),
        }
    }
    match (clipped, default, aotuv, inline) {
        (Some(codebooks), ..) => {
            let (setup, _, decoder) = rebuild(codebooks)?;
            Ok((codebooks, setup, decoder))
        }
        (None, Some(default), Some(aotuv), Some(inline)) => {
            Err(StreamError::Audio(AudioError::CodebooksRejected {
                default,
                aotuv,
                inline,
            }))
        }
        _ => Err(VorbisError::InvalidSetup("no codebooks were tried").into()),
    }
}

struct AudioExtent {
    sample_count: u64,
    decoded_end: usize,
}

fn audio_extent<E>(
    header: &Header,
    mut audio: impl AudioRead<E>,
) -> Result<AudioExtent, StreamError<E>> {
    let mut packet = Vec::new();
    let mut extent = AudioExtent {
        sample_count: 0,
        decoded_end: header.audio_offset,
    };
    while let Some(rebuilt) = audio.next_packet(&mut packet)? {
        let count = trimmed(header, extent.sample_count, rebuilt);
        if count > extent.sample_count {
            extent.decoded_end = rebuilt.end;
        }
        extent.sample_count = count;
    }
    Ok(extent)
}

fn trimmed(header: &Header, previous: u64, rebuilt: Rebuilt) -> u64 {
    let expected = u64::from(header.sample_count);
    if rebuilt.last && (previous..rebuilt.granule).contains(&expected) {
        expected
    } else {
        rebuilt.granule
    }
}
