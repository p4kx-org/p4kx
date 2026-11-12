// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::animation_keys::{EncodedValues, read_rotations, read_times, read_translations};
use crate::reader::to_usize;
use crate::{CorruptAnimation, Keys, Track};

pub(crate) const CHANNEL_LEN: usize = 12;
pub(crate) const CONTROLLER_LEN: usize = 2 * CHANNEL_LEN;

/// Decoded keys and the encoded byte intervals read to produce them.
#[derive(Debug)]
#[non_exhaustive]
pub struct DecodedAnimationChannel<const N: usize> {
    /// Original frame indices and decoded values.
    pub keys: Keys<[f32; N]>,
    /// The original frame-encoding interval, relative to the Entry start.
    pub frame_range: Range<u64>,
    /// The original value-encoding interval, relative to the Entry start.
    pub value_range: Range<u64>,
}

/// The stored encoding and offsets of one controller channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnimationChannel {
    /// The declared number of keys.
    pub key_count: u16,
    /// The original little-endian word, with time encoding in the low byte.
    pub packed_format: u16,
    /// The encoded time offset from the start of the controller entry.
    pub time_offset: u32,
    /// The encoded value offset from the start of the controller entry.
    pub value_offset: u32,
}

impl AnimationChannel {
    const fn of(bytes: &[u8; CHANNEL_LEN]) -> Self {
        let &[
            count_low,
            count_high,
            format_low,
            format_high,
            t0,
            t1,
            t2,
            t3,
            v0,
            v1,
            v2,
            v3,
        ] = bytes;
        Self {
            key_count: u16::from_le_bytes([count_low, count_high]),
            packed_format: u16::from_le_bytes([format_low, format_high]),
            time_offset: u32::from_le_bytes([t0, t1, t2, t3]),
            value_offset: u32::from_le_bytes([v0, v1, v2, v3]),
        }
    }

    fn decode<const N: usize>(
        self,
        data: &[u8],
        entry_at: usize,
        file_offset: u64,
        controller_id: u32,
        values: impl FnOnce(
            &[u8],
            usize,
            u8,
            usize,
        ) -> Result<EncodedValues<[f32; N]>, CorruptAnimation>,
    ) -> Result<Option<DecodedAnimationChannel<N>>, CorruptAnimation> {
        if self.key_count == 0 {
            return Ok(None);
        }
        let [times, format] = self.packed_format.to_le_bytes();
        let count = usize::from(self.key_count);
        let time_at = entry_at.saturating_add(to_usize(self.time_offset));
        let data_at = entry_at.saturating_add(to_usize(self.value_offset));
        let frames = read_times(data, time_at, times, count, controller_id)?;
        let values = values(data, data_at, format, count)?;
        let range = |at: usize, len: usize| {
            let start = file_offset + at as u64;
            start..start + len as u64
        };
        Ok(Some(DecodedAnimationChannel {
            frame_range: range(time_at, frames.bytes),
            value_range: range(data_at, values.bytes),
            keys: Keys {
                frames: frames.values,
                values: values.values,
            },
        }))
    }
}

/// A controller table entry and its physical position in the source file.
#[derive(Debug, Clone, Copy)]
pub struct AnimationController<'a> {
    ordinal: usize,
    id: u32,
    id_at: u64,
    entry_at: usize,
    file_offset: u64,
    rotation: AnimationChannel,
    translation: AnimationChannel,
    data: &'a [u8],
}

impl<'a> AnimationController<'a> {
    pub(crate) const fn of(
        data: &'a [u8],
        file_offset: u64,
        ordinal: usize,
        id_at: usize,
        id: u32,
        entry_at: usize,
        channels: &[[u8; CHANNEL_LEN]; 2],
    ) -> Self {
        let [rotation, translation] = channels;
        Self {
            ordinal,
            id,
            id_at: file_offset + id_at as u64,
            entry_at,
            file_offset,
            rotation: AnimationChannel::of(rotation),
            translation: AnimationChannel::of(translation),
            data,
        }
    }

    /// Returns the zero-based position in the controller block.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the original controller identifier.
    #[must_use]
    pub const fn controller_id(&self) -> u32 {
        self.id
    }

    /// Returns the byte interval of the controller identifier in the source file.
    #[must_use]
    pub const fn id_range(&self) -> Range<u64> {
        self.id_at..self.id_at + 4
    }

    /// Returns the byte interval of the two channel descriptors in the source file.
    #[must_use]
    pub const fn table_range(&self) -> Range<u64> {
        let start = self.file_offset + self.entry_at as u64;
        start..start + CONTROLLER_LEN as u64
    }

    /// Returns the original rotation channel descriptor, including empty channels.
    #[must_use]
    pub const fn rotation(&self) -> AnimationChannel {
        self.rotation
    }

    /// Returns the original translation channel descriptor, including empty channels.
    #[must_use]
    pub const fn translation(&self) -> AnimationChannel {
        self.translation
    }

    pub(crate) const fn declared_keys(&self) -> u32 {
        self.rotation.key_count as u32 + self.translation.key_count as u32
    }

    /// Decodes the rotation channel, including its original frame indices.
    ///
    /// # Errors
    ///
    /// Returns the rotation channel's format or data error.
    pub fn decode_rotation(&self) -> Result<Option<DecodedAnimationChannel<4>>, CorruptAnimation> {
        self.rotation.decode(
            self.data,
            self.entry_at,
            self.file_offset,
            self.id,
            |data, start, format, count| read_rotations(data, start, format, count, self.id),
        )
    }

    /// Decodes the translation channel, including its original frame indices.
    ///
    /// # Errors
    ///
    /// Returns the translation channel's format or data error.
    pub fn decode_translation(
        &self,
    ) -> Result<Option<DecodedAnimationChannel<3>>, CorruptAnimation> {
        self.translation.decode(
            self.data,
            self.entry_at,
            self.file_offset,
            self.id,
            read_translations,
        )
    }

    /// Decodes the controller's original frame indices and numeric keys.
    ///
    /// # Errors
    ///
    /// Returns the format or data error of a channel that cannot be decoded.
    pub fn decode(&self) -> Result<Option<Track>, CorruptAnimation> {
        let rotations = self.decode_rotation()?.map(|channel| channel.keys);
        let translations = self.decode_translation()?.map(|channel| channel.keys);
        Ok(
            (rotations.is_some() || translations.is_some()).then_some(Track {
                controller_id: self.id,
                rotations,
                translations,
            }),
        )
    }
}
