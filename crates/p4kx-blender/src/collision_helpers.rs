// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io;
use std::path::Path;

use p4kx_assembly::Assembly;
use p4kx_extraction::ModelArchiveSource;
use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{ArchiveWatch, EntryOrigin, SourceUses};
use p4kx_model::Model;

use crate::collision_helper_document::{CollisionHelpers, ModelCollision, ModelInstance};
use crate::collision_helper_model::{OriginalModels, error};
use crate::package_receipt::PackageFile;
use crate::{PackageResult, PackageWriteContext};

pub(crate) fn write(
    manifest: &mut crate::Manifest,
    assembly: Option<&Assembly>,
    lod: u32,
    staging: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Option<PackageFile>> {
    let Some(assembly) = assembly else {
        return Ok(None);
    };
    let Some(helpers) = capture(assembly, lod, context)? else {
        return Ok(None);
    };
    let file = helpers.write(staging, context)?;
    manifest.collision_helpers = Some(crate::CollisionHelperReference::new());
    crate::manifest::COLLISION_HELPER_FORMAT_VERSION.clone_into(&mut manifest.version);
    Ok(Some(file))
}

fn capture(
    assembly: &Assembly,
    lod: u32,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Option<CollisionHelpers>> {
    let Some(archive) = context.collision_source else {
        return Ok(None);
    };
    let watch = ArchiveWatch::new(archive);
    watch.check().map_err(error)?;
    let evidence = SourceUses::with_compact_use_refs();
    let input = OriginalModels {
        archive,
        watch: &watch,
        evidence: &evidence,
        context,
    };
    let mut collection = Collection::default();
    for (node, part) in assembly.parts().iter().enumerate() {
        if let Some(model) = part.model() {
            collection.model(node, model, part.model_archive_sources(), lod, &input)?;
        }
    }
    capture_attachments(&mut collection, assembly, lod, &input)?;
    watch.check().map_err(error)?;
    if collection.instances.is_empty() {
        return Ok(None);
    }
    let inventory = evidence.freeze().map_err(error)?;
    Ok(Some(CollisionHelpers::new(
        collection.models,
        collection.instances,
        inventory,
    )))
}

fn capture_attachments(
    collection: &mut Collection,
    assembly: &Assembly,
    lod: u32,
    input: &OriginalModels<'_, '_>,
) -> PackageResult<()> {
    let mut definitions = HashMap::<usize, Vec<u8>>::new();
    for (ordinal, (part, attachment, piece)) in assembly.indexed_attachments().enumerate() {
        let node = assembly.parts().len() + ordinal;
        let start = collection.instances.len();
        collection.model(
            node,
            piece.model(),
            piece.model_archive_sources(),
            lod,
            input,
        )?;
        if start == collection.instances.len() {
            continue;
        }
        let Some(source) = piece.model_definition_origin() else {
            continue;
        };
        let owner = assembly
            .parts()
            .get(part)
            .ok_or_else(|| error(io::Error::other("collision attachment lost its part")))?;
        if !input.archive.owns_origin(source) {
            return Err(error(io::Error::other(
                "collision attachment comes from another Archive",
            )));
        }
        let bytes = match definitions.entry(source.entry().index()) {
            std::collections::hash_map::Entry::Vacant(slot) => slot.insert(input.bytes(source)?),
            std::collections::hash_map::Entry::Occupied(slot) => slot.into_mut(),
        };
        for instance in collection.instances.get_mut(start..).unwrap_or_default() {
            instance.cdf =
                crate::collision_helper_cdf::capture(owner, attachment, piece, bytes, input)?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Collection {
    seen: HashMap<usize, Option<usize>>,
    models: Vec<ModelCollision>,
    instances: Vec<ModelInstance>,
}

impl Collection {
    fn model(
        &mut self,
        node: usize,
        model: &Model,
        sources: &[ModelArchiveSource],
        lod: u32,
        input: &OriginalModels<'_, '_>,
    ) -> PackageResult<()> {
        let level = model.nearest_lod(lod).map(|lod| lod.level);
        for source in sources {
            let include = match source.file() {
                p4kx_model::ModelFile::Model => true,
                p4kx_model::ModelFile::LodModel(found) => Some(found) == level,
                _ => false,
            };
            if !include
                || !model.chunks.iter().any(|chunk| {
                    chunk.file == source.file() && chunk.chunk_type == ivo::STAT_OBJ_PHYSICS
                })
            {
                continue;
            }
            let Some(origin) = source.origin() else {
                continue;
            };
            input.check()?;
            let Some(definition) = self.definition(origin, input)? else {
                continue;
            };
            self.instances.push(ModelInstance {
                node,
                model: definition,
                cdf: None,
            });
        }
        Ok(())
    }

    fn definition(
        &mut self,
        origin: &EntryOrigin,
        input: &OriginalModels<'_, '_>,
    ) -> PackageResult<Option<usize>> {
        if !input.archive.owns_origin(origin) {
            return Err(error(io::Error::other(
                "collision model comes from another Archive",
            )));
        }
        let key = origin.entry().index();
        Ok(match self.seen.entry(key) {
            std::collections::hash_map::Entry::Occupied(slot) => *slot.get(),
            std::collections::hash_map::Entry::Vacant(slot) => {
                *slot.insert(input.load(origin)?.map(|model| {
                    let next = self.models.len();
                    self.models.push(model);
                    next
                }))
            }
        })
    }
}
