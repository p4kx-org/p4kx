// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Bone;
use crate::controller_binding::{BoneControllers, ControllerEligibility};

/// The keys of one channel of a track: the frames they sit at and their values.
///
/// The frames increase strictly; there is one value per frame.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Keys<T> {
    /// The frames of the keys, counted from the start of the animation.
    pub frames: Vec<u16>,
    /// The values of the keys.
    pub values: Vec<T>,
}

/// The keys of one controller, which addresses a bone by [`Bone::controller_id`].
///
/// Rotations and translations are absolute and relative to the bone's parent, in `CryEngine`
/// axes (Z up, metres), like [`Bone::relative`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Track {
    /// The controller the keys belong to: the CRC-32 of the bone's name, as the file spells it.
    pub controller_id: u32,
    /// The rotations as unit quaternions `x, y, z, w`; consecutive keys lie in the same
    /// hemisphere, so interpolating between them takes the short way. `None` when the track
    /// rotates nothing.
    pub rotations: Option<Keys<[f32; 4]>>,
    /// The translations in metres, or `None` when the track moves nothing.
    pub translations: Option<Keys<[f32; 3]>>,
}

impl Track {
    /// Returns the number of keys of both channels.
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.rotations.as_ref().map_or(0, |keys| keys.frames.len())
            + self
                .translations
                .as_ref()
                .map_or(0, |keys| keys.frames.len())
    }

    fn last_frame(&self) -> u16 {
        let rotation = self
            .rotations
            .as_ref()
            .and_then(|keys| keys.frames.last().copied());
        let translation = self
            .translations
            .as_ref()
            .and_then(|keys| keys.frames.last().copied());
        rotation.max(translation).unwrap_or(0)
    }
}

/// One animation of a `.caf` file or of a `.dba` database: tracks per bone over frames.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Animation {
    /// The animation's name: the file name without folder and extension.
    pub name: String,
    /// The file name or path the animation is stored under, e.g. the `.caf` path a database
    /// lists it under.
    pub path: String,
    /// The number of frames per second.
    pub frames_per_second: u16,
    /// The last frame of the animation.
    pub end_frame: u32,
    /// Whether the keys are offsets to be added to another animation instead of poses.
    pub additive: bool,
    /// The tracks, one per controller.
    pub tracks: Vec<Track>,
}

/// The tracks of an animation sorted by what [`Animation::bind`] found for them.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BoundTracks<'a> {
    /// The tracks whose bone exists, each with the bone's index in the skeleton.
    pub bound: Vec<(usize, &'a Track)>,
    /// The controller IDs of the tracks without a bone: bones the skeleton lacks, and a bone
    /// that an earlier track of the animation already animates.
    pub unbound: Vec<u32>,
}

impl Animation {
    /// Returns the length of the animation in seconds: its end frame, or the last frame of a
    /// key if that is later, over the frame rate.
    #[must_use]
    pub fn duration_seconds(&self) -> f32 {
        let last = self.tracks.iter().map(Track::last_frame).max().unwrap_or(0);
        let frames = self.end_frame.max(u32::from(last));
        self.seconds(frames)
    }

    /// Returns the time of `frame` in seconds.
    #[must_use]
    pub fn seconds(&self, frame: u32) -> f32 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "frames are far below the 24 bits an f32 holds exactly"
        )]
        let frames = frame as f32;
        frames / f32::from(self.frames_per_second.max(1))
    }

    /// Returns the number of keys of all tracks.
    #[must_use]
    pub fn key_count(&self) -> usize {
        self.tracks.iter().map(Track::key_count).sum()
    }

    /// Matches the tracks to `bones` by controller ID. A bone gets the first track that names
    /// it; when several bones share a controller ID, the first of them gets the track.
    #[must_use]
    pub fn bind(&self, bones: &[Bone]) -> BoundTracks<'_> {
        let matches = self.track_binding_indices(bones);
        let mut bound = Vec::new();
        let mut unbound = Vec::new();
        for (track, bone) in self.tracks.iter().zip(matches) {
            match bone {
                Some(bone) => bound.push((bone, track)),
                None => unbound.push(track.controller_id),
            }
        }
        BoundTracks { bound, unbound }
    }

    /// Returns each decoded track's first matching bone index in track order.
    #[must_use]
    pub fn track_binding_indices(&self, bones: &[Bone]) -> Vec<Option<usize>> {
        bind_controllers(bones, self.tracks.iter().map(|track| track.controller_id))
    }
}

/// Matches `controllers`, in order, to the index of the bone of `bones` each animates: the first
/// bone with that controller ID, and each bone only once.
pub(crate) fn bind_controllers(
    bones: &[Bone],
    controllers: impl IntoIterator<Item = u32>,
) -> Vec<Option<usize>> {
    let bones = BoneControllers::new(bones);
    let mut binder = bones.binder();
    controllers
        .into_iter()
        .enumerate()
        .map(|(ordinal, id)| {
            binder
                .observe(ordinal, id, ControllerEligibility::Track)
                .bound_bone()
        })
        .collect()
}
