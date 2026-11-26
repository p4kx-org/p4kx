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
use super::resource_density::{self, Density};
use super::resource_groups::{self, ResourceGroups};
use super::resource_properties::{self, Property};
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::Kind;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries, ResourceTypeSnapshot};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::PublicNameRef;
use crate::guid_text::CatalogGuid;
use crate::resource_type_contract::ResourceDefinition;

const PATH: &str = "definitions.resourceTypes";

pub(super) struct Resources {
    pub(super) definitions: StagedCatalogTable,
    pub(super) groups: ResourceGroups,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Resources, ReducedError> {
    let (definitions, targets) =
        parsed.with_resource_types(CatalogRecordSelection::All, cancel, |snapshot| {
            stage(&snapshot, scratch, cancel, texts)
        })??;
    let groups = resource_groups::collect(parsed, scratch, cancel, &targets, texts)?;
    Ok(Resources {
        definitions,
        groups,
    })
}

fn stage(
    snapshot: &ResourceTypeSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(PATH, availability, snapshot.coverage())?;
    completeness::all_fields(PATH, availability, snapshot.dependency_coverage())?;
    let mut targets = RecordTargets::default();
    for source in snapshot.rows() {
        targets.insert(PATH, source.source())?;
    }
    let mut rows = StagedCatalogRows::new(scratch, cancel);
    let mut text_refs = TextRefs::default();
    for source in snapshot.rows() {
        text_refs.name(source.name());
        rows.append(&Definition::read(source, &targets)?)?;
    }
    let rows = rows.finish(availability)?;
    completeness::rows(PATH, snapshot.rows().len(), rows.len())?;
    texts.add(
        text_refs,
        snapshot.public_names(),
        snapshot.game_texts(),
        cancel,
    )?;
    Ok((rows, targets))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Definition<'source> {
    #[serde(flatten)]
    identity: RecordIdentity<'source>,
    name: Cell<&'source PublicNameRef>,
    group_guids: Cell<Vec<Cell<&'source CatalogGuid>>>,
    refined_version: Cell<RecordLink<'source>>,
    density: Cell<Density<'source>>,
    default_cargo_containers: Cell<Vec<Cell<CargoSlot<'source>>>>,
    properties: Cell<Vec<Cell<Property<'source>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CargoSlot<'source> {
    slot: Cell<&'source str>,
    entity_class: Cell<RecordLink<'source>>,
}

impl<'source> Definition<'source> {
    fn read(
        source: &'source ResourceDefinition<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let refined_version = completeness::cell(PATH, source.refined_version())?
            .try_map(|link| RecordLink::read(PATH, link, Kind::ResourceTypes, targets))?;
        let default_cargo_containers =
            completeness::list(PATH, source.default_cargo_containers(), |slot| {
                Ok(CargoSlot {
                    slot: completeness::cell(PATH, slot.slot())?.map(|name| *name),
                    entity_class: completeness::cell(PATH, slot.entity_class())?
                        .map(RecordLink::from),
                })
            })?;
        Ok(Self {
            identity: source.source().into(),
            name: completeness::cell(PATH, source.name())?,
            group_guids: groups(source.group_guids())?,
            refined_version,
            density: resource_density::read(PATH, source.density())?,
            default_cargo_containers,
            properties: resource_properties::read(PATH, source.properties())?,
        })
    }
}

fn groups(
    source: &SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
) -> Result<Cell<Vec<Cell<&CatalogGuid>>>, ReducedError> {
    completeness::cell(PATH, source)?.try_map(|groups| {
        groups
            .iter()
            .map(|group| completeness::cell(PATH, group))
            .collect()
    })
}
