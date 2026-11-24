// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};

use super::{
    ANIM_INFO_VERSION, AnimationChunk, CAF_DATA_VERSION, CAF_SIGNATURE, MAX_ANIMATION_KEYS,
    PositionCapture, RECORD_LEN, build, check_budget, corrupt, declared_keys, decode_tracks,
    read_block, read_record, stem,
};
use crate::{Animation, AnimationClipCoordinates, AnimationError, PositionedAnimation};

impl Animation {
    /// Reads the animation of the `.caf` file `file` named `file_name`: its `AnimInfo` chunk
    /// and its `CafData` chunk with one block of controllers. The animation is named after the
    /// file without folder and extension.
    ///
    /// # Errors
    ///
    /// Returns [`AnimationError::UnsupportedContainer`] for a `CrCh` file,
    /// [`AnimationError::MissingChunk`] and [`AnimationError::UnsupportedVersion`] when a chunk is
    /// absent or has a layout this reader does not know, and [`AnimationError::Corrupt`] when
    /// the data is truncated, malformed or in a key format this reader does not know.
    pub fn read_caf<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        Self::read_caf_projected(
            file_name,
            file,
            PositionCapture::Off,
            |animation, _, _, _| animation,
        )
    }

    /// Reads a CAF and retains each successful track's original controller ordinal.
    ///
    /// # Errors
    /// As [`Self::read_caf`].
    pub fn read_caf_with_positions<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<PositionedAnimation, AnimationError> {
        Self::read_caf_projected(
            file_name,
            file,
            PositionCapture::Tracks,
            PositionedAnimation::new,
        )
    }

    /// Reads a CAF and retains its clip metadata positions.
    ///
    /// # Errors
    /// As [`Self::read_caf`].
    pub fn read_caf_with_clip_positions<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<PositionedAnimation, AnimationError> {
        Self::read_caf_projected(
            file_name,
            file,
            PositionCapture::Clip,
            PositionedAnimation::new,
        )
    }

    fn read_caf_projected<B: AsRef<[u8]>, T>(
        file_name: &str,
        file: &ChunkFile<B>,
        capture: PositionCapture,
        project: impl FnOnce(Self, usize, Vec<usize>, Option<AnimationClipCoordinates>) -> T,
    ) -> Result<T, AnimationError> {
        let info = AnimationChunk::read(
            file_name,
            file,
            ivo::ANIM_INFO,
            ANIM_INFO_VERSION,
            "AnimInfo",
        )?;
        let record = read_record(info.bytes.get(..RECORD_LEN).unwrap_or(info.bytes))
            .map_err(|source| corrupt(file_name, "AnimInfo", source))?;
        let data =
            AnimationChunk::read(file_name, file, ivo::CAF_DATA, CAF_DATA_VERSION, "CafData")?;
        let block = read_block(data.bytes, 0, CAF_SIGNATURE)
            .map_err(|source| corrupt(file_name, "CafData", source))?;
        let mut positions = capture.track_positions();
        let tracks = declared_keys(data.bytes, &block)
            .and_then(|keys| {
                check_budget(keys, MAX_ANIMATION_KEYS)?;
                decode_tracks(data, &block, &mut positions)
            })
            .map_err(|source| corrupt(file_name, "CafData", source))?;
        let animation = build(
            stem(file_name).to_owned(),
            file_name.to_owned(),
            record,
            tracks,
        );
        let (ordinals, coordinates) = if capture.includes_metadata() {
            let chunks = AnimationChunk::observe(file, info, data);
            let (ordinals, coordinates) =
                positions
                    .unwrap_or_default()
                    .project(chunks, info.offset, None, data, &block);
            (ordinals, Some(coordinates))
        } else {
            (Vec::new(), None)
        };
        Ok(project(animation, 0, ordinals, coordinates))
    }
}
