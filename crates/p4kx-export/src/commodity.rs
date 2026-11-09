// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUse, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::commodity_authoring_contract::CommodityAuthoring;
use crate::commodity_authoring_read;
use crate::commodity_backfill::BackfillEvidence;
use crate::commodity_contract::{
    Commodity, CommodityDefinition, CommodityPresentation, CommoditySelection,
};
use crate::commodity_inputs::CommodityInputs;
use crate::commodity_presentation::CommodityPresentationContext;
use crate::commodity_resource::CommodityResourceSelection;
use crate::commodity_source;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::resource_group::Groups;
use crate::source_fields::FieldError;

pub(crate) struct CommodityRead<'source> {
    pub(crate) value: Commodity<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) resource_notes: Vec<FieldError>,
}

struct PreparedEntity<'source> {
    entity: SourcedValue<CommodityAuthoring<'source>>,
    links: Vec<SourceUse>,
    presentation: CommodityPresentation,
    crafter_inputs: Vec<SourceUse>,
    crafter_complete: bool,
    notes: Vec<FieldError>,
    resource_notes: Vec<FieldError>,
}

/// Prepares shared authored facts and the unchanged Entity Commodity presentation.
pub(crate) fn commodity<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    resource: Option<&RecordTree<'source>>,
    groups: &Groups,
    selected: &CommodityResourceSelection,
) -> Result<CommodityRead<'source>, CatalogError> {
    let prepared = prepare_entity(tree, projection, resource, groups, selected)?;
    finish_entity(tree, selected, projection.origins(), prepared)
}

fn prepare_entity<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    resource: Option<&RecordTree<'source>>,
    groups: &Groups,
    selected: &CommodityResourceSelection,
) -> Result<PreparedEntity<'source>, CatalogError> {
    let origins = projection.origins();
    let mut notes = Vec::new();
    let authored = commodity_authoring_read::entity(tree, projection, &mut notes)?;
    let links = commodity_source::link_inputs(tree, selected, origins)?;
    let resource_inputs = commodity_source::resource_inputs(selected, origins, &links)?;
    let complete = resource_complete(resource, selected);
    resource_attempt(selected, complete, origins)?;
    let context =
        CommodityPresentationContext::new(projection, resource, &links, &resource_inputs, complete);
    let mut resource_notes = Vec::new();
    let texts = context.texts(
        Some(&authored.own_name),
        Some(&authored.own_description),
        Some(&authored.display_type),
        &mut resource_notes,
    )?;
    let subtype = commodity_source::subtype(tree, selected, origins, &links)?;
    let presentation = context.finish(tree, texts, subtype, groups)?;
    let entity = origins.uses.value(
        SourceValue::Present(authored.value),
        &authored.inputs,
        authored.complete,
    );
    Ok(PreparedEntity {
        entity,
        links,
        presentation,
        crafter_inputs: authored.crafter_inputs,
        crafter_complete: authored.crafter_complete,
        notes,
        resource_notes,
    })
}

fn resource_attempt(
    selected: &CommodityResourceSelection,
    complete: bool,
    origins: &CatalogOrigins,
) -> Result<(), CatalogError> {
    if !complete
        && let Some(index) = selected
            .link
            .as_ref()
            .and_then(|link| link.reference.target_record_index)
    {
        let mut attempted = CommodityInputs::new(origins);
        attempted.at(index, "$record", SourceUseRole::Attempted)?;
    }
    Ok(())
}

fn finish_entity<'source>(
    tree: &RecordTree<'source>,
    selected: &CommodityResourceSelection,
    origins: &CatalogOrigins,
    prepared: PreparedEntity<'source>,
) -> Result<CommodityRead<'source>, CatalogError> {
    let selection = entity_selection(tree, origins)?;
    let definition =
        commodity_source::definition(tree, prepared.entity, selection, Vec::new(), origins)?;
    let crafter = representation_headers(
        tree,
        prepared.crafter_inputs,
        prepared.crafter_complete,
        origins,
    )?;

    let value = Commodity {
        guid: CatalogGuid(tree.guid()),
        record_path: tree.record_path(),
        subtype_guid: selected.link.as_ref().map(|link| link.reference.guid),
        definition,
        presentation: prepared.presentation,
        relationship_inputs: prepared.links,
        crafter_inputs: crafter.0,
        crafter_complete: crafter.1,
    };
    Ok(CommodityRead {
        value,
        notes: prepared.notes,
        resource_notes: prepared.resource_notes,
    })
}
fn resource_complete(
    resource: Option<&RecordTree<'_>>,
    selected: &CommodityResourceSelection,
) -> bool {
    resource.is_some()
        || !selected.link.as_ref().is_some_and(|link| {
            link.reference.target_type.as_deref() == Some("ResourceType")
                && link.reference.target_record_index.is_some()
        })
}

fn entity_selection(
    tree: &RecordTree<'_>,
    origins: &CatalogOrigins,
) -> Result<SourcedValue<CommoditySelection>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    for field in [
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
    ] {
        inputs.header_input(tree.record_index(), field, SourceUseRole::Selector)?;
    }
    Ok(inputs.value(CommoditySelection::Entity))
}

fn representation_headers(
    tree: &RecordTree<'_>,
    sources: Vec<SourceUse>,
    complete: bool,
    origins: &CatalogOrigins,
) -> Result<(Vec<SourceUse>, bool), CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    for field in [
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
    ] {
        inputs.header_input(tree.record_index(), field, SourceUseRole::Selector)?;
    }
    inputs.extend(sources, complete);
    Ok(inputs.into_evidence())
}

/// Prepares the original resource-only Commodity identity without inventing an Entity input.
pub(crate) fn resource_commodity<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    groups: &Groups,
    evidence: BackfillEvidence<'_>,
) -> Result<CommodityRead<'source>, CatalogError> {
    let origins = projection.origins();
    let context = origins.record(tree.record_index());
    let mut links = CommodityInputs::new(origins);
    links.header(
        tree.record_index(),
        RecordHeaderField::Guid,
        SourceUseRole::Selector,
    )?;
    let (links, _) = links.into_evidence();
    let presentation_context =
        CommodityPresentationContext::new(projection, Some(tree), &links, &links, true);
    let mut resource_notes = Vec::new();
    let texts = presentation_context.texts(None, None, None, &mut resource_notes)?;
    let subtype = context.header_value(
        RecordHeaderField::Guid,
        SourceValue::Present(Some(CatalogGuid(tree.guid()))),
    )?;
    let presentation = presentation_context.finish(tree, texts, subtype, groups)?;
    let selection = crate::commodity_backfill::selection(&evidence);
    let entity = context.header_selector(RecordHeaderField::SourceType, SourceValue::Absent)?;
    let definition =
        commodity_source::definition(tree, entity, selection, evidence.diagnostics, origins)?;
    Ok(CommodityRead {
        value: Commodity {
            guid: CatalogGuid(tree.guid()),
            record_path: tree.record_path(),
            subtype_guid: Some(CatalogGuid(tree.guid())),
            definition,
            presentation,
            relationship_inputs: Vec::new(),
            crafter_inputs: Vec::new(),
            crafter_complete: true,
        },
        notes: Vec::new(),
        resource_notes,
    })
}

/// Separates already-prepared rows without re-reading inputs or projecting a second view.
pub(crate) fn split(
    rows: Vec<Commodity<'_>>,
) -> (Vec<CommodityDefinition<'_>>, Vec<CommodityPresentation>) {
    rows.into_iter()
        .map(|row| (row.definition, row.presentation))
        .unzip()
}
