// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::adpcm::Layout;
use crate::opus::{OPUS_RATE, TAG_SWITCH};
use crate::read_at::ReadAt;
use crate::riff::{RiffHeader, TailPolicy};
use crate::{
    AudioError, ByteOrder, Codec, Loop, WemInfoSource, WemMetadata, WemSampleCountSource,
    WemSourceChunk,
};

const SAMPLE_COUNT_OFFSET: usize = 0x18;
const VORBIS_FMT_LEN: u64 = 0x42;
const CHANNEL_LAYOUT_OFFSET: usize = 0x14;
const EXTRA_LEN_OFFSET: usize = 0x10;
const FMT_PREFIX_LEN: usize = SAMPLE_COUNT_OFFSET + 4;

struct Format {
    tag: u16,
    channels: u16,
    sample_rate: u32,
    block_align: u16,
    bits: u16,
    channel_layout: Option<u32>,
    channel_mask: u32,
    embedded_sample_count: Option<u32>,
    extra_len: Option<u16>,
    len: u64,
}

impl WemMetadata {
    pub(crate) fn read<I: ReadAt>(input: &mut I, header: RiffHeader) -> Result<Self, I::Error> {
        let SelectedChunks {
            format,
            vorbis,
            sample,
            data,
        } = selected_chunks(input, header)?;
        let order = header.order;
        let fields = format_fields(input, order, format)?;
        let (codec, codec_source) = Codec::classify_format(
            fields.tag,
            fields.channels,
            fields.block_align,
            fields.extra_len,
        );
        let vorbis_count = if codec == Codec::Vorbis {
            read_vorbis_count(input, order, vorbis)?
        } else {
            None
        };
        let (sample_count, sample_count_source) = sample_count(
            codec,
            &fields,
            vorbis_count,
            vorbis.map(|chunk| chunk.body_offset),
            data.body_len,
        )?;
        let (loop_points, loop_source) = Loop::from_smpl(order, input, sample, sample_count)?;
        let sample_rate = if codec == Codec::Opus {
            OPUS_RATE
        } else {
            fields.sample_rate
        };
        Ok(Self {
            order,
            format_tag: fields.tag,
            codec,
            channels: fields.channels,
            declared_sample_rate: fields.sample_rate,
            sample_rate,
            block_align: fields.block_align,
            bits_per_sample: fields.bits,
            channel_layout: fields.channel_layout,
            channel_mask: fields.channel_mask,
            sample_count,
            loop_points,
            data,
            info_source: WemInfoSource {
                format_body_offset: format.body_offset,
                data_body_offset: data.body_offset,
                codec: codec_source,
                sample_count: sample_count_source,
                loop_points: loop_source,
            },
        })
    }
}

pub(crate) struct SelectedChunks {
    pub(crate) format: WemSourceChunk,
    pub(crate) vorbis: Option<WemSourceChunk>,
    sample: Option<WemSourceChunk>,
    data: WemSourceChunk,
}

pub(crate) fn selected_chunks<I: ReadAt>(
    input: &mut I,
    header: RiffHeader,
) -> Result<SelectedChunks, I::Error> {
    let mut directory = header.directory(TailPolicy::Legacy);
    let mut format = None;
    let mut vorbis = None;
    let mut sample = None;
    let mut data = None;
    while let Some(chunk) = directory.next(input).transpose()? {
        match &chunk.id {
            b"fmt " => format = Some(chunk),
            b"vorb" => vorbis = Some(chunk),
            b"smpl" => sample = Some(chunk),
            b"data" => {
                data = Some(chunk);
                break;
            }
            _ => {}
        }
    }
    Ok(SelectedChunks {
        format: format.ok_or(AudioError::MissingChunk("fmt "))?,
        vorbis,
        sample,
        data: data.ok_or(AudioError::MissingChunk("data"))?,
    })
}

fn format_fields<I: ReadAt>(
    input: &mut I,
    order: ByteOrder,
    chunk: WemSourceChunk,
) -> Result<Format, I::Error> {
    let mut prefix = [0; FMT_PREFIX_LEN];
    let count = usize::try_from(chunk.body_len.min(FMT_PREFIX_LEN as u64))
        .map_err(|_| AudioError::InvalidFormat("the fmt prefix does not fit in memory"))?;
    let bytes = prefix
        .get_mut(..count)
        .ok_or(AudioError::InvalidFormat("the fmt prefix is out of bounds"))?;
    input.read_at(chunk.body_offset, bytes)?;
    Ok(read_format(order, bytes, chunk.body_len)?)
}

