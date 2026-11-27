// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Streams the blocks of an IMA ADPCM or PTADPCM WEM through their decoders into 16-bit PCM.

use std::io::{self, Write};

use rayon::prelude::*;

use crate::{ByteOrder, Codec, ima, ptadpcm};

const BATCH_BYTES: usize = 1 << 20;
const MIN_BLOCKS_PER_TASK: usize = 16;

/// The ADPCM codecs with a decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ima,
    PtAdpcm,
}

impl Kind {
    const fn header_len(self) -> usize {
        match self {
            Self::Ima => ima::HEADER_LEN,
            Self::PtAdpcm => ptadpcm::HEADER_LEN,
        }
    }

    const fn samples(self, sub_block: usize) -> usize {
        match self {
            Self::Ima => ima::samples_per_sub_block(sub_block),
            Self::PtAdpcm => ptadpcm::samples_per_frame(sub_block),
        }
    }
}

/// The block layout of an ADPCM WEM: every block holds one contiguous sub-block per channel.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    kind: Kind,
    channels: usize,
    sub_block: usize,
}

impl Layout {
    /// Returns the layout of an ADPCM codec, or `None` when the codec is not ADPCM or the block
    /// alignment does not split into sub-blocks with at least one sample after the header.
    pub(crate) fn of(codec: Codec, channels: u16, block_align: u16) -> Option<Self> {
        let kind = match codec {
            Codec::ImaAdpcm => Kind::Ima,
            Codec::PtAdpcm => Kind::PtAdpcm,
            _ => return None,
        };
        let channels = usize::from(channels);
        let block_align = usize::from(block_align);
        let sub_block = block_align.checked_div(channels)?;
        (block_align % channels == 0 && sub_block > kind.header_len()).then_some(Self {
            kind,
            channels,
            sub_block,
        })
    }

    /// Returns the frames, samples per channel, that `data_len` bytes decode to.
    pub(crate) fn frames(self, data_len: u64) -> u64 {
        let block = self.block_len() as u64;
        let rest = usize::try_from(data_len % block).unwrap_or(0);
        let partial = self
            .partial_sub_block(rest)
            .map_or(0, |len| self.kind.samples(len));
        data_len / block * self.kind.samples(self.sub_block) as u64 + partial as u64
    }

    /// Returns the source bytes in one complete ADPCM block.
    pub(crate) const fn block_len(self) -> usize {
        self.sub_block * self.channels
    }

    /// Returns the consumed complete-block and admitted partial-block bytes.
    pub(crate) fn consumed_bytes(self, data_len: u64) -> u64 {
        let remainder = data_len % self.block_len() as u64;
        let admitted_tail = self
            .partial_sub_block(usize::try_from(remainder).unwrap_or_default())
            .map_or(0, |per_channel| per_channel * self.channels);
        data_len - remainder + admitted_tail as u64
    }

    /// Decodes the supplied blocks using a reusable PCM buffer.
    pub(crate) fn write_pcm(
        self,
        data: &[u8],
        order: ByteOrder,
        pcm: &mut Vec<u8>,
        out: &mut (impl Write + ?Sized),
    ) -> io::Result<()> {
        let block = self.block_len();
        let block_pcm = self.kind.samples(self.sub_block) * self.channels * 2;
        let (blocks, rest) = data
            .split_at_checked(data.len() - data.len() % block)
            .unwrap_or((data, &[]));
        let batch_blocks = (BATCH_BYTES / block_pcm).max(1);
        for batch in blocks.chunks(block * batch_blocks) {
            pcm.resize(batch.len() / block * block_pcm, 0);
            pcm.par_chunks_mut(block_pcm)
                .zip(batch.par_chunks(block))
                .with_min_len(MIN_BLOCKS_PER_TASK)
                .for_each(|(pcm, block)| self.decode_block(block, self.sub_block, order, pcm));
            out.write_all(pcm)?;
        }
        if let Some(sub_block) = self.partial_sub_block(rest.len()) {
            pcm.resize(self.kind.samples(sub_block) * self.channels * 2, 0);
            self.decode_block(rest, sub_block, order, pcm);
            out.write_all(pcm)?;
        }
        Ok(())
    }

    fn decode_block(self, block: &[u8], sub_block: usize, order: ByteOrder, pcm: &mut [u8]) {
        let samples = self.kind.samples(sub_block);
        let (slots, _) = pcm.as_chunks_mut::<2>();
        for (channel, sub) in block
            .chunks_exact(sub_block)
            .take(self.channels)
            .enumerate()
        {
            let out = slots
                .iter_mut()
                .skip(channel)
                .step_by(self.channels)
                .take(samples);
            match self.kind {
                Kind::Ima => ima::decode(sub, order, out),
                Kind::PtAdpcm => ptadpcm::decode(sub, out),
            }
        }
    }

    fn partial_sub_block(self, rest: usize) -> Option<usize> {
        let sub_block = rest / self.channels;
        (self.kind == Kind::Ima && sub_block > ima::HEADER_LEN).then_some(sub_block)
    }
}

/// Returns the nibbles after a header of `header_len` bytes, the low nibble of each byte first.
pub(crate) fn nibbles(sub_block: &[u8], header_len: usize) -> impl Iterator<Item = u8> {
    sub_block
        .get(header_len..)
        .unwrap_or_default()
        .iter()
        .flat_map(|&byte| [byte & 0x0F, byte >> 4])
}

/// Saturates `sample` to 16 bits.
pub(crate) fn clamp16(sample: i32) -> i32 {
    sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX))
}

/// Returns a sample already within 16 bits as little-endian bytes.
pub(crate) fn sample_bytes(sample: i32) -> [u8; 2] {
    i16::try_from(clamp16(sample))
        .unwrap_or_default()
        .to_le_bytes()
}
