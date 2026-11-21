// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::layout::Layout;
use super::ogg_read::{OggPackets, PageSource};
use super::packets::{Framing, PACKET_SIZE_LEN};
use super::{OPUS_RATE, OpusError};
use crate::loops::LOOP_COMMENTS;
use crate::{ByteOrder, Loop};

/// The format tag of Switch Opus.
pub(crate) const TAG_SWITCH: u16 = 0x3039;
const TAG_OGG: u16 = 0x3040;
const TAG_WWISE: u16 = 0x3041;
const WWISE_VERSION: u8 = 1;
const OPUS_HEAD_LEN: usize = 19;
const VENDOR: &[u8] = b"P4KX";

/// One original Opus declaration with source-specific packet storage.
pub(super) struct Header<'a> {
    pub(super) data: &'a [u8],
    pub(super) framing: Framing<'a>,
    pub(super) info: StreamHeader,
}

pub(super) struct StreamHeader {
    pub(super) layout: Layout,
    pub(super) pre_skip: Option<u16>,
    pub(super) declared: Option<u64>,
    pub(super) head: Option<Vec<u8>>,
    pub(super) tags: Option<Vec<u8>>,
    pub(super) output_gain: i16,
}

pub(super) enum Format {
    Table { bytes: u64, info: StreamHeader },
    Switch { prefix: u64, info: StreamHeader },
    Ogg { declared: Option<u64> },
}

const OGG_FMT_PREFIX: usize = 0x1C;
const WWISE_FMT_PREFIX: usize = 0x24;
pub(super) const SWITCH_FMT_LEN: usize = 0x28;

impl Format {
    pub(super) const fn prefix_length(tag: u16) -> usize {
        match tag {
            TAG_WWISE => WWISE_FMT_PREFIX,
            TAG_SWITCH => SWITCH_FMT_LEN,
            _ => OGG_FMT_PREFIX,
        }
    }

    pub(super) fn read(
        order: ByteOrder,
        fmt: &[u8],
        fmt_len: u64,
        seek_len: Option<u64>,
        data_len: u64,
        channels: u16,
    ) -> Result<Self, OpusError> {
        let fmt = Fmt { order, bytes: fmt };
        let tag = fmt.u16_at(0).unwrap_or_default();
        let unaligned = fmt.u16_at(0x0C) != Some(0) || fmt.u16_at(0x0E) != Some(0);
        if unaligned && tag != TAG_SWITCH {
            return Err(OpusError::InvalidHeader(
                "an Opus fmt chunk declares a block alignment or sample size",
            ));
        }
        match tag {
            TAG_WWISE => Self::wwise(fmt, seek_len, channels),
            TAG_SWITCH => Self::switch(fmt, fmt_len, data_len, channels),
            TAG_OGG => Ok(Self::Ogg {
                declared: fmt.u32_at(0x18).map(u64::from),
            }),
            _ => Err(OpusError::InvalidHeader("unknown Opus format tag")),
        }
    }

    fn wwise(fmt: Fmt<'_>, seek_len: Option<u64>, channels: u16) -> Result<Self, OpusError> {
        let byte = |offset: usize| fmt.bytes.get(offset).copied();
        let (Some(layout), Some(count), Some(pre_skip), Some(version), Some(mapping)) = (
            fmt.u32_at(0x14),
            fmt.u32_at(0x1C),
            fmt.u16_at(0x20),
            byte(0x22),
            byte(0x23),
        ) else {
            return Err(OpusError::InvalidHeader(
                "the Wwise Opus fmt chunk is shorter than 36 bytes",
            ));
        };
        if version != WWISE_VERSION {
            return Err(OpusError::InvalidHeader("unknown Wwise Opus version"));
        }
        let bytes = u64::from(count) * PACKET_SIZE_LEN as u64;
        if seek_len.is_none_or(|len| len < bytes) {
            return Err(OpusError::InvalidHeader(
                "the seek chunk holds fewer packet sizes than the fmt chunk counts",
            ));
        }
        Ok(Self::Table {
            bytes,
            info: StreamHeader::framed(
                Layout::wwise(channels, layout, mapping)?,
                Some(pre_skip),
                fmt.u32_at(0x18).map(u64::from),
            ),
        })
    }

    fn switch(fmt: Fmt<'_>, fmt_len: u64, data_len: u64, channels: u16) -> Result<Self, OpusError> {
        let prefix = fmt
            .u32_at(0x24)
            .filter(|_| fmt_len == SWITCH_FMT_LEN as u64)
            .map(u64::from)
            .ok_or(OpusError::InvalidHeader(
                "the Switch Opus fmt chunk is not 40 bytes",
            ))?;
        if prefix > data_len {
            return Err(OpusError::InvalidHeader(
                "the seek table is larger than the data",
            ));
        }
        let declared = fmt
            .u32_at(0x18)
            .filter(|_| fmt.u32_at(4) == Some(OPUS_RATE))
            .map(u64::from);
        Ok(Self::Switch {
            prefix,
            info: StreamHeader::framed(Layout::wwise(channels, 0, 0)?, None, declared),
        })
    }
}

/// The `fmt` chunk of an Opus WEM in its byte order.
#[derive(Clone, Copy)]
struct Fmt<'b> {
    order: ByteOrder,
    bytes: &'b [u8],
}

impl Fmt<'_> {
    fn u16_at(self, offset: usize) -> Option<u16> {
        self.order.u16_at(self.bytes, offset)
    }

    fn u32_at(self, offset: usize) -> Option<u32> {
        self.order.u32_at(self.bytes, offset)
    }
}

