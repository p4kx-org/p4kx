// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::TryFromIntError;

use p4kx_testkit::{TestCodec, TestWem};

pub(super) fn rifx_pcm() -> Result<Vec<u8>, TryFromIntError> {
    Ok(TestWem::pcm(TestCodec::Pcm, 1, 16_000, &[0; 16])
        .big_endian()
        .smpl_loop(0, 0, 1)
        .chunk(*b"JUNK", [1, 2, 3])
        .chunk(*b"fmt ", pcm_format(0xfffe, 2, 32_000))
        .chunk(*b"seek", [0xff, 0xfe, 0xfd])
        .chunk(
            *b"smpl",
            sampler(&[(1, 1, 2), (0, 9, 10), (0, 2, 0), (0, 1, 4)])?,
        )
        .tail_chunk(*b"fmt ", pcm_format(1, 4, 96_000))
        .tail_chunk(*b"smpl", sampler(&[(0, 0, 4)])?)
        .tail_chunk(*b"data", [7; 64])
        .bytes())
}

pub(super) fn empty_vorb() -> Vec<u8> {
    TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 24_000)
        .chunk(*b"vorb", 12_000_u32.to_le_bytes())
        .chunk(*b"seek", [0xff, 0xfe])
        .chunk(*b"vorb", [])
        .data([1, 2, 3, 4])
        .tail_chunk(*b"vorb", 36_000_u32.to_le_bytes())
        .bytes()
}

pub(super) const SWITCH_FORMAT_TAG: u16 = 0x3039;

pub(super) fn switch_opus() -> Vec<u8> {
    TestWem::encoded(TestCodec::Tag(SWITCH_FORMAT_TAG), 1, 24_000, 1_200)
        .chunk(*b"seek", [0xff, 0xfe, 0xfd])
        .data([0xff, 0xfe, 0xfd, 0xfc])
        .bytes()
}

fn pcm_format(tag: u16, channels: u16, rate: u32) -> Vec<u8> {
    let align = channels * 2;
    let mut body = Vec::new();
    body.extend_from_slice(&tag.to_be_bytes());
    body.extend_from_slice(&channels.to_be_bytes());
    body.extend_from_slice(&rate.to_be_bytes());
    body.extend_from_slice(&(rate * u32::from(align)).to_be_bytes());
    body.extend_from_slice(&align.to_be_bytes());
    body.extend_from_slice(&16_u16.to_be_bytes());
    body
}

fn sampler(loops: &[(u32, u32, u32)]) -> Result<Vec<u8>, TryFromIntError> {
    let count = u32::try_from(loops.len())?;
    Ok([0_u32, 0, 0, 60, 0, 0, 0, count, 0]
        .into_iter()
        .chain(
            loops
                .iter()
                .flat_map(|&(kind, start, end)| [0, kind, start, end, 0, 0]),
        )
        .flat_map(u32::to_be_bytes)
        .collect())
}
