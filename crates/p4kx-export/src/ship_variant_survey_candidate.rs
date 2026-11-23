// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{
    CancelToken, FieldOrigin, Record, RecordTree, SourceLocation, SourceUseRole, SourceValueState,
    StructValue,
};

use crate::build::original_input::{self, OriginalCatalogInput};
use crate::catalog_v3::authored_fields::ship_source_read::{
    DefinitionMembership, read_variant_operands,
};
use crate::digest::Digest;
use crate::entity_selection::{self, Selected};
use crate::export_diagnostic::ExportSourceRole;
use crate::guid_text::CatalogGuid;
use crate::loadout::ledger::{Capture, structure_binding};
use crate::playability_projection::OwnerProof;
use crate::playability_rules::Evidence;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::source_header;
use crate::variant_evidence::VariantEvidence;
use crate::variants::{ShipDefinitionKey, VariantMember, VariantRank};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) enum Candidate {
    Other,
    Unknown(VariantEvidence),
    Admitted(VariantEvidence),
}

pub(super) enum Membership<'source> {
    Unrelated,
    Unknown(VariantEvidence),
    Reached {
        definition: ShipDefinitionKey,
        structural: StructuralCandidate<'source>,
    },
}

pub(super) struct StructuralCandidate<'source> {
    pub(super) evidence: VariantEvidence,
    pub(super) digest: Option<Digest>,
    pub(super) rank: Option<Ranking<'source>>,
}

pub(super) struct Ranking<'source> {
    pub(super) rank: VariantRank<'source>,
    pub(super) evidence: VariantEvidence,
}

pub(super) fn admission(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
) -> Result<Candidate, CatalogError> {
    let mut fields = Vec::new();
    let actual = entity_selection::select_family_observed(record, Kind::Ships, |field, role| {
        fields.push((field, role));
    });
    if matches!(&actual, Ok(found) if *found != Some(Selected::Entity(Kind::Ships))) {
        return Ok(Candidate::Other);
    }
    let owner = input.record_context(record)?;
    let mut uses = Vec::new();
    for (field, role) in fields {
        owner.append_header_input(field, role, &mut uses)?;
    }
    for field in [RecordHeaderField::RecordIndex, RecordHeaderField::Guid] {
        uses.push(owner.observe_header(field, SourceUseRole::Selector)?);
    }
    let evidence = VariantEvidence::from_uses(&input.origins().uses, &uses, actual.is_ok());
    Ok(if actual.is_ok() {
        Candidate::Admitted(evidence)
    } else {
        Candidate::Unknown(evidence)
    })
}

pub(super) fn unreadable_body(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    header: &VariantEvidence,
) -> Result<VariantEvidence, CatalogError> {
    let attempt = input.origins().dataforge.at(
        original_input::record_body_location(record.index()),
        SourceUseRole::Attempted,
    )?;
    let sources = &input.origins().uses;
    let body = VariantEvidence::from_uses(sources, &[attempt], false);
    VariantEvidence::join(sources, [header, &body], false)
}

pub(super) fn definition_census(
    input: &OriginalCatalogInput<'_, '_>,
    inspected: usize,
) -> Result<FieldOrigin, CatalogError> {
    if inspected != input.forge().record_count() {
        return Err(ExportAccountingError::MissingFieldInspection.into());
    }
    let end = u64::try_from(inspected).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let sources = &input.origins().uses;
    let census = input.origins().dataforge.at(
        SourceLocation::Subresource {
            namespace: "dataforgeShipVehicleDefinitionSurvey".to_owned(),
            indices: vec![0, end],
            field: "Components.VehicleComponentParams.vehicleDefinition".to_owned(),
        },
        SourceUseRole::Selector,
    )?;
    sources
        .origin_with_origins(
            [
                &original_input::record_inventory(input)?,
                &sources.origin(&[census], true),
            ],
            true,
        )
        .map_err(Into::into)
}

pub(super) fn read<'source>(
    input: &'source OriginalCatalogInput<'source, 'source>,
    files: &ArchiveFiles<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    wanted: &HashSet<ShipDefinitionKey>,
    cancel: Option<&CancelToken>,
) -> Result<Membership<'source>, CatalogError> {
    let owner = RecordObservation::capture(input, record)?;
    let operands = read_variant_operands(tree, input, &owner, cancel)?;
    let definition = match DefinitionMembership::of(&operands.definition) {
        DefinitionMembership::Comparable(key) if wanted.contains(&key) => key,
        DefinitionMembership::Unavailable => {
            return Ok(Membership::Unknown(VariantEvidence::from_origin(
                operands.definition.origin().clone(),
            )));
        }
        _ => return Ok(Membership::Unrelated),
    };
    let ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        tree,
        &owner,
        Some(Kind::Loadouts),
        ExportSourceRole::Dependency,
        cancel,
    )?;
    let capture = Arc::new(Capture::for_input(input, &ledger, cancel, Kind::Loadouts));
    let configured = operands
        .definition
        .map_ref(|definition| !definition.trim().is_empty());
    let observed = structure_binding::read(
        &capture,
        files,
        record,
        tree,
        structure_binding::Operands {
            record_index: record.index(),
            vehicle: &configured,
            modification: &operands.modification,
        },
    )?;
    let (digest, rank) = if let structure_binding::Decision::Digest(digest) = observed.decision {
        let rank = rank_candidate(
            input,
            record,
            tree,
            operands.vehicle,
            &operands.playability,
            cancel,
        )?;
        (Some(digest), Some(rank))
    } else {
        (None, None)
    };
    Ok(Membership::Reached {
        definition,
        structural: StructuralCandidate {
            evidence: observed.evidence,
            digest,
            rank,
        },
    })
}

fn rank_candidate<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    tree: &RecordTree<'source>,
    vehicle: Option<&StructValue<'source>>,
    owner: &OwnerProof,
    cancel: Option<&CancelToken>,
) -> Result<Ranking<'source>, CatalogError> {
    let (evidence, _) = Evidence::read_for_ships(std::iter::once(tree), input, cancel)?;
    let playability = owner.decide(tree, vehicle, &evidence, input)?;
    let class = source_header::class_name(tree, input.origins())?;
    let class_name = class
        .value()
        .copied()
        .ok_or(ExportAccountingError::MissingFieldInspection)?;
    let complete = matches!(
        playability.state(),
        SourceValueState::Present | SourceValueState::Absent
    );
    let rank = VariantRank {
        member: VariantMember {
            guid: CatalogGuid(record.guid()),
            record_index: record.index(),
        },
        class_name,
        playability: playability.value().copied(),
    };
    let evidence = input
        .origins()
        .uses
        .origin_with_origins([class.origin(), playability.origin()], complete)?;
    Ok(Ranking {
        rank,
        evidence: VariantEvidence::from_origin(evidence),
    })
}
