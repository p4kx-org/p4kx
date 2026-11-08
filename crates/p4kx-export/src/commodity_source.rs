// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUse, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::commodity_authoring_contract::CommodityAuthoring;
use crate::commodity_contract::{
    CommodityDefinition, CommoditySelection, CommoditySelectionDiagnostic,
};
use crate::commodity_inputs::CommodityInputs;
use crate::commodity_resource::CommodityResourceSelection;
use crate::commodity_text_input::CommodityTextInput;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

/// Captures only the original first-resource selectors for one Commodity choice.
pub(crate) fn link_inputs(
    tree: &RecordTree<'_>,
    selected: &CommodityResourceSelection,
    origins: &CatalogOrigins,
) -> Result<Vec<SourceUse>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    for field in &selected.selectors {
        inputs.at(tree.record_index(), field, SourceUseRole::Selector)?;
    }
    Ok(inputs.into_evidence().0)
}

/// Retains the target header actually consulted before reading a resource dependency body.
pub(crate) fn resource_inputs(
    selected: &CommodityResourceSelection,
    origins: &CatalogOrigins,
    links: &[SourceUse],
) -> Result<Vec<SourceUse>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    inputs.extend(links.iter().cloned(), true);
    if let Some(index) = selected
        .link
        .as_ref()
        .and_then(|link| link.reference.target_record_index)
    {
        inputs.at(index, "$type", SourceUseRole::Selector)?;
    }
    Ok(inputs.into_evidence().0)
}
/// Binds a raw resource Locale candidate at its actual physical owner and field.
pub(crate) fn resource_text<'source>(
    tree: &RecordTree<'source>,
    field: &str,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CommodityTextInput<'source>, CatalogError> {
    let context = projection.origins().record(tree.record_index());
    let key = CatalogFields::new(tree.root(), "", &context).locale(field, notes)?;
    let mut input = CommodityTextInput::new(key, tree.record_index(), field.to_owned());
    input.selectors.push("$type".to_owned());
    Ok(input)
}

/// Captures the source header without attaching the row-presence derivation to unrelated facts.
pub(crate) fn definition<'source>(
    tree: &RecordTree<'source>,
    entity: SourcedValue<CommodityAuthoring<'source>>,
    selection: SourcedValue<CommoditySelection>,
    selection_diagnostics: Vec<CommoditySelectionDiagnostic>,
    origins: &CatalogOrigins,
) -> Result<CommodityDefinition<'source>, CatalogError> {
    let context = origins.record(tree.record_index());
    Ok(CommodityDefinition {
        guid: context.value("$guid", SourceValue::Present(CatalogGuid(tree.guid())))?,
        class_name: crate::source_header::class_name(tree, origins)?,
        record_path: context.value("$recordPath", SourceValue::Present(tree.record_path()))?,
        record_index: context
            .selector("$recordIndex", SourceValue::Present(tree.record_index()))?,
        source_type: context.selector("$type", SourceValue::Present(tree.type_name()))?,
        selection,
        selection_diagnostics,
        entity,
    })
}

/// Binds the existing first-resource GUID independently of its target's body availability.
pub(crate) fn subtype(
    tree: &RecordTree<'_>,
    selected: &CommodityResourceSelection,
    origins: &CatalogOrigins,
    links: &[SourceUse],
) -> Result<SourcedValue<Option<CatalogGuid>>, CatalogError> {
    let mut inputs = CommodityInputs::new(origins);
    inputs.extend(links.iter().cloned(), true);
    if let Some(link) = &selected.link {
        inputs.at(tree.record_index(), &link.field, SourceUseRole::Payload)?;
    }
    Ok(inputs.value(selected.link.as_ref().map(|link| link.reference.guid)))
}
