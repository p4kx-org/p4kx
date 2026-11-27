// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::Kind;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries, ResourceGroupSnapshot};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::PublicNameRef;
use crate::guid_text::CatalogGuid;

pub(super) const DEFINITIONS: &str = "definitions.resourceGroups";
pub(super) const MEMBERS: &str = "declarations.resourceGroupMembers";
pub(super) const PARENTS: &str = "projections.resourceGroupParents";

pub(super) struct ResourceGroups {
    pub(super) definitions: StagedCatalogTable,
    pub(super) members: StagedCatalogTable,
    pub(super) parents: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    resources: &RecordTargets,
    texts: &mut SelectedTexts,
) -> Result<ResourceGroups, ReducedError> {
    let (definitions, members, groups) =
        parsed.with_resource_groups(CatalogRecordSelection::All, cancel, |snapshot| {
            stage_groups(&snapshot, scratch, cancel, resources, texts)
        })??;
    let parents =
        parsed.with_resource_group_parents(CatalogRecordSelection::All, cancel, |snapshot| {
            completeness::all_fields(PARENTS, snapshot.availability(), snapshot.coverage())?;
            completeness::all_fields(
                PARENTS,
                snapshot.availability(),
                snapshot.dependency_coverage(),
            )?;
            let mut table = StagedCatalogRows::new(scratch, cancel);
            for source in snapshot.rows() {
                let owner = Owner::read(PARENTS, source.guid(), source.record_index(), &groups)?;
                completeness::value(PARENTS, source.parent())?;
                let parent = Cell::from(source.parent()).try_map(|reference| {
                    reference
                        .as_ref()
                        .map(|reference| {
                            RecordLink::read(PARENTS, reference, Kind::ResourceGroups, &groups)
                        })
                        .transpose()
                })?;
                table.append(&Parent { owner, parent })?;
            }
            let table = table.finish(snapshot.availability())?;
            completeness::rows(PARENTS, definitions.len(), table.len())?;
            Ok::<_, ReducedError>(table)
        })??;
    Ok(ResourceGroups {
        definitions,
        members,
        parents,
    })
}

fn stage_groups(
    snapshot: &ResourceGroupSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    resources: &RecordTargets,
    texts: &mut SelectedTexts,
) -> Result<(StagedCatalogTable, StagedCatalogTable, RecordTargets), ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(DEFINITIONS, availability, snapshot.coverage())?;
    let mut references = TextRefs::default();
    let mut groups = RecordTargets::default();
    let mut definitions = StagedCatalogRows::new(scratch, cancel);
    for row in snapshot.definitions() {
        groups.insert(DEFINITIONS, row.header())?;
        completeness::value(DEFINITIONS, row.name())?;
        references.name(row.name().value());
        definitions.append(&Definition {
            identity: row.header().into(),
            name: row.name().into(),
        })?;
    }
    completeness::rows(
        MEMBERS,
        snapshot.definitions().len(),
        snapshot.members().len(),
    )?;
    let mut members = StagedCatalogRows::new(scratch, cancel);
    for (definition, row) in snapshot.definitions().iter().zip(snapshot.members()) {
        if definition.header().record_index().value() != row.record_index().value() {
            return Err(ReducedError::IncompleteScope(MEMBERS));
        }
        let owner = Owner::read(MEMBERS, row.guid(), row.record_index(), &groups)?;
        members.append(&Members {
            owner,
            groups: links(row.groups(), Kind::ResourceGroups, &groups)?,
            resources: links(row.resources(), Kind::ResourceTypes, resources)?,
        })?;
    }
    let definitions = definitions.finish(availability)?;
    let members = members.finish(availability)?;
    completeness::rows(DEFINITIONS, snapshot.definitions().len(), definitions.len())?;
    completeness::rows(MEMBERS, definitions.len(), members.len())?;
    texts.add(
        references,
        snapshot.public_names(),
        snapshot.game_texts(),
        cancel,
    )?;
    Ok((definitions, members, groups))
}

#[derive(Serialize)]
struct Definition<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read PublicNameRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Owner<'read> {
    guid: Cell<&'read CatalogGuid>,
    record_index: Cell<&'read u32>,
}

impl<'read> Owner<'read> {
    fn read(
        path: &'static str,
        guid: &'read SourcedValue<CatalogGuid>,
        index: &'read SourcedValue<u32>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let identity = guid.value().ok_or(ReducedError::IncompleteScope(path))?;
        let position = index.value().ok_or(ReducedError::IncompleteScope(path))?;
        targets.verify(path, *position, identity.get())?;
        Ok(Self {
            guid: guid.into(),
            record_index: index.into(),
        })
    }
}

#[derive(Serialize)]
struct Members<'read> {
    #[serde(flatten)]
    owner: Owner<'read>,
    groups: Cell<Vec<Cell<RecordLink<'read>>>>,
    resources: Cell<Vec<Cell<RecordLink<'read>>>>,
}

#[derive(Serialize)]
struct Parent<'read> {
    #[serde(flatten)]
    owner: Owner<'read>,
    parent: Cell<Option<RecordLink<'read>>>,
}

fn links<'read>(
    source: &'read SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
    target_kind: Kind,
    targets: &RecordTargets,
) -> Result<Cell<Vec<Cell<RecordLink<'read>>>>, ReducedError> {
    completeness::list(MEMBERS, source, |link| {
        RecordLink::read(MEMBERS, link, target_kind, targets)
    })
}
