// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sound::WemOriginField;

/// A value derived from the original WEM read.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum PayloadField {
    /// The media type selected from the retained prefix.
    Class,
    /// Whether the bank retains only the beginning of a WEM.
    Prefetch,
    /// Whether and why the reader could describe the WEM.
    Status,
    /// A field of the parsed WEM, or the reason it is absent.
    Metadata(MetadataField),
}

/// A field of the parsed WEM descriptor.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum MetadataField {
    /// Codec identity or its display name.
    Codec,
    /// Channel count.
    Channels,
    /// Effective playback sample rate.
    SampleRate,
    /// Frame count or its absence.
    SampleCount,
    /// Playback time or its absence.
    Duration,
    /// Loop state.
    Loop,
}

impl MetadataField {
    pub(crate) const ALL: [Self; 6] = [
        Self::Codec,
        Self::Channels,
        Self::SampleRate,
        Self::SampleCount,
        Self::Duration,
        Self::Loop,
    ];

    pub(crate) const fn source_field(self) -> WemOriginField {
        match self {
            Self::Codec => WemOriginField::Codec,
            Self::Channels => WemOriginField::Channels,
            Self::SampleRate => WemOriginField::SampleRate,
            Self::SampleCount => WemOriginField::SampleCount,
            Self::Duration => WemOriginField::Duration,
            Self::Loop => WemOriginField::Loop,
        }
    }
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Codec => 0,
            Self::Channels => 1,
            Self::SampleRate => 2,
            Self::SampleCount => 3,
            Self::Duration => 4,
            Self::Loop => 5,
        }
    }
}
