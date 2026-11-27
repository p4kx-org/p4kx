// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldPath;
use p4kx_extraction::model::character_definition::is_definition_path;

use super::payload::ModelSourceFamily;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_lookup::ObservedInputLookup;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::reference_resolve::ReferenceObservations;
use crate::{
    CandidateInventory, DirectoryLookup, MediaError, MediaKind, MediaWriter, OriginalIdentifier,
    Reference, ReferenceResolution, ReferenceUnavailable, origin_error,
};

pub(super) fn field(
    directory: &DirectoryLookup<'_>,
    path: &str,
    pointer: &str,
    authored: &OriginContributions,
    prepared: &mut PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<Reference, MediaError> {
    let observed = lookup(directory, path, writer)?;
    observed.write(pointer, authored, authored, prepared, writer)?;
    Ok(observed.reference)
}

pub(super) struct Observation {
    pub(super) reference: Reference,
    evidence: ReferenceObservations<MediaOriginInput>,
    inventory: OriginContributions,
}

impl Observation {
    pub(super) fn unavailable(
        original: OriginalIdentifier,
        reason: ReferenceUnavailable,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let (reference, evidence) = writer.try_resolve_reference_observed(
            original,
            std::iter::empty(),
            CandidateInventory::Unavailable(reason),
        )?;
        Ok(Self {
            reference,
            evidence,
            inventory: OriginContributions::independent(&writer.origins.owner),
        })
    }

    pub(super) fn write(
        &self,
        pointer: &str,
        authored: &OriginContributions,
        selector: &OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let prefix = FieldPath::parse(pointer).map_err(|cause| origin_error::address(&cause))?;
        let semantic = OriginContributions::independent(&writer.origins.owner);
        self.inputs(authored, selector, &semantic)
            .write_with_origin(prepared, &prefix, &self.reference, writer)
    }

    const fn inputs<'a>(
        &'a self,
        original: &'a OriginContributions,
        selector: &'a OriginContributions,
        semantic: &'a OriginContributions,
    ) -> ReferenceOriginInputs<'a, MediaOriginInput> {
        ReferenceOriginInputs {
            original,
            selector,
            inventory: &self.inventory,
            semantic,
            candidates: &self.evidence,
            read_selected: false,
            identifier: ReferenceIdentifier::Path,
            subresource: ReferenceSubresource::None,
        }
    }
}

impl From<ObservedInputLookup<'_>> for Observation {
    fn from(observed: ObservedInputLookup<'_>) -> Self {
        Self {
            reference: observed.lookup.reference,
            evidence: observed.evidence,
            inventory: observed.inventory,
        }
    }
}

pub(super) fn lookup(
    directory: &DirectoryLookup<'_>,
    original: &str,
    writer: &mut MediaWriter<'_>,
) -> Result<Observation, MediaError> {
    let identifier = OriginalIdentifier::new_with_path(original);
    let path = original.trim_start_matches(['/', '\\']);
    if path.contains([':', '!', '\0'])
        || path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Observation::unavailable(
            identifier,
            ReferenceUnavailable::InvalidIdentifier,
            writer,
        );
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
    Ok(observed.into())
}
