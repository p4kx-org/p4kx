// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::ModelArchiveSource;
use p4kx_extraction::model::Model;

use super::{ArchiveModels, ModelFailure, ProvidedAttachment, ProvidedModel, without_mtl};
use crate::build_models::vertices;
use crate::cdf::{self, MAX_CDF_BYTES};
use crate::cdf_bone::BoneAttachment;

type AttachmentModels<'bone> = HashMap<String, HashMap<Option<&'bone str>, ReadAttachment>>;

#[derive(Clone)]
struct ReadAttachment {
    model: Arc<Model>,
    sources: Arc<[ModelArchiveSource]>,
}

impl ArchiveModels<'_> {
    /// Reads the selected model, drawable bone attachments and omitted bindings.
    pub(super) fn character(&self, entry: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let (definition_origin, read) = self.observed_file(entry, MAX_CDF_BYTES);
        let bytes = read?;
        let unreadable = |reason: String| ModelFailure::Unreadable {
            reason: format!("{entry}: {reason}"),
        };
        let observed = self.observe_nodes && definition_origin.is_some();
        let (mut character, body) = cdf::read_drawing(&bytes, observed, |path| {
            self.drawable_declared_body(path, lod)
        })
        .map_err(|error| match error {
            cdf::DrawError::Definition(reason) => unreadable(reason),
            cdf::DrawError::Body(error) => error,
        })?;
        let mut provided = if let Some(body) = body {
            body
        } else {
            let model_entry = self
                .entry_of(&character.model)
                .ok_or_else(|| unreadable(format!("{} does not exist", character.model)))?;
            self.read_model(model_entry, lod, Vec::new())?
        };
        provided.definition_origin = definition_origin;
        provided.cdf_model_sources = character.model_sources.take().map(Arc::new);
        self.attach_bones(
            &character.bones,
            &mut character.left_out,
            lod,
            &mut provided,
        )?;
        provided.left_out = character.left_out;
        provided.skeleton = character
            .skeleton
            .as_deref()
            .map_or(Ok(None), |path| self.skeleton_problem(path))?;
        if let Some(material) = &character.material {
            provided.model.material_library = Some(without_mtl(material).to_owned());
        }
        Ok(provided)
    }

    fn drawable_declared_body(
        &self,
        path: &str,
        lod: u32,
    ) -> Result<Option<ProvidedModel>, ModelFailure> {
        let Some(entry) = self.entry_of(path) else {
            return Ok(None);
        };
        match self.read_model(entry, lod, Vec::new()) {
            Ok(provided)
                if provided
                    .model
                    .lods
                    .iter()
                    .any(|lod| lod.drawable_material_ids().next().is_some()) =>
            {
                Ok(Some(provided))
            }
            Err(failure @ ModelFailure::Fatal(_)) => Err(failure),
            // Skeleton validation reports the declared input independently of geometry selection.
            Ok(_) | Err(ModelFailure::Missing | ModelFailure::Unreadable { .. }) => Ok(None),
        }
    }

    fn attach_bones(
        &self,
        bones: &[BoneAttachment],
        left_out: &mut Vec<String>,
        lod: u32,
        provided: &mut ProvidedModel,
    ) -> Result<(), ModelFailure> {
        let mut remaining = self
            .max_attachment_vertices
            .saturating_sub(vertices(&provided.model));
        let mut models = HashMap::new();
        for bone in bones {
            if remaining == 0 {
                left_out.push(bone.binding.clone());
                continue;
            }
            match self.attachment(bone, lod, &mut models) {
                Ok(attachment) if vertices(&attachment.model) <= remaining => {
                    remaining -= vertices(&attachment.model);
                    provided.attachments.push(attachment);
                }
                Ok(_) => {
                    remaining = 0;
                    left_out.push(bone.binding.clone());
                }
                Err(ModelFailure::Fatal(source)) => return Err(ModelFailure::Fatal(source)),
                Err(_) => left_out.push(bone.binding.clone()),
            }
        }
        Ok(())
    }

    fn attachment<'bone>(
        &self,
        bone: &'bone BoneAttachment,
        lod: u32,
        models: &mut AttachmentModels<'bone>,
    ) -> Result<ProvidedAttachment, ModelFailure> {
        let entry = self.entry_of(&bone.binding).ok_or(ModelFailure::Missing)?;
        let read = self.attachment_model(&entry, bone.material.as_deref(), lod, models)?;
        Ok(ProvidedAttachment {
            name: bone.name.clone(),
            bone: bone.bone.clone(),
            position: bone.position,
            rotation: bone.rotation,
            entry,
            model: read.model,
            archive_sources: read.sources,
            cdf_sources: bone.sources.clone(),
        })
    }

    fn attachment_model<'bone>(
        &self,
        entry: &str,
        material: Option<&'bone str>,
        lod: u32,
        models: &mut AttachmentModels<'bone>,
    ) -> Result<ReadAttachment, ModelFailure> {
        if let Some(model) = models
            .get(entry)
            .and_then(|variants| variants.get(&material))
        {
            return Ok(model.clone());
        }
        let ProvidedModel {
            entry,
            mut model,
            archive_sources,
            ..
        } = self.read_model(entry.to_owned(), lod, Vec::new())?;
        if let Some(material) = material {
            model.material_library = Some(without_mtl(material).to_owned());
        }
        let read = ReadAttachment {
            model: Arc::new(model),
            sources: archive_sources,
        };
        models
            .entry(entry)
            .or_default()
            .insert(material, read.clone());
        Ok(read)
    }
}
