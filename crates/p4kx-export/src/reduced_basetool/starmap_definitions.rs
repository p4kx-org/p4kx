// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_queries::starmap::{
    StarmapAmenityDefinition, StarmapDefinitionFamily, StarmapDefinitionRows,
    StarmapDefinitionSelection, StarmapDefinitionSnapshot, StarmapTypeDefinition,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries, PublicNameRef};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const TYPES: &str = "definitions.starmapTypes";
pub(super) const AMENITIES: &str = "definitions.starmapAmenities";

pub(super) struct DefinitionFamily {
    pub(super) table: StagedCatalogTable,
    pub(super) targets: RecordTargets,
}

pub(super) fn collect_types(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<DefinitionFamily, ReducedError> {
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Types,
        CatalogRecordSelection::All,
    );
    parsed.with_starmap_definitions(selection, cancel, |snapshot| {
        let StarmapDefinitionRows::Types(rows) = &snapshot.rows else {
            return Err(ReducedError::IncompleteScope(TYPES));
        };
        let mut references = TextRefs::default();
        let prepared = stage(TYPES, &snapshot, rows, scratch, cancel, |source| {
            Ok((source.header(), ObjectType::read(source, &mut references)?))
        })?;
        texts.add(
            references,
            &snapshot.public_names,
            &snapshot.game_texts,
            cancel,
        )?;
        Ok(prepared)
    })?
}

pub(super) fn collect_amenities(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<DefinitionFamily, ReducedError> {
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Amenities,
        CatalogRecordSelection::All,
    );
    parsed.with_starmap_definitions(selection, cancel, |snapshot| {
        let StarmapDefinitionRows::Amenities(rows) = &snapshot.rows else {
            return Err(ReducedError::IncompleteScope(AMENITIES));
        };
        stage(AMENITIES, &snapshot, rows, scratch, cancel, |source| {
            Ok((source.header(), Amenity::read(source)?))
        })
    })?
}

fn stage<'read, Original, Row: Serialize>(
    path: &'static str,
    snapshot: &StarmapDefinitionSnapshot<'_>,
    rows: &'read [Original],
    scratch: &Path,
    cancel: Option<&CancelToken>,
    mut prepare: impl FnMut(
        &'read Original,
    ) -> Result<(&'read CanonicalRecordHeader<'read>, Row), ReducedError>,
) -> Result<DefinitionFamily, ReducedError> {
    completeness::inventory(path, snapshot.availability, &snapshot.coverage)?;
    let mut targets = RecordTargets::default();
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for original in rows {
        let (header, row) = prepare(original)?;
        completeness::value(path, &header.guid())?;
        completeness::value(path, header.record_index())?;
        completeness::value(path, header.class_name())?;
        completeness::value(path, header.record_path())?;
        targets.insert(path, header)?;
        output.append(&row)?;
    }
    let table = output.finish(snapshot.availability)?;
    completeness::rows(path, rows.len(), table.len())?;
    Ok(DefinitionFamily { table, targets })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectType<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read str>,
    classification: Cell<&'read PublicNameRef>,
    valid_quantum_travel_destination: Cell<&'read bool>,
    ignore_as_routing_obstacle: Cell<&'read bool>,
}

impl<'read> ObjectType<'read> {
    fn read(
        source: &'read StarmapTypeDefinition<'_>,
        texts: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let classification = completeness::cell(TYPES, source.classification())?;
        texts.name(classification.value);
        Ok(Self {
            identity: source.header().into(),
            name: completeness::cell(TYPES, source.name())?.map(|name| *name),
            classification,
            valid_quantum_travel_destination: completeness::cell(
                TYPES,
                source.valid_quantum_travel_destination(),
            )?,
            ignore_as_routing_obstacle: completeness::cell(
                TYPES,
                source.ignore_as_routing_obstacle(),
            )?,
        })
    }
}

#[derive(Serialize)]
struct Amenity<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read str>,
}

impl<'read> Amenity<'read> {
    fn read(source: &'read StarmapAmenityDefinition<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            identity: source.header().into(),
            name: completeness::cell(AMENITIES, source.name())?.map(|name| *name),
        })
    }
}
