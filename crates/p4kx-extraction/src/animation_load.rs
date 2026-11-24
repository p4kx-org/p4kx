// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{
    Animation, AnimationDatabaseSelection, AnimationError, AnimationHeader, Dba, DbaHeaders,
    PositionedAnimation, SkippedAnimation, UnreadClipCoordinates,
};

use crate::{ChunkSource, ConversionError, PositionedUnreadAnimation, UnreadAnimation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Caf,
    Dba,
}

pub(crate) fn kind(path: &str) -> Option<Kind> {
    let bytes = path.as_bytes();
    let at = bytes
        .len()
        .checked_sub(4)
        .filter(|&at| at > 0 && !matches!(bytes.get(at - 1), Some(b'/' | b'\\')))?;
    let extension = bytes.get(at..)?;
    if extension.eq_ignore_ascii_case(b".caf") {
        Some(Kind::Caf)
    } else if extension.eq_ignore_ascii_case(b".dba") {
        Some(Kind::Dba)
    } else {
        None
    }
}

pub(crate) struct Loaded<T> {
    pub(crate) animations: Vec<T>,
    pub(crate) unread: Vec<PositionedUnreadAnimation>,
    pub(crate) database: Option<AnimationDatabaseSelection>,
}

impl<T> Loaded<T> {
    const fn new(animations: Vec<T>, unread: Vec<PositionedUnreadAnimation>) -> Self {
        Self {
            animations,
            unread,

            database: None,
        }
    }
}

pub(crate) trait Described {
    fn name(&self) -> &str;
    fn path(&self) -> &str;
    fn additive(&self) -> bool;

    fn additive_coordinates(&self) -> Option<UnreadClipCoordinates> {
        None
    }
}

impl Described for Animation {
    fn name(&self) -> &str {
        &self.name
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn additive(&self) -> bool {
        self.additive
    }
}

impl Described for PositionedAnimation {
    fn name(&self) -> &str {
        self.animation().name()
    }

    fn path(&self) -> &str {
        self.animation().path()
    }

    fn additive(&self) -> bool {
        self.animation().additive()
    }

    fn additive_coordinates(&self) -> Option<UnreadClipCoordinates> {
        self.additive_rejection()
    }
}

impl Described for AnimationHeader {
    fn name(&self) -> &str {
        &self.name
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn additive(&self) -> bool {
        self.additive
    }
}

fn skipped(
    label: &str,
    skipped: Vec<SkippedAnimation>,
    coordinates: Vec<UnreadClipCoordinates>,
) -> Vec<PositionedUnreadAnimation> {
    let mut coordinates = coordinates.into_iter();
    skipped
        .into_iter()
        .map(|skipped| {
            PositionedUnreadAnimation::new(
                UnreadAnimation {
                    source: label.to_owned(),
                    name: Some(skipped.name),
                    reason: skipped.reason.to_string(),
                },
                coordinates.next(),
            )
        })
        .collect()
}

fn file_name(label: &str) -> &str {
    label.rsplit(['/', '\\']).next().unwrap_or(label)
}

fn failed(label: &str) -> impl Fn(AnimationError) -> ConversionError {
    move |source| ConversionError::Animation {
        file: label.to_owned(),
        source,
    }
}

pub(crate) fn load(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
) -> Result<Loaded<Animation>, ConversionError> {
    let name = file_name(label);
    match kind {
        Kind::Caf => {
            let clip = Animation::read_caf(name, source.file()).map_err(failed(label))?;
            Ok(Loaded::new(vec![clip], Vec::new()))
        }
        Kind::Dba => {
            let dba = Dba::read(name, source.file()).map_err(failed(label))?;
            Ok(Loaded::new(
                dba.animations,
                skipped(label, dba.skipped, Vec::new()),
            ))
        }
    }
}

pub(crate) fn load_with_positions(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
) -> Result<Loaded<PositionedAnimation>, ConversionError> {
    load_positioned(source, label, kind, PositionedClipSelection::All)
}

pub(crate) fn load_unread_clips(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
) -> Result<Loaded<PositionedAnimation>, ConversionError> {
    load_positioned(source, label, kind, PositionedClipSelection::Additive)
}

#[derive(Clone, Copy)]
enum PositionedClipSelection {
    All,
    Additive,
}

impl PositionedClipSelection {
    fn needs(self, clip: &PositionedAnimation) -> bool {
        matches!(self, Self::All) || clip.animation().additive()
    }
}

fn load_positioned(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
    selection: PositionedClipSelection,
) -> Result<Loaded<PositionedAnimation>, ConversionError> {
    let name = file_name(label);
    match kind {
        Kind::Caf => {
            let read = match selection {
                PositionedClipSelection::All => Animation::read_caf_with_positions,
                PositionedClipSelection::Additive => Animation::read_caf_with_clip_positions,
            };
            let clip = read(name, source.file()).map_err(failed(label))?;
            let animations = if selection.needs(&clip) {
                vec![clip]
            } else {
                Vec::new()
            };
            Ok(Loaded::new(animations, Vec::new()))
        }
        Kind::Dba => {
            let read = match selection {
                PositionedClipSelection::All => Dba::read_with_positions,
                PositionedClipSelection::Additive => Dba::read_with_clip_positions,
            };
            let mut dba = read(name, source.file()).map_err(failed(label))?;
            if matches!(selection, PositionedClipSelection::Additive) {
                dba.animations.retain(|clip| selection.needs(clip));
            }
            let database = dba.selection().cloned();
            let unread_coordinates = dba.take_skipped_coordinates();
            Ok(Loaded {
                animations: dba.animations,
                unread: skipped(label, dba.skipped, unread_coordinates),
                database,
            })
        }
    }
}

pub(crate) fn load_headers(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
) -> Result<Loaded<AnimationHeader>, ConversionError> {
    let name = file_name(label);
    match kind {
        Kind::Caf => {
            let clip = AnimationHeader::read_caf(name, source.file()).map_err(failed(label))?;
            Ok(Loaded::new(vec![clip], Vec::new()))
        }
        Kind::Dba => {
            let dba = DbaHeaders::read(name, source.file()).map_err(failed(label))?;
            Ok(Loaded::new(
                dba.animations,
                skipped(label, dba.skipped, Vec::new()),
            ))
        }
    }
}

pub(crate) fn load_listed(
    source: &ChunkSource,
    label: &str,
    kind: Kind,
    explicit: bool,
) -> Result<Loaded<AnimationHeader>, ConversionError> {
    if !explicit {
        return load_headers(source, label, kind);
    }
    let loaded = load(source, label, kind)?;
    Ok(Loaded::new(
        loaded
            .animations
            .iter()
            .map(AnimationHeader::of_animation)
            .collect(),
        loaded.unread,
    ))
}

#[cfg(test)]
mod tests {
    use super::{Kind, kind};

    #[test]
    fn a_path_ending_in_an_animation_extension_has_a_kind() {
        assert_eq!(kind("Data\\a\\walk.CAF"), Some(Kind::Caf), "caf");
        assert_eq!(kind("a/pack.dba"), Some(Kind::Dba), "dba");
    }

    #[test]
    fn a_bare_extension_is_no_animation_file() {
        assert_eq!(kind(".caf"), None, "bare");
        assert_eq!(kind("Data\\a\\.dba"), None, "bare in a folder");
        assert_eq!(kind("a/b.caff"), None, "other extension");
    }
}
