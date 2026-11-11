// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use crate::adpcm::Layout;
use crate::loops::SMPL_CHUNK_LEN;
use crate::{AudioError, ByteOrder, Codec, Loop, Wem, WemMetadata};

const BITS_PER_SAMPLE: u16 = 16;
const FORMAT_PCM: u16 = 0x0001;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const PCM_FMT_LEN: u32 = 16;
const EXTENSIBLE_FMT_LEN: u32 = 40;
const EXTENSION_LEN: u16 = 22;
const SUBTYPE_PCM: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
pub(crate) const PCM_TRANSFER_BYTES: usize = 64 * 1024;

/// What [`Wem::write_wav`] wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavReport {
    /// Channels of the WAV.
    pub channels: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Frames written, samples per channel.
    pub sample_count: u64,
    /// Size of the whole WAV file in bytes.
    pub size: u64,
    /// The loop the `smpl` chunk holds, `None` for a sound without a loop that fits the frames.
    pub loop_points: Option<Loop>,
}

impl<B: AsRef<[u8]>> Wem<B> {
    /// Writes the sound as a 16-bit PCM WAV file to `out`: a canonical `WAVE_FORMAT_PCM` header
    /// for one or two channels, `WAVE_FORMAT_EXTENSIBLE` with the channel mask for more. PCM is
    /// copied, IMA ADPCM and PTADPCM are decoded. A trailing partial frame or block is dropped.
    /// The loop of a looping sound that fits the written frames goes into a `smpl` chunk before
    /// the samples, which are written once.
    /// Wwise Vorbis is rebuilt as for [`Wem::write_ogg`] and decoded to the same sample count,
    /// the channels in the order of the WEM. Opus is decoded at 48 kHz, its pre-skip dropped and
    /// its streams mapped to the speaker order of the WEM's channel mask.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::UnsupportedCodec`] before writing anything when the codec has no
    /// decoder, [`AudioError::InvalidFormat`] when the ADPCM block alignment does not fit the
    /// channels, [`AudioError::WavTooLarge`] when the WAV would exceed 4 GiB, and
    /// [`AudioError::Write`] when `out` fails. For Wwise Vorbis it returns the errors of
    /// [`Wem::write_ogg`] before writing anything, and [`AudioError::InvalidVorbis`] with
    /// [`VorbisError::Undecodable`](crate::VorbisError::Undecodable) when the decoder fails on a
    /// packet or [`VorbisError::MissingSamples`](crate::VorbisError::MissingSamples) when it
    /// gives fewer frames than the stream counts, after part of the WAV is written. For Opus it
    /// returns [`AudioError::InvalidOpus`] for a malformed header or packet before writing
    /// anything, and for a packet the decoder fails on after part of the WAV is written.
    pub fn write_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        match self.codec() {
            Codec::Pcm => self.write_pcm_wav(out),
            Codec::ImaAdpcm | Codec::PtAdpcm => self.write_adpcm_wav(out),
            Codec::Vorbis => self.write_vorbis_wav(out),
            Codec::Opus => self.write_opus_wav(out),
            codec => Err(AudioError::UnsupportedCodec(codec)),
        }
    }

    fn write_adpcm_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        let layout = Layout::of(self.codec(), self.channels(), self.block_align()).ok_or(
            AudioError::InvalidFormat("the ADPCM block alignment does not fit the channels"),
        )?;
        let (header, report) = self.decoded_wav_header(layout.frames(self.data().len() as u64))?;
        out.write_all(&header).map_err(AudioError::Write)?;
        layout
            .write_pcm(self.data(), self.byte_order(), &mut Vec::new(), out)
            .map_err(AudioError::Write)?;
        Ok(report)
    }

    pub(crate) fn decoded_wav_header(
        &self,
        frames: u64,
    ) -> Result<(Vec<u8>, WavReport), AudioError> {
        self.metadata().decoded_wav_header(frames)
    }

    fn write_pcm_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        let channels = self.channels();
        let block_align = wav_block_align(channels)?;
        let frames = self.data().len() / usize::from(block_align);
        let data = self
            .data()
            .get(..frames * usize::from(block_align))
            .unwrap_or_default();
        let (header, loop_points) = self.metadata().wav_header(block_align, data.len())?;
        out.write_all(&header).map_err(AudioError::Write)?;
        match self.byte_order() {
            ByteOrder::Little => out.write_all(data).map_err(AudioError::Write)?,
            ByteOrder::Big => write_swapped(data, out)?,
        }
        Ok(WavReport {
            channels,
            sample_rate: self.sample_rate(),
            sample_count: frames as u64,
            size: (header.len() + data.len()) as u64,
            loop_points,
        })
    }
}

