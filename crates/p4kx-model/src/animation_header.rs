// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};

use crate::animation::bind_controllers;
use crate::animation_read::{
    ANIM_INFO_VERSION, CAF_DATA_VERSION, CAF_SIGNATURE, MAX_ANIMATION_KEYS, RECORD_LEN,
    check_budget, chunk_bytes, corrupt, declared_keys, listed_tracks, read_block, read_record,
    stem,
};
use crate::{Animation, AnimationError, AnimationTiming, Bone};

/// What the headers of an animation file say about one animation, read without decoding a key.
///
/// The key counts are the ones the controllers declare; a key format the reader does not know
/// shows only when [`crate::Animation::read_caf`] or [`crate::Dba::read`] decodes the keys.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnimationHeader {
    /// The animation's name: the file name or path without folder and extension.
    pub name: String,
    /// The file name or path the animation is stored under.
    pub path: String,
    /// The number of frames per second.
    pub frames_per_second: u16,
    /// The last frame of the animation.
    pub end_frame: u32,
    /// Whether the keys are offsets to be added to another animation instead of poses.
    pub additive: bool,
    /// The controller IDs of the tracks that have keys, in the order of the file.
    pub controller_ids: Vec<u32>,
    /// The number of keys of all tracks.
    pub keys: usize,
}

impl AnimationHeader {
    pub(crate) const fn of(name: String, path: String, record: AnimationTiming) -> Self {
        Self {
            name,
            path,
            frames_per_second: record.frames_per_second,
            end_frame: record.end_frame,
            additive: record.additive,
            controller_ids: Vec::new(),
            keys: 0,
        }
    }

    /// Describes the decoded `animation`.
    #[must_use]
    pub fn of_animation(animation: &Animation) -> Self {
        Self {
            name: animation.name.clone(),
            path: animation.path.clone(),
            frames_per_second: animation.frames_per_second,
            end_frame: animation.end_frame,
            additive: animation.additive,
            controller_ids: animation
                .tracks
                .iter()
                .map(|track| track.controller_id)
                .collect(),
            keys: animation.key_count(),
        }
    }

    /// Returns the length of the animation in seconds: its end frame over the frame rate, which
    /// is shorter than [`Animation::duration_seconds`] when a key lies after the end frame.
    #[must_use]
    pub fn duration_seconds(&self) -> f32 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "frames are far below the 24 bits an f32 holds exactly"
        )]
        let frames = self.end_frame as f32;
        frames / f32::from(self.frames_per_second.max(1))
    }

    /// Returns how many tracks [`crate::Animation::bind`] would match to a bone of `bones`.
    #[must_use]
    pub fn bound_tracks(&self, bones: &[Bone]) -> usize {
        bind_controllers(bones, self.controller_ids.iter().copied())
            .into_iter()
            .flatten()
            .count()
    }

    /// Reads the header of the `.caf` file `file` named `file_name`, as [`crate::Animation::read_caf`]
    /// reads the animation.
    ///
    /// # Errors
    ///
    /// As [`crate::Animation::read_caf`], except for errors in the keys themselves.
    pub fn read_caf<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        let info = chunk_bytes(
            file_name,
            file,
            ivo::ANIM_INFO,
            ANIM_INFO_VERSION,
            "AnimInfo",
        )?;
        let record = read_record(info.get(..RECORD_LEN).unwrap_or(info))
            .map_err(|source| corrupt(file_name, "AnimInfo", source))?;
        let data = chunk_bytes(file_name, file, ivo::CAF_DATA, CAF_DATA_VERSION, "CafData")?;
        let (ids, keys) = read_block(data, 0, CAF_SIGNATURE)
            .and_then(|block| {
                check_budget(declared_keys(data, &block)?, MAX_ANIMATION_KEYS)?;
                listed_tracks(data, &block)
            })
            .map_err(|source| corrupt(file_name, "CafData", source))?;
        let mut header = Self::of(stem(file_name).to_owned(), file_name.to_owned(), record);
        header.controller_ids = ids;
        header.keys = keys;
        Ok(header)
    }
}
