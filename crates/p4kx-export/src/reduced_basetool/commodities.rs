// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::commodities::{
    CommodityDefinition, CommodityPresentation, CommoditySnapshot,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;

const DEFINITIONS: &str = "definitions.commodities";
const PRESENTATIONS: &str = "projections.commodityPresentation";

pub(super) struct Commodities {
    pub(super) definitions: StagedCatalogTable,
    pub(super) presentations: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Commodities, ReducedError> {
    parsed.with_commodities(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(&snapshot, scratch, cancel, texts)
    })?
}

fn stage(
    snapshot: &CommoditySnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Commodities, ReducedError> {
    let availability = snapshot.availability;
    completeness::inventory(DEFINITIONS, availability, &snapshot.coverage)?;
    completeness::all_fields(PRESENTATIONS, availability, &snapshot.dependency_coverage)?;
    completeness::rows(
        PRESENTATIONS,
        snapshot.definitions.len(),
        snapshot.presentations.len(),
    )?;

    let mut definitions = StagedCatalogRows::new(scratch, cancel);
    let mut presentations = StagedCatalogRows::new(scratch, cancel);
    let mut definitions_by_record = RecordTargets::default();
    let mut text_refs = TextRefs::default();
    for (definition, presentation) in snapshot.definitions.iter().zip(&snapshot.presentations) {
        let header = definition.source();
        definitions_by_record.insert(DEFINITIONS, header)?;
        let row = Presentation::read(definition, presentation)?;
        text_refs.name(row.name.value.flatten());
        text_refs.game_text(row.description.value.flatten());
        text_refs.game_text(row.display_type.value.flatten());
        definitions.append(&RecordIdentity::from(header))?;
        presentations.append(&row)?;
    }
    let definitions = definitions.finish(availability)?;
    let presentations = presentations.finish(availability)?;
    completeness::rows(DEFINITIONS, snapshot.definitions.len(), definitions.len())?;
    completeness::rows(PRESENTATIONS, definitions.len(), presentations.len())?;
    texts.add(
        text_refs,
        &snapshot.public_names,
        &snapshot.game_texts,
        cancel,
    )?;
    Ok(Commodities {
        definitions,
        presentations,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Presentation<'source> {
    commodity_guid: Cell<&'source CatalogGuid>,
    record_index: Cell<&'source u32>,
    name: Cell<Option<&'source PublicNameRef>>,
    description: Cell<Option<&'source GameTextRef>>,
    display_type: Cell<Option<&'source GameTextRef>>,
    density_grams_per_cubic_centimetre: Cell<Option<f32>>,
    type_guid: Cell<Option<&'source CatalogGuid>>,
    subtype_guid: Cell<Option<&'source CatalogGuid>>,
    refined_subtype_guid: Cell<Option<&'source CatalogGuid>>,
}

impl<'source> Presentation<'source> {
    fn read(
        definition: &'source CommodityDefinition<'_>,
        source: &'source CommodityPresentation,
    ) -> Result<Self, ReducedError> {
        let header = definition.source();
        let commodity_guid = completeness::cell(PRESENTATIONS, source.commodity_guid())?;
        if commodity_guid.value.is_none() || commodity_guid.value != header.guid.value() {
            return Err(ReducedError::IncompleteScope(PRESENTATIONS));
        }
        Ok(Self {
            commodity_guid,
            record_index: completeness::cell(PRESENTATIONS, header.record_index())?,
            name: optional(source.name())?,
            description: optional(source.description())?,
            display_type: optional(source.display_type())?,
            density_grams_per_cubic_centimetre: completeness::cell(
                PRESENTATIONS,
                source.density_grams_per_cubic_centimetre(),
            )?
            .map(|value| *value),
            type_guid: optional(source.type_guid())?,
            subtype_guid: optional(source.subtype_guid())?,
            refined_subtype_guid: optional(source.refined_subtype_guid())?,
        })
    }
}

fn optional<T>(source: &SourcedValue<Option<T>>) -> Result<Cell<Option<&T>>, ReducedError> {
    Ok(completeness::cell(PRESENTATIONS, source)?.map(Option::as_ref))
}
