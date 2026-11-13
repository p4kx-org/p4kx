// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use anyhow::Result;
use p4kx_extraction::{
    DecimalU64, FieldOrigin, FieldPath, OriginOutputAddress, RiggedBufferSource, RiggedGlbEmission,
    RiggedJsonAddress, SourceUses,
};

use super::{Clips, Contributors, ImageOrigin, Parts};

pub(super) type Fields = Vec<(OriginOutputAddress, FieldOrigin)>;

pub(super) fn fields(
    uses: &SourceUses,
    emission: &RiggedGlbEmission,
    parts: &Parts,
    clips: &Clips,
    images: &[ImageOrigin],
) -> Result<Fields> {
    let input = Inputs {
        uses,
        parts,
        clips,
        images,
    };
    let mut found = Fields::new();
    let mut known = HashMap::new();
    for view in emission.buffer_views() {
        let origin = known
            .entry(view.source)
            .or_insert_with(|| input.source(view.source));
        let path = FieldPath::parse(&format!("/bufferViews/{}", view.index))?;
        found.push((
            OriginOutputAddress::GlbBinaryBytes {
                path,
                start: DecimalU64::new(view.start),
                end_exclusive: DecimalU64::new(view.end_exclusive),
            },
            origin.clone(),
        ));
    }
    for datum in emission.json_addresses() {
        let Some((path, origin)) = input.address(*datum, &mut known) else {
            continue;
        };
        found.push((
            OriginOutputAddress::GlbJsonField {
                path: FieldPath::parse(&path)?,
            },
            origin,
        ));
    }
    for image in images {
        super::transfer::addresses(&mut found, image, uses)?;
    }
    Ok(found)
}

struct Inputs<'a> {
    uses: &'a SourceUses,
    parts: &'a Parts,
    clips: &'a Clips,
    images: &'a [ImageOrigin],
}

impl Inputs<'_> {
    fn address(
        &self,
        datum: RiggedJsonAddress,
        known: &mut HashMap<RiggedBufferSource, FieldOrigin>,
    ) -> Option<(String, FieldOrigin)> {
        use RiggedBufferSource as Source;
        let cache = |source, known: &mut HashMap<_, _>| {
            known
                .entry(source)
                .or_insert_with(|| self.source(source))
                .clone()
        };
        Some(match datum {
            RiggedJsonAddress::Image(index) => (
                format!("/images/{index}"),
                cache(Source::Image(index), known),
            ),
            RiggedJsonAddress::Texture { index, image } => (
                format!("/textures/{index}"),
                cache(Source::Image(image), known),
            ),
            RiggedJsonAddress::Node { index, part } => {
                let origin = self.parts.individual.get(part).map(|part| &part.selection);
                (format!("/nodes/{index}"), self.measured(origin))
            }
            RiggedJsonAddress::Bone(index) => {
                (format!("/nodes/{index}"), cache(Source::Skeleton, known))
            }
            RiggedJsonAddress::Material { index, part, slot } => (
                format!("/materials/{index}"),
                slot.map_or_else(
                    || self.uses.origin(&[], true),
                    |slot| {
                        self.measured(
                            self.parts
                                .individual
                                .get(part)
                                .and_then(|part| part.material_slots.get(&slot)),
                        )
                    },
                ),
            ),
            RiggedJsonAddress::Mesh { index, part } => (
                format!("/meshes/{index}"),
                self.combined(
                    self.parts
                        .individual
                        .get(part)
                        .map(|part| (&part.geometry, &part.morphs)),
                ),
            ),
            RiggedJsonAddress::Skin(index) => {
                (format!("/skins/{index}"), cache(Source::Skeleton, known))
            }
            RiggedJsonAddress::Animation { index, clip } => (
                format!("/animations/{index}"),
                cache(Source::AnimationPose(clip), known),
            ),
            RiggedJsonAddress::Accessor { index, source } => {
                (format!("/accessors/{index}"), cache(source, known))
            }
            _ => return None,
        })
    }

    fn measured(&self, origin: Option<&Contributors>) -> FieldOrigin {
        origin.map_or_else(
            || self.uses.origin(&[], false),
            |origin| origin.origin(self.uses),
        )
    }
    fn source(&self, source: RiggedBufferSource) -> FieldOrigin {
        use RiggedBufferSource as Source;
        let part = |part: usize| self.parts.individual.get(part);
        let origin = match source {
            Source::Geometry(index) => part(index).map(|value| &value.geometry),
            Source::Surface(index) => return self.surface(index),
            Source::Skin(index) => {
                return self
                    .combined(part(index).map(|value| (&value.geometry, &self.parts.bones)));
            }
            Source::Morph(index) => part(index).map(|value| &value.morphs),
            Source::Skeleton => Some(&self.parts.bones),
            Source::AnimationTimes(clip) => self.clips.inputs.get(clip),
            Source::AnimationPose(clip) => self.clips.all.get(clip),
            Source::AnimationWeights { clip, part: index } => {
                return self.combined(
                    self.clips
                        .inputs
                        .get(clip)
                        .zip(part(index))
                        .map(|(clip, part)| (clip, &part.morphs)),
                );
            }
            Source::Image(index) => {
                return self.images.get(index).map_or_else(
                    || self.uses.origin(&[], false),
                    |image| image.origin.clone(),
                );
            }
            _ => None,
        };
        self.measured(origin)
    }

    fn surface(&self, index: usize) -> FieldOrigin {
        let Some(part) = self.parts.individual.get(index) else {
            return self.uses.origin(&[], false);
        };
        let mut surface = Contributors::default();
        surface.extend(&part.geometry);
        surface.extend(&part.shaping);
        if part.shaped {
            surface.extend(&self.parts.shape);
        }
        surface.origin(self.uses)
    }

    fn combined(&self, sources: Option<(&Contributors, &Contributors)>) -> FieldOrigin {
        let Some((first, second)) = sources else {
            return self.uses.origin(&[], false);
        };
        let mut combined = Contributors::default();
        combined.extend(first);
        combined.extend(second);
        combined.origin(self.uses)
    }
}
