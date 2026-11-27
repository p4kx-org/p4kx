// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Display;
use std::panic::{self, AssertUnwindSafe};

use lewton::audio::{PreviousWindowRight, read_audio_packet_generic};
use lewton::header::{
    IdentHeader, SetupHeader, read_header_comment, read_header_ident, read_header_setup,
};

use super::VorbisError;
use super::stream::{AudioRead, StreamError};

const CHECKED_PACKETS: usize = 16;
const CLIPPED_SHARE: usize = 10;

/// How well rebuilt headers fit the audio packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fit {
    /// The packets decode to plausible samples.
    Clean,
    /// The packets decode, but more than a tenth of the samples are clipped, which garbage from
    /// codebooks of another library usually is.
    Clipped,
}

/// The rebuilt headers as the decoder parsed them.
pub(crate) struct Decoder {
    pub(crate) ident: IdentHeader,
    pub(crate) setup: SetupHeader,
}

/// Decodes the rebuilt headers and the first audio packets with a standard Vorbis decoder and
/// returns the parsed headers for decoding the rest.
pub(super) fn validate<E>(
    ident: &[u8],
    comment: &[u8],
    setup: &[u8],
    audio: impl AudioRead<E>,
) -> Result<(Fit, Decoder), StreamError<E>> {
    panic::catch_unwind(AssertUnwindSafe(|| decode(ident, comment, setup, audio)))
        .unwrap_or_else(|_| Err(VorbisError::Rejected("the decoder failed".to_owned()).into()))
}

fn rejected(error: &dyn Display) -> VorbisError {
    VorbisError::Rejected(error.to_string())
}

fn decode<E>(
    ident: &[u8],
    comment: &[u8],
    setup: &[u8],
    mut audio: impl AudioRead<E>,
) -> Result<(Fit, Decoder), StreamError<E>> {
    let ident = read_header_ident(ident).map_err(|error| rejected(&error))?;
    read_header_comment(comment).map_err(|error| rejected(&error))?;
    let blocksizes = (ident.blocksize_0, ident.blocksize_1);
    let setup = read_header_setup(setup, ident.audio_channels, blocksizes)
        .map_err(|error| rejected(&error))?;
    let mut window = PreviousWindowRight::new();
    let mut packet = Vec::new();
    let (mut samples, mut clipped) = (0, 0);
    for _ in 0..CHECKED_PACKETS {
        if audio.next_packet(&mut packet)?.is_none() {
            break;
        }
        let channels: Vec<Vec<f32>> =
            read_audio_packet_generic(&ident, &setup, &packet, &mut window)
                .map_err(|error| rejected(&error))?;
        for &sample in channels.iter().flatten() {
            if !sample.is_finite() {
                return Err(
                    VorbisError::Rejected("the decoder returns no number".to_owned()).into(),
                );
            }
            samples += 1;
            if sample.abs() >= 1.0 {
                clipped += 1;
            }
        }
    }
    let fit = if clipped * CLIPPED_SHARE > samples {
        Fit::Clipped
    } else {
        Fit::Clean
    };
    Ok((fit, Decoder { ident, setup }))
}