impl WemMetadata {
    /// Returns the WAV header and the report for `frames` decoded 16-bit frames.
    pub(crate) fn decoded_wav_header(
        &self,
        frames: u64,
    ) -> Result<(Vec<u8>, WavReport), AudioError> {
        let channels = self.channels();
        let block_align = wav_block_align(channels)?;
        let data_len = frames
            .checked_mul(u64::from(block_align))
            .and_then(|len| usize::try_from(len).ok())
            .ok_or(AudioError::WavTooLarge)?;
        let (header, loop_points) = self.wav_header(block_align, data_len)?;
        let size = header
            .len()
            .checked_add(data_len)
            .ok_or(AudioError::WavTooLarge)? as u64;
        let report = WavReport {
            channels,
            sample_rate: self.sample_rate(),
            sample_count: frames,
            size,
            loop_points,
        };
        Ok((header, report))
    }

    fn wav_header(
        &self,
        block_align: u16,
        data_len: usize,
    ) -> Result<(Vec<u8>, Option<Loop>), AudioError> {
        let channels = self.channels();
        let extensible = channels > 2;
        let fmt_len = if extensible {
            EXTENSIBLE_FMT_LEN
        } else {
            PCM_FMT_LEN
        };
        let frames = data_len
            .checked_div(usize::from(block_align))
            .unwrap_or_default() as u64;
        let smpl = self
            .loop_points()
            .filter(|points| points.fits(frames))
            .and_then(|points| Some((points, points.smpl_chunk(self.sample_rate())?)));
        let smpl_len = u32::try_from(smpl.as_ref().map_or(0, |(_, chunk)| chunk.len()))
            .map_err(|_| AudioError::WavTooLarge)?;
        let data_len = u32::try_from(data_len).map_err(|_| AudioError::WavTooLarge)?;
        let riff_len = (4 + 8 + 8 + fmt_len + smpl_len)
            .checked_add(data_len)
            .ok_or(AudioError::WavTooLarge)?;
        let byte_rate = self
            .sample_rate()
            .checked_mul(u32::from(block_align))
            .ok_or(AudioError::InvalidFormat("the byte rate exceeds 32 bits"))?;
        let mut header = Vec::with_capacity(68 + SMPL_CHUNK_LEN);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&riff_len.to_le_bytes());
        header.extend_from_slice(b"WAVEfmt ");
        header.extend_from_slice(&fmt_len.to_le_bytes());
        let tag = if extensible {
            FORMAT_EXTENSIBLE
        } else {
            FORMAT_PCM
        };
        header.extend_from_slice(&tag.to_le_bytes());
        header.extend_from_slice(&channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate().to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
        if extensible {
            let mask = self.channel_mask();
            let mask = if mask.count_ones() == u32::from(channels) {
                mask
            } else {
                0
            };
            header.extend_from_slice(&EXTENSION_LEN.to_le_bytes());
            header.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
            header.extend_from_slice(&mask.to_le_bytes());
            header.extend_from_slice(&SUBTYPE_PCM);
        }
        if let Some((_, chunk)) = &smpl {
            header.extend_from_slice(chunk);
        }
        header.extend_from_slice(b"data");
        header.extend_from_slice(&data_len.to_le_bytes());
        Ok((header, smpl.map(|(points, _)| points)))
    }
}

fn write_swapped(data: &[u8], out: &mut impl Write) -> Result<(), AudioError> {
    let mut buffer = vec![0; PCM_TRANSFER_BYTES.min(data.len())];
    for chunk in data.chunks(PCM_TRANSFER_BYTES) {
        let target = buffer
            .get_mut(..chunk.len())
            .ok_or(AudioError::InvalidFormat(
                "the PCM output buffer is out of bounds",
            ))?;
        target.copy_from_slice(chunk);
        swap_pcm_samples(target);
        out.write_all(target).map_err(AudioError::Write)?;
    }
    Ok(())
}

pub(crate) fn swap_pcm_samples(bytes: &mut [u8]) {
    for [first, second] in bytes.as_chunks_mut::<2>().0 {
        std::mem::swap(first, second);
    }
}

fn wav_block_align(channels: u16) -> Result<u16, AudioError> {
    channels
        .checked_mul(BITS_PER_SAMPLE / 8)
        .ok_or(AudioError::InvalidFormat("too many channels"))
}
