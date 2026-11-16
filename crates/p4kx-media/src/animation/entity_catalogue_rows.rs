// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityClipCatalogue, EntityNode};
use p4kx_extraction::{InputScope, Record, SourceUseRole};

use super::super::entity_loadout_evidence::EntityLoadoutEvidence;

use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryLookup, MediaError, MediaWriter};

#[path = "entity_catalogue_wire.rs"]
mod wire;

use wire::{Payload, Scope};

pub(crate) use wire::{CATALOGUE, CatalogueState, bind_schema};

pub(crate) fn loadout_issue(
    root: &EntityNode,
    writer: &MediaWriter<'_>,
) -> Result<Option<String>, MediaError> {
    writer.checkpoint()?;
    let mut result = Ok(root.issues().first().map(ToString::to_string));
    root.visit_ports(|_, route| {
        if !matches!(result, Ok(None)) {
            return;
        }
        result = writer
            .checkpoint()
            .map(|()| match route.last().map(|port| port.attachment()) {
                Some(Attachment::Entity(child)) => child.issues().first().map(ToString::to_string),
                Some(Attachment::Unresolved(child)) => Some(child.error().to_string()),
                _ => None,
            });
    });
    result
}

pub(super) fn unavailable(
    selector: &str,
    state: CatalogueState,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    Scope::new(writer).write(
        CATALOGUE,
        vec![],
        &Payload::Catalogue(wire::Catalogue {
            selector,
            state,
            owner: None,
        }),
        state.issue().as_slice(),
        writer,
    )
}

pub(super) struct EntitySources<'row, 'dir, 'forge> {
    pub(super) owner: &'row Record<'forge>,
    pub(super) input: &'row MediaOriginInput,
    pub(super) lookup: &'row DirectoryLookup<'dir>,
    pub(super) loadout: &'row EntityLoadoutEvidence<'forge>,
}

pub(super) fn write(
    selector: &str,
    state: CatalogueState,
    EntitySources {
        owner,
        input,
        lookup,
        loadout,
    }: EntitySources<'_, '_, '_>,
    catalogue: &EntityClipCatalogue,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut scope = Scope::new(writer);
    let owner_proof = db(input, owner.index(), "guid", writer)?;
    scope.at("owner/guid", owner_proof, writer)?;
    scope.at(
        "owner/record",
        db(input, owner.index(), "recordIndex", writer)?,
        writer,
    )?;
    let observed = loadout.issues(input, writer)?;
    let mut issues: Vec<_> = state.issue().into_iter().collect();
    if !issues.is_empty() {
        scope.health(0, db(input, owner.index(), "loadout", writer)?, writer)?;
    }
    for attempt in observed {
        scope.health(issues.len(), attempt.origin, writer)?;
        issues.push(attempt.issue);
    }
    scope.write(
        CATALOGUE,
        vec![],
        &Payload::Catalogue(wire::Catalogue {
            selector,
            state,
            owner: Some(wire::Owner::new(owner.guid().to_string(), owner.index())),
        }),
        issues.as_slice(),
        writer,
    )?;
    let rows = wire::Rows { lookup, input };
    rows.rigs(catalogue, loadout, writer)?;
    rows.sources(catalogue, writer)
}

pub(super) fn db(
    input: &MediaOriginInput,
    index: u32,
    member: &str,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("dataforge-record", &[u64::from(index)], member),
        SourceUseRole::Payload,
        writer,
    )?;
    Ok(capture.finish())
}