impl<'a> Header<'a> {
    pub(super) fn read(
        order: ByteOrder,
        fmt: &[u8],
        seek: Option<&'a [u8]>,
        data: &'a [u8],
        channels: u16,
    ) -> Result<Self, OpusError> {
        let config = Format::read(
            order,
            fmt,
            fmt.len() as u64,
            seek.map(|bytes| bytes.len() as u64),
            data.len() as u64,
            channels,
        )?;
        let (data, framing, info) = match config {
            Format::Table { bytes, info } => {
                let sizes = usize::try_from(bytes)
                    .ok()
                    .and_then(|len| seek?.get(..len))
                    .ok_or(OpusError::InvalidHeader(
                        "the seek chunk holds fewer packet sizes than the fmt chunk counts",
                    ))?;
                (data, Framing::Table(sizes), info)
            }
            Format::Switch { prefix, info } => {
                let packets = usize::try_from(prefix)
                    .ok()
                    .and_then(|prefix| data.get(prefix..))
                    .ok_or(OpusError::InvalidHeader(
                        "the seek table is larger than the data",
                    ))?;
                (packets, Framing::Switch, info)
            }
            Format::Ogg { declared } => {
                let info = StreamHeader::embedded(&mut OggPackets::new(data), channels, declared)?;
                (data, Framing::Ogg, info)
            }
        };
        info.check_channels(channels)?;
        Ok(Self {
            data,
            framing,
            info,
        })
    }
}

impl StreamHeader {
    const fn framed(layout: Layout, pre_skip: Option<u16>, declared: Option<u64>) -> Self {
        Self {
            layout,
            pre_skip,
            declared,
            head: None,
            tags: None,
            output_gain: 0,
        }
    }

    pub(super) fn embedded<S: PageSource>(
        packets: &mut OggPackets<S>,
        channels: u16,
        declared: Option<u64>,
    ) -> Result<Self, S::Error> {
        let head = packets
            .next_packet()?
            .ok_or(OpusError::InvalidOgg("the OpusHead packet is missing"))?
            .to_vec();
        let (pre_skip, layout) = read_head(&head, channels)?;
        let output_gain = head
            .get(16..18)
            .and_then(<[u8]>::first_chunk::<2>)
            .map_or(0, |gain| i16::from_le_bytes(*gain));
        let tags = packets
            .next_packet()?
            .ok_or(OpusError::InvalidOgg("the OpusTags packet is missing"))?
            .to_vec();
        if !tags.starts_with(b"OpusTags") {
            return Err(OpusError::InvalidOgg("the OpusTags packet is missing").into());
        }
        Ok(Self {
            layout,
            pre_skip: Some(pre_skip),
            declared,
            head: Some(head),
            tags: Some(tags),
            output_gain,
        })
    }

    pub(super) fn check_channels(&self, channels: u16) -> Result<(), OpusError> {
        if self.layout.mapping.len() != usize::from(channels) {
            return Err(OpusError::InvalidHeader(
                "the channel mapping does not cover the channels of the fmt chunk",
            ));
        }
        Ok(())
    }
}

fn read_head(head: &[u8], channels: u16) -> Result<(u16, Layout), OpusError> {
    let invalid = OpusError::InvalidHeader;
    if head.len() < OPUS_HEAD_LEN || !head.starts_with(b"OpusHead") {
        return Err(invalid("the OpusHead packet is malformed"));
    }
    let byte = |offset: usize| head.get(offset).copied().unwrap_or_default();
    if byte(8) & 0xF0 != 0 {
        return Err(invalid("the OpusHead version is not supported"));
    }
    if u16::from(byte(9)) != channels {
        return Err(invalid("the OpusHead channels differ from the fmt chunk"));
    }
    let pre_skip = u16::from_le_bytes([byte(10), byte(11)]);
    let family = byte(18);
    let layout = if family == 0 {
        let coupled = u8::from(channels == 2);
        Layout::new(0, 1, coupled, (0..=coupled).collect())?
    } else {
        let mapping = head
            .get(21..21 + usize::from(channels))
            .ok_or_else(|| invalid("the OpusHead channel mapping is truncated"))?;
        Layout::new(family, byte(19), byte(20), mapping.to_vec())?
    };
    Ok((pre_skip, layout))
}

/// Writes the `OpusHead` packet of a stream of `layout` that skips `pre_skip` samples.
pub(crate) fn head_packet(layout: &Layout, pre_skip: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(b"OpusHead");
    out.push(1);
    out.push(u8::try_from(layout.mapping.len()).unwrap_or(u8::MAX));
    out.extend_from_slice(&pre_skip.to_le_bytes());
    out.extend_from_slice(&OPUS_RATE.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.push(layout.family);
    if layout.family != 0 {
        out.push(layout.streams);
        out.push(layout.coupled);
        out.extend_from_slice(&layout.mapping);
    }
}

/// Writes an `OpusTags` packet that names the vendor and holds the comments of `points`.
pub(crate) fn tags_packet(points: Option<Loop>, out: &mut Vec<u8>) {
    out.extend_from_slice(b"OpusTags");
    out.extend_from_slice(
        &u32::try_from(VENDOR.len())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    out.extend_from_slice(VENDOR);
    match points {
        Some(points) => {
            out.extend_from_slice(&LOOP_COMMENTS.to_le_bytes());
            points.write_comments(out);
        }
        None => out.extend_from_slice(&0_u32.to_le_bytes()),
    }
}
