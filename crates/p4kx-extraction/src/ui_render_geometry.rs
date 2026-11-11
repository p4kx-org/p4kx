// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io;

use p4kx_gfx::{
    AssetOrigins, Background, SceneDefinitionOrigin, SceneDrawOrigin, SceneOrigins, TagSite,
    TimelineOrigins,
};

use crate::ui_render::ComposedFrame;
use crate::{FieldOrigin, SourceUseRole, SourceUses, UiMovieEvidence, UiSourceSite as Site};

use SourceUseRole::{Conversion, Selector};

pub(crate) fn register(
    registry: &SourceUses,
    movie: &UiMovieEvidence,
    asset_origins: &AssetOrigins,
    frame: &ComposedFrame<'_>,
    item_inputs: &[usize],
    background: Background,
) -> io::Result<FieldOrigin> {
    let observed = frame.origins.as_ref().ok_or_else(inconsistent)?;
    let mut geometry = Geometry {
        assets: asset_origins,
        scene: observed,
        tags: BTreeMap::new(),
        contexts: BTreeSet::new(),
        timelines: BTreeSet::new(),
    };
    geometry.timelines.insert(None);
    if matches!(background, Background::Movie)
        && let Some(site) = asset_origins.background
    {
        geometry.record(site, "backgroundColor", Conversion);
    }
    let mut masks = BTreeSet::new();
    for &ordinal in item_inputs {
        let item = frame.scene.items.get(ordinal).ok_or_else(inconsistent)?;
        geometry.draw(*observed.items.get(ordinal).ok_or_else(inconsistent)?)?;
        let mut next = item.mask;
        while let Some(ordinal) = next {
            if !masks.insert(ordinal) {
                break;
            }
            next = frame
                .scene
                .masks
                .get(ordinal)
                .ok_or_else(inconsistent)?
                .parent;
        }
    }
    for ordinal in masks {
        geometry.mask(ordinal)?;
    }
    let uses = geometry.bind(movie)?;
    Ok(registry.origin(&uses, movie.is_bound()))
}

struct Geometry<'a> {
    assets: &'a AssetOrigins,
    scene: &'a SceneOrigins,
    tags: BTreeMap<(usize, &'static str), (TagSite, SourceUseRole)>,
    contexts: BTreeSet<usize>,
    timelines: BTreeSet<Option<usize>>,
}

impl Geometry<'_> {
    fn record(&mut self, site: TagSite, field: &'static str, role: SourceUseRole) {
        self.tags.insert((site.header.offset, field), (site, role));
    }

    fn timeline(&self, sprite: Option<usize>) -> io::Result<&TimelineOrigins> {
        sprite.map_or(Ok(&self.assets.timeline), |ordinal| {
            self.assets
                .sprites
                .get(ordinal)
                .map(|sprite| &sprite.timeline)
                .ok_or_else(inconsistent)
        })
    }

    fn mask(&mut self, ordinal: usize) -> io::Result<()> {
        let mask = self.scene.masks.get(ordinal).ok_or_else(inconsistent)?;
        let context = self
            .scene
            .placements
            .get(mask.placement.ok_or_else(inconsistent)?)
            .ok_or_else(inconsistent)?;
        let clip = self
            .timeline(context.timeline)?
            .first_frame
            .get(&context.depth)
            .and_then(|origin| origin.clip_depth)
            .ok_or_else(inconsistent)?;
        self.record(clip, "clipDepth", Selector);
        self.placement(mask.placement)?;
        for &draw in &mask.draws {
            self.draw(draw)?;
        }
        Ok(())
    }

    fn draw(&mut self, draw: SceneDrawOrigin) -> io::Result<()> {
        let (declaration, name) = match draw.definition.ok_or_else(inconsistent)? {
            SceneDefinitionOrigin::Shape(ordinal) => (self.assets.shapes.get(ordinal), "shape"),
            SceneDefinitionOrigin::Morph(ordinal) => {
                let context = self
                    .scene
                    .placements
                    .get(draw.placement.ok_or_else(inconsistent)?)
                    .ok_or_else(inconsistent)?;
                if let Some(ratio) = self
                    .timeline(context.timeline)?
                    .first_frame
                    .get(&context.depth)
                    .and_then(|origin| origin.ratio)
                {
                    self.record(ratio, "morphRatio", Conversion);
                }
                (self.assets.morph_shapes.get(ordinal), "morphShape")
            }
            SceneDefinitionOrigin::StaticText(ordinal) => {
                (self.assets.static_texts.get(ordinal), "staticText")
            }
            SceneDefinitionOrigin::EditText(ordinal) => {
                (self.assets.edit_texts.get(ordinal), "editText")
            }
            _ => return Err(inconsistent()),
        };
        self.record(*declaration.ok_or_else(inconsistent)?, name, Conversion);
        self.placement(draw.placement)
    }

    fn placement(&mut self, mut next: Option<usize>) -> io::Result<()> {
        while let Some(ordinal) = next {
            if !self.contexts.insert(ordinal) {
                break;
            }
            let context = self
                .scene
                .placements
                .get(ordinal)
                .ok_or_else(inconsistent)?;
            let origin = *self
                .timeline(context.timeline)?
                .first_frame
                .get(&context.depth)
                .ok_or_else(inconsistent)?;
            self.timelines.insert(context.timeline);
            if let Some(sprite) = context.timeline {
                let site = self
                    .assets
                    .sprites
                    .get(sprite)
                    .ok_or_else(inconsistent)?
                    .definition;
                self.record(site, "spriteDefinition", Selector);
            }
            self.record(
                origin.character.ok_or_else(inconsistent)?,
                "characterId",
                Selector,
            );
            for (site, field, role) in [
                (origin.matrix, "placementMatrix", Conversion),
                (origin.color_transform, "colorTransform", Conversion),
                (origin.visible, "visible", Selector),
            ] {
                if let Some(site) = site {
                    self.record(site, field, role);
                }
            }
            next = context.parent;
        }
        Ok(())
    }

    fn bind(&self, movie: &UiMovieEvidence) -> io::Result<Vec<crate::SourceUse>> {
        let mut uses = vec![
            movie
                .decoded(Site::Bytes(movie.stage()), "stageRectangle", Conversion)
                .map_err(io::Error::other)?,
        ];
        for &ordinal in &self.timelines {
            let timeline = self.timeline(ordinal)?;
            let boundary = timeline
                .frames
                .runs()
                .first()
                .map_or(Site::Survey(timeline.scan), |run| Site::Bytes(run.first));
            uses.push(
                movie
                    .decoded(boundary, "firstFrameBoundary", Selector)
                    .map_err(io::Error::other)?,
            );
        }
        for (&(_, field), &(site, role)) in &self.tags {
            uses.push(
                movie
                    .decoded(Site::Body(site), field, role)
                    .map_err(io::Error::other)?,
            );
        }
        movie.append_decoding(&mut uses);
        Ok(uses)
    }
}

fn inconsistent() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "drawn geometry has no observed source",
    )
}
