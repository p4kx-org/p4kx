// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, SourceUses, SourceValue, SourceValueState, SourcedValue,
    TreeBuilder,
};

use super::super::authored_fields::Observed;
use super::super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

type AuthoredLink = SourcedValue<Box<CatalogRecordReference>>;

/// The actual source branch used to choose a harvestable entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum EntitySelection {
    /// Used the provider element's direct entity selector.
    Override,
    /// Used the consulted preset's entity selector.
    PresetFallback,
}

/// The direct original references and the physical records admitted by their selection.
pub(super) struct HarvestDependencySelection {
    pub(super) authored_preset: AuthoredLink,
    pub(super) authored_override: AuthoredLink,
    pub(super) fallback_entity: Option<AuthoredLink>,
    pub(super) preset_record_index: SourcedValue<u32>,
    pub(super) entity_record_index: SourcedValue<u32>,
    pub(super) entity_selection: SourcedValue<EntitySelection>,
}

pub(super) fn choose<'source>(
    owner: Observed<'_, '_, '_, 'source>,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    notes: &mut Vec<FieldError>,
) -> Result<HarvestDependencySelection, CatalogError> {
    input.check_forge(forge)?;
    let sources = &input.origins().uses;
    let authored_preset = link(&owner, "harvestable", notes)?;
    let authored_override = link(&owner, "harvestableEntityClass", notes)?;
    let preset_record_index = target(forge, sources, &authored_preset, "HarvestablePreset", &[])?;
    let (fallback_entity, entity_record_index, entity_selection) = if authored_override
        .value()
        .and_then(|reference| reference.target_record_index.value())
        .is_some()
    {
        let selected = target(
            forge,
            sources,
            &authored_override,
            "EntityClassDefinition",
            &[],
        )?;
        let route = sources.value_with_origins(
            SourceValue::Present(EntitySelection::Override),
            &[authored_override.origin()],
        )?;
        (None, selected, route)
    } else {
        fallback(
            forge,
            input,
            &authored_override,
            &preset_record_index,
            cancel,
        )?
    };
    Ok(HarvestDependencySelection {
        authored_preset,
        authored_override,
        fallback_entity,
        preset_record_index,
        entity_record_index,
        entity_selection,
    })
}

fn link(
    owner: &Observed<'_, '_, '_, '_>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<AuthoredLink, CatalogError> {
    Ok(owner
        .record_link(field, notes)?
        .map(|value| Box::new((*value).into_catalog())))
}

fn target(
    forge: &DataForge<'_>,
    sources: &SourceUses,
    authored: &AuthoredLink,
    family: &str,
    inherited: &[&FieldOrigin],
) -> Result<SourcedValue<u32>, CatalogError> {
    let mut origins = Vec::with_capacity(inherited.len() + 3);
    origins.extend_from_slice(inherited);
    origins.push(authored.origin());
    let state = match authored.value() {
        Some(reference) => {
            origins.extend([
                reference.target_record_index.origin(),
                reference.target_type.origin(),
            ]);
            if let Some(&index) = reference.target_record_index.value() {
                let record = forge
                    .record(index)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let source_type = record.type_name().map_err(CatalogError::DataForge)?;
                if reference.guid.value().map(|guid| guid.0) != Some(record.guid())
                    || reference.target_type.value().map(String::as_str) != Some(source_type)
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                if source_type == family && matches!(reference.status(), DefinitionStatus::Present)
                {
                    SourceValue::Present(index)
                } else {
                    SourceValue::Unsupported
                }
            } else if matches!(reference.status(), DefinitionStatus::Dangling) {
                SourceValue::Absent
            } else {
                SourceValue::Unsupported
            }
        }
        None => missing(authored.state())?,
    };
    Ok(sources.value_with_origins(state, &origins)?)
}

type Fallback = (
    Option<AuthoredLink>,
    SourcedValue<u32>,
    SourcedValue<EntitySelection>,
);

fn fallback<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    authored_override: &AuthoredLink,
    preset_record_index: &SourcedValue<u32>,
    cancel: Option<&CancelToken>,
) -> Result<Fallback, CatalogError> {
    let sources = &input.origins().uses;
    let incomplete = sources.origin(&[], false);
    let mut origins = vec![authored_override.origin(), preset_record_index.origin()];
    if matches!(
        authored_override.state(),
        SourceValueState::Invalid | SourceValueState::Unreadable | SourceValueState::Unsupported
    ) {
        origins.push(&incomplete);
    }
    let Some(&index) = preset_record_index.value() else {
        return Ok((
            None,
            sources.value_with_origins(missing(preset_record_index.state())?, &origins)?,
            sources.value_with_origins(missing(preset_record_index.state())?, &origins)?,
        ));
    };
    let selection = sources.value_with_origins(
        SourceValue::Present(EntitySelection::PresetFallback),
        &origins,
    )?;
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let record = forge
        .record(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let Ok(tree) = TreeBuilder::new().build(&record) else {
        origins.push(&incomplete);
        return Ok((
            None,
            sources.value_with_origins(SourceValue::Unreadable, &origins)?,
            selection,
        ));
    };
    let admission = RecordObservation::capture(input, &record)?;
    let (prepared, _) =
        PreparedDefinition::read_original(&tree, input, &admission, cancel, |_, fields, notes| {
            link(&fields, "entityClass", notes)
        })?;
    let actual = prepared.into_definition();
    let selected = target(forge, sources, &actual, "EntityClassDefinition", &origins)?;
    Ok((Some(actual), selected, selection))
}

pub(super) fn missing<T>(state: SourceValueState) -> Result<SourceValue<T>, CatalogError> {
    Ok(match state {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        SourceValueState::Invalid => SourceValue::Invalid,
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    })
}

#[cfg(test)]
#[path = "harvest_dependency_selection_tests.rs"]
mod tests;
