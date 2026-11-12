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
    /// Reads the character definition at `entry`: its skin as the model, its bone attachments as
    /// models of their own, and the files of what it cannot draw.
    pub(super) fn character(&self, entry: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let (definition_origin, read) = self.observed_file(entry, MAX_CDF_BYTES);
        let bytes = read?;
        let unreadable = |reason: String| ModelFailure::Unreadable {
            reason: format!("{entry}: {reason}"),
        };
        let mut character = cdf::read(&bytes).map_err(unreadable)?;
        let model_entry = self
            .entry_of(&character.model)
            .ok_or_else(|| unreadable(format!("{} does not exist", character.model)))?;
        let mut provided = self.read_model(model_entry, lod, Vec::new())?;
        provided.definition_origin = definition_origin;
        let mut remaining = self
            .max_attachment_vertices
            .saturating_sub(vertices(&provided.model));
        let mut models = HashMap::new();
        for bone in &character.bones {
            if remaining == 0 {
                character.left_out.push(bone.binding.clone());
                continue;
            }
            match self.attachment(bone, lod, &mut models) {
                Ok(attachment) if vertices(&attachment.model) <= remaining => {
                    remaining -= vertices(&attachment.model);
                    provided.attachments.push(attachment);
                }
                Ok(_) => {
                    remaining = 0;
                    character.left_out.push(bone.binding.clone());
                }
                Err(ModelFailure::Fatal(source)) => return Err(ModelFailure::Fatal(source)),
                Err(_) => character.left_out.push(bone.binding.clone()),
            }
        }
        drop(models);
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
