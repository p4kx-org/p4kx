// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldPath;
use p4kx_extraction::model::character_definition::is_definition_path;

use super::payload::ModelSourceFamily;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
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
    let (reference, candidates, inventory) = lookup(directory, path, writer)?;
    let prefix = FieldPath::parse(pointer).map_err(|cause| origin_error::address(&cause))?;
    ReferenceOriginInputs {
        original: authored,
        selector: authored,
        inventory: &inventory,
        semantic: &OriginContributions::independent(&writer.origins.owner),
        candidates: &candidates,
        read_selected: false,
        identifier: ReferenceIdentifier::Path,
        subresource: ReferenceSubresource::None,
    }
    .write(prepared, &prefix, &reference, writer)?;
    Ok(reference)
}
fn lookup(
    directory: &DirectoryLookup<'_>,
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
        return Ok((
            reference,
            evidence,
            OriginContributions::independent(&writer.origins.owner),
        ));
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