fn read_vorbis_count<I: ReadAt>(
    input: &mut I,
    order: ByteOrder,
    chunk: Option<WemSourceChunk>,
) -> Result<Option<(u64, u32)>, I::Error> {
    let Some(chunk) = chunk.filter(|chunk| chunk.body_len >= 4) else {
        return Ok(None);
    };
    let mut count = [0; 4];
    input.read_at(chunk.body_offset, &mut count)?;
    Ok(Some((chunk.body_offset, order.u32(count))))
}

fn read_format(order: ByteOrder, fmt: &[u8], len: u64) -> Result<Format, AudioError> {
    let field16 = |offset| order.u16_at(fmt, offset);
    let field32 = |offset| order.u32_at(fmt, offset);
    let (Some(tag), Some(channels), Some(sample_rate), Some(block_align), Some(bits)) = (
        field16(0),
        field16(2),
        field32(4),
        field16(0x0C),
        field16(0x0E),
    ) else {
        return Err(AudioError::InvalidFormat(
            "the fmt chunk is shorter than 16 bytes",
        ));
    };
    if channels == 0 {
        return Err(AudioError::InvalidFormat("the WEM has no channels"));
    }
    if sample_rate == 0 {
        return Err(AudioError::InvalidFormat("the sample rate is 0"));
    }
    let channel_layout = field32(CHANNEL_LAYOUT_OFFSET);
    let channel_mask = channel_layout.map_or(0, |layout| {
        let is_wwise_layout = layout & 0xFF == u32::from(channels) && (layout >> 8) & 0x0F == 1;
        if is_wwise_layout {
            layout >> 12
        } else {
            layout
        }
    });
    Ok(Format {
        tag,
        channels,
        sample_rate,
        block_align,
        bits,
        channel_layout,
        channel_mask,
        embedded_sample_count: field32(SAMPLE_COUNT_OFFSET),
        extra_len: field16(EXTRA_LEN_OFFSET),
        len,
    })
}

fn sample_count(
    codec: Codec,
    format: &Format,
    vorbis_count: Option<(u64, u32)>,
    vorbis_body_offset: Option<u64>,
    data_len: u64,
) -> Result<(Option<u64>, WemSampleCountSource), AudioError> {
    let channels = u64::from(format.channels);
    let block_align = u64::from(format.block_align);
    Ok(match codec {
        Codec::Pcm => {
            if format.bits != 16 {
                return Err(AudioError::InvalidFormat(
                    "PCM WEMs must have 16 bits per sample",
                ));
            }
            if block_align != channels * 2 {
                return Err(AudioError::InvalidFormat(
                    "the PCM block alignment does not match the channels",
                ));
            }
            (Some(data_len / block_align), WemSampleCountSource::Pcm)
        }
        Codec::ImaAdpcm | Codec::PtAdpcm => {
            let layout = Layout::of(codec, format.channels, format.block_align);
            let supported_layout = layout.is_some();
            (
                layout.map(|layout| layout.frames(data_len)),
                WemSampleCountSource::Adpcm { supported_layout },
            )
        }
        Codec::Vorbis => {
            if let Some((body_offset, count)) = vorbis_count {
                (
                    Some(u64::from(count)),
                    WemSampleCountSource::VorbisChunk { body_offset },
                )
            } else {
                let count = format
                    .embedded_sample_count
                    .filter(|_| format.len == VORBIS_FMT_LEN);
                (
                    count.map(u64::from),
                    WemSampleCountSource::VorbisFormat {
                        rejected_vorbis_body: vorbis_body_offset,
                        accepted: count.is_some(),
                    },
                )
            }
        }
        Codec::Opus => {
            let count_present = format.embedded_sample_count.is_some();
            let rate_checked = count_present && format.tag == TAG_SWITCH;
            let count = format
                .embedded_sample_count
                .filter(|_| format.tag != TAG_SWITCH || format.sample_rate == OPUS_RATE);
            (
                count.map(u64::from),
                WemSampleCountSource::Opus {
                    rate_checked,
                    accepted: count.is_some(),
                },
            )
        }
        _ => (None, WemSampleCountSource::Unsupported),
    })
}
