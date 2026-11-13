// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldPath;
use p4kx_extraction::model::character_definition::is_definition_path;

use super::character_origin::CharacterElement;
use super::origins::{self, ModelOrigins};
use super::payload::ModelSourceFamily;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::reference_resolve::ReferenceObservations;
use crate::{
    CandidateInventory, MediaError, MediaKind, MediaWriter, OriginalIdentifier, Reference,
    ReferenceResolution, ReferenceUnavailable, SourceContext, origin_error,
};

pub(super) struct CharacterReferences<'context, 'input> {
    pub(super) context: &'context SourceContext<'input>,
    pub(super) origins: &'context ModelOrigins<'input>,
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
        let (reference, candidates, inventory) = self.lookup(path, writer)?;
        let prefix = FieldPath::parse(pointer).map_err(|cause| origin_error::address(&cause))?;
        ReferenceOriginInputs {
            original: &authored,
            selector: &authored,
            inventory: &inventory,
            semantic: &self.origins.independent(),
            candidates: &candidates,
            read_selected: false,
            identifier: ReferenceIdentifier::Path,
            subresource: ReferenceSubresource::None,
        }
        .write(prepared, &prefix, &reference, writer)?;
        Ok(Some(reference))
    }

    fn lookup(
        &self,
        original: &str,
        writer: &mut MediaWriter<'_>,
    ) -> Result<
        (
            Reference,
            ReferenceObservations<MediaOriginInput>,
            OriginContributions,
        ),
        MediaError,
    > {
        let directory = &self.context.directory;
        let identifier = OriginalIdentifier::new_with_path(original);
        let path = original.trim_start_matches(['/', '\\']);
        if path.contains([':', '!', '\0'])
            || path
                .split(['/', '\\'])
                .any(|part| matches!(part, "" | "." | ".."))
        {
            let (reference, evidence) = writer.try_resolve_reference_observed(
                identifier,
                std::iter::empty(),
                CandidateInventory::Unavailable(ReferenceUnavailable::InvalidIdentifier),
            )?;
            return Ok((reference, evidence, self.origins.independent()));
        }
        let kind = (ModelSourceFamily::of(path).is_some() || is_definition_path(path))
            .then_some(MediaKind::Model);
        let prefixed = directory.lookup_input_observed(
            identifier.clone(),
            &format!("Data\\{path}"),
            kind,
            writer,
        )?;
        let observed = if matches!(
            prefixed.lookup.reference.resolution(),
            ReferenceResolution::Missing
        ) {
            directory.lookup_input_observed(identifier, path, kind, writer)?
        } else {
            prefixed
        };
        Ok((
            observed.lookup.reference,
            observed.evidence,
            observed.inventory,
        ))
    }
}
