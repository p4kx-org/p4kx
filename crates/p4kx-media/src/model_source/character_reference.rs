// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ArchiveMaterialPaths;

use super::character_origin::CharacterElement;
use super::path_reference::Observation;
use super::payload::ModelSourceFamily;
use super::{material_path, origins, path_reference};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    Identity, MediaError, MediaWriter, OriginalIdentifier, Reference, ReferenceResolution,
    ReferenceUnavailable, SourceContext,
};

const BINDING: &str = "/data/payload/binding";
const MATERIAL: &str = "/data/payload/material";
const MATERIAL_REFERENCE: &str = "/data/payload/material_reference";

pub(super) struct CharacterReferences<'context, 'input> {
    pub(super) context: &'context SourceContext<'input>,
    pub(super) strings_lossy: bool,
}

pub(super) struct AttachmentLinks<'document> {
    pub(super) binding: Option<Reference>,
    pub(super) material: Option<&'document str>,
    pub(super) material_reference: Option<Box<Reference>>,
}

impl CharacterReferences<'_, '_> {
    pub(super) fn field(
        &self,
        element: &CharacterElement<'_, '_>,
        name: &str,
        pointer: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<Reference>, MediaError> {
        let (value, authored) = element.value(name, writer)?;
        let Some(path) = value else {
            origins::at(prepared, pointer, authored, writer)?;
            return Ok(None);
        };
        path_reference::field(
            self.context.directory,
            path,
            pointer,
            &authored,
            prepared,
            writer,
        )
        .map(Some)
    }

    pub(super) fn attachment<'document>(
        &self,
        element: &CharacterElement<'_, 'document>,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<AttachmentLinks<'document>, MediaError> {
        let (binding_value, binding_origin) = element.value("Binding", writer)?;
        let binding = binding_value
            .map(|path| path_reference::lookup(self.context.directory, path, writer))
            .transpose()?;
        let binding_selection = if let Some(observed) = &binding {
            observed.write(BINDING, &binding_origin, &binding_origin, prepared, writer)?
        } else {
            origins::at(prepared, BINDING, binding_origin.clone(), writer)?;
            binding_origin
        };
        let (material, authored) = element.value("Material", writer)?;
        origins::at(prepared, MATERIAL, authored.clone(), writer)?;
        let material_reference = material
            .map(|value| {
                let base = MaterialBase {
                    binding: binding.as_ref(),
                    binding_text: binding_value,
                    selection: &binding_selection,
                    strings_lossy: self.strings_lossy,
                };
                self.material(value, base, &authored, prepared, writer)
                    .map(Box::new)
            })
            .transpose()?;
        Ok(AttachmentLinks {
            binding: binding.map(|observed| observed.reference),
            material,
            material_reference,
        })
    }

    fn material(
        &self,
        value: &str,
        base: MaterialBase<'_>,
        authored: &OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Reference, MediaError> {
        let (paths, selector) = base.material_paths(value, authored, writer)?;
        let observed = material_path::observe(
            self.context.directory,
            OriginalIdentifier::new_with_path(value),
            paths,
            writer,
        )?;
        observed.write(MATERIAL_REFERENCE, authored, &selector, prepared, writer)?;
        Ok(observed.reference)
    }
}

struct MaterialBase<'a> {
    binding: Option<&'a Observation>,
    binding_text: Option<&'a str>,
    selection: &'a OriginContributions,
    strings_lossy: bool,
}

impl MaterialBase<'_> {
    fn material_paths(
        self,
        value: &str,
        authored: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<
        (
            Result<ArchiveMaterialPaths, ReferenceUnavailable>,
            OriginContributions,
        ),
        MediaError,
    > {
        let model = self
            .binding_text
            .filter(|path| !self.maybe_repaired(path))
            .and(self.binding)
            .and_then(bound_model);
        let paths = if self.maybe_repaired(value) {
            Err(ReferenceUnavailable::InvalidIdentifier)
        } else {
            let normalized = value.replace('\\', "/");
            ArchiveMaterialPaths::new(model.unwrap_or_default(), &normalized)
                .map_err(|_| ReferenceUnavailable::InvalidIdentifier)
        };
        if !paths.as_ref().is_ok_and(|paths| paths.sibling.is_some()) {
            return Ok((paths, authored.clone()));
        }
        let mut selector = writer.capture_origins();
        selector.include_original(authored)?;
        selector.include_original(self.selection)?;
        let paths = if model.is_some() {
            paths
        } else {
            Err(ReferenceUnavailable::IncompleteInventory)
        };
        Ok((paths, selector.finish()))
    }

    fn maybe_repaired(&self, value: &str) -> bool {
        self.strings_lossy && value.contains(char::REPLACEMENT_CHARACTER)
    }
}

fn bound_model(binding: &Observation) -> Option<&str> {
    let (ReferenceResolution::Resolved { target }
    | ReferenceResolution::NotSelected { target, .. }) = binding.reference.resolution()
    else {
        return None;
    };
    let Identity::Entry { entry, .. } = &target.source else {
        return None;
    };
    (entry.raw_path_hex.is_none()
        && ModelSourceFamily::of(&entry.path).is_some_and(ModelSourceFamily::is_model))
    .then_some(entry.path.as_str())
}
