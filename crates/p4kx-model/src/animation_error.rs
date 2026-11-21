// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::Container;
use p4kx_foundation::ParseError;
use thiserror::Error;

/// A failure to read an animation file.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AnimationError {
    /// The file is in a container whose animation chunks are not interpreted.
    #[error("animations in the {container} container are not supported")]
    UnsupportedContainer {
        /// The animation file.
        file: String,
        /// Its container form.
        container: Container,
    },
    /// The file lacks a chunk every animation file of its kind has.
    #[error("holds no {chunk} chunk")]
    MissingChunk {
        /// The animation file.
        file: String,
        /// The chunk type's name.
        chunk: &'static str,
    },
    /// A chunk has a version whose layout this reader does not know.
    #[error("the {chunk} chunk has version {version:#x}, which is not supported")]
    UnsupportedVersion {
        /// The animation file.
        file: String,
        /// The chunk type's name.
        chunk: &'static str,
        /// The version found.
        version: u32,
    },
    /// A chunk is corrupt.
    #[error("the {chunk} chunk is corrupt")]
    Corrupt {
        /// The animation file.
        file: String,
        /// The chunk type's name.
        chunk: &'static str,
        /// What is wrong.
        #[source]
        source: CorruptAnimation,
    },
}

/// What is wrong with the animation data of a chunk.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CorruptAnimation {
    /// The data ends before a value it declares.
    #[error("it is truncated")]
    Truncated(#[from] ParseError),
    /// A block of controllers starts with another signature than its kind has.
    #[error("a block starts with {found:?} instead of {expected:?}")]
    Signature {
        /// The signature the chunk kind uses.
        expected: [u8; 4],
        /// The four bytes found.
        found: [u8; 4],
    },
    /// The frame rate is zero.
    #[error("the frame rate is zero")]
    FrameRate,
    /// A rotation, position or time format this reader does not know.
    #[error("the {kind} format {code:#04x} is not known")]
    UnknownFormat {
        /// Which kind of keys: `rotation`, `position` or `time`.
        kind: &'static str,
        /// The format code found.
        code: u8,
    },
    /// The key times of a track do not increase.
    #[error("the key times of controller {controller_id:#010x} do not increase")]
    TimesNotIncreasing {
        /// The controller the track belongs to.
        controller_id: u32,
    },
    /// The bitmap of a track's key times does not mark as many frames as the track has keys.
    #[error("the key bitmap of controller {controller_id:#010x} marks {found} of {expected} keys")]
    BitmapCount {
        /// The controller the track belongs to.
        controller_id: u32,
        /// The number of keys the track declares.
        expected: usize,
        /// The number of frames the bitmap marks.
        found: usize,
    },
    /// The last frame of a track's key bitmap is before its first.
    #[error("the key bitmap of controller {controller_id:#010x} runs from frame {first} to {last}")]
    BitmapRange {
        /// The controller the track belongs to.
        controller_id: u32,
        /// The first frame.
        first: u16,
        /// The last frame.
        last: u16,
    },
    /// A value is NaN or infinite.
    #[error("a {0} is not finite")]
    NonFinite(&'static str),
    /// A rotation has no length.
    #[error("a rotation of controller {controller_id:#010x} has no length")]
    ZeroRotation {
        /// The controller the track belongs to.
        controller_id: u32,
    },
    /// The animation declares more keys than the reader accepts.
    #[error("it declares {keys} keys, more than the {limit} the reader accepts")]
    TooManyKeys {
        /// The number of keys declared.
        keys: u64,
        /// The most the reader accepts.
        limit: u64,
    },
    /// The database declares a key total wider than the count type.
    #[error("the declared database key count does not fit in u64")]
    DeclaredKeyCountOverflow,
    /// The names of a database are fewer than its animations.
    #[error("the name table holds fewer than the {animations} animation names")]
    Names {
        /// The number of animations.
        animations: usize,
    },
}
