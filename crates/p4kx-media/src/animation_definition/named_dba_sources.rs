// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

#[path = "named_dba_inventory.rs"]
mod inventory;

use p4kx_extraction::cryxml::{SourceAttribute, SourceElement};
use p4kx_extraction::{Entry, InputScope, SourceUseRole};

use self::inventory::Inventory;
pub(super) use self::inventory::ObservedMatch;
use super::super::links;
use super::super::origins;
use super::super::payload::LinkKind;
use super::super::projection::{DefinitionProjection, Failure};
use super::DatabaseState;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::xml_source_projection::{declaration_position, element_position};
use crate::{
    MediaError, MediaKind, MediaWriter, OriginalIdentifier, ReferenceResolution, Subresource,
    required_input,
};

pub(super) struct Bank {
    pub(super) path: Option<String>,
    pub(super) path_evidence: OriginContributions,
    pub(super) evidence: OriginContributions,
    pub(super) state: BankState,
}

pub(super) enum BankState {
    Available(Arc<Inventory>),
    Other(DatabaseState),
}

impl BankState {
    pub(super) fn state(&self) -> DatabaseState {
        match self {
            Self::Available(bank) if bank.complete => DatabaseState::Enumerated,
            Self::Available(_) => DatabaseState::Incomplete,
            Self::Other(state) => *state,
        }
    }

    pub(super) fn matches(&self, path: &str) -> impl Iterator<Item = ObservedMatch<'_>> {
        match self {
            Self::Available(bank) => Some(bank.matches(path)),
            Self::Other(_) => None,
        }
        .into_iter()
        .flatten()
    }
}

pub(super) fn read(
    list: SourceElement<'_>,
    projection: &DefinitionProjection<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<Bank>, Failure> {
    let budget = &projection.context.directory.input_state()?.budget;
    let mut cache = HashMap::new();
    let mut banks = Vec::new();
    for element in list.elements() {
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        if element.namespace().is_some() || !element.name().eq_ignore_ascii_case("Animation") {
            continue;
        }
        if let Some(bank) = bank(element, projection, &mut cache, writer)? {
            banks
                .try_reserve(1)
                .map_err(|error| allocation(projection, error))?;
            banks.push(bank);
        }
    }
    Ok(banks)
}

fn bank(
    element: SourceElement<'_>,
    projection: &DefinitionProjection<'_, '_>,
    cache: &mut HashMap<usize, Arc<Inventory>>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<Bank>, Failure> {
    let budget = &projection.context.directory.input_state()?.budget;
    let mut name = None;
    let mut path = None;
    let mut duplicate = false;
    let mut tracks_database = false;
    for attribute in element
        .attributes()
        .filter(|attribute| attribute.namespace().is_none())
    {
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        if attribute.name().eq_ignore_ascii_case("name") {
            tracks_database |= attribute.value().eq_ignore_ascii_case("$TracksDatabase");
            duplicate |= name.replace(attribute).is_some();
        } else if attribute.name().eq_ignore_ascii_case("path") {
            duplicate |= path.replace(attribute).is_some();
        }
    }
    if !tracks_database {
        return Ok(None);
    }
    let mut evidence = writer.capture_origins();
    if duplicate || path.is_none() {
        evidence.include(
            &origins::position(
                projection.input,
                element_position(&element.position())?,
                SourceUseRole::Selector,
                writer,
            )?,
            writer,
        )?;
        let proof = evidence.finish();
        return Ok(Some(Bank {
            path: None,
            path_evidence: proof.clone(),
            evidence: proof,
            state: BankState::Other(DatabaseState::InvalidIdentifier),
        }));
    }
    let (Some(name), Some(path)) = (name, path) else {
        return Ok(None);
    };
    let path_evidence = attribute_origin(projection, &path, writer)?;
    evidence.include(&attribute_origin(projection, &name, writer)?, writer)?;
    evidence.include(&path_evidence, writer)?;
    let path = path.value();
    let state = select(path, projection, cache, &mut evidence, writer)?;
    Ok(Some(Bank {
        path: Some(path.to_owned()),
        path_evidence,
        evidence: evidence.finish(),
        state,
    }))
}

fn attribute_origin(
    projection: &DefinitionProjection<'_, '_>,
    attribute: &SourceAttribute<'_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, Failure> {
    let position = declaration_position(attribute.position())?;
    Ok(origins::position(
        projection.input,
        position,
        SourceUseRole::Selector,
        writer,
    )?)
}

fn select(
    path: &str,
    projection: &DefinitionProjection<'_, '_>,
    cache: &mut HashMap<usize, Arc<Inventory>>,
    evidence: &mut crate::origin_capture::OriginCapture,
    writer: &mut MediaWriter<'_>,
) -> Result<BankState, Failure> {
    let Some(archive_path) = links::target_path(LinkKind::TracksDatabase, path) else {
        return Ok(BankState::Other(DatabaseState::InvalidIdentifier));
    };
    let directory = projection.context.directory;
    let observed = directory.lookup_input_observed(
        OriginalIdentifier::new_with_path(path),
        &archive_path,
        Some(MediaKind::Animation),
        writer,
    )?;
    evidence.include(&observed.inventory, writer)?;
    for candidate in observed
        .evidence
        .retained
        .iter()
        .chain(observed.evidence.discarded.iter())
    {
        evidence.add(
            candidate,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[], "reference-candidate"),
            SourceUseRole::Selector,
            writer,
        )?;
    }
    let resolution = observed.lookup.reference.resolution();
    let ReferenceResolution::Resolved { .. } = resolution else {
        let state = match resolution {
            ReferenceResolution::Missing => DatabaseState::Missing,
            ReferenceResolution::Ambiguous { .. } => DatabaseState::Ambiguous,
            ReferenceResolution::NotSelected { .. } => DatabaseState::NotSelected,
            _ => DatabaseState::Unavailable,
        };
        return Ok(BankState::Other(state));
    };
    let input = observed.lookup.input.ok_or_else(|| {
        MediaError::Contract("resolved animation bank lost its observed physical Entry".into())
    })?;
    if directory
        .index
        .archive
        .entries()
        .get(input.position)
        .is_some_and(Entry::is_encrypted)
        && !directory.index.archive.has_key()
    {
        return Ok(BankState::Other(DatabaseState::Unavailable));
    }
    let bank = if let Some(bank) = cache.get(&input.position) {
        Arc::clone(bank)
    } else {
        let bank = Arc::new(read_bank(input.position, projection, writer)?);
        cache
            .try_reserve(1)
            .map_err(|error| allocation(projection, error))?;
        cache.insert(input.position, Arc::clone(&bank));
        bank
    };
    evidence.include(&bank.coverage, writer)?;
    Ok(BankState::Available(bank))
}

fn read_bank(
    position: usize,
    projection: &DefinitionProjection<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Inventory, MediaError> {
    let directory = projection.context.directory;
    let entry = directory
        .index
        .archive
        .entries()
        .get(position)
        .ok_or_else(|| MediaError::Contract("observed animation bank Entry is absent".into()))?;
    let use_at = Subresource::new(super::super::payload::NAMED_DBA_BINDING, Vec::new());
    required_input::with_reference_input(directory, entry, &use_at, writer, |input, writer| {
        inventory::read(input, projection, writer)
    })
}

fn allocation(
    projection: &DefinitionProjection<'_, '_>,
    error: std::collections::TryReserveError,
) -> MediaError {
    MediaError::Io {
        path: projection.context.entry.path().into(),
        source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, error),
    }
}
