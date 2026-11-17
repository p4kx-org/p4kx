// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, Guid, SourceUseRole, SourceUses, SourceValue,
    SourceValueState, SourcedValue, TreeBuilder,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::prepared_definition::PreparedDefinition;
use super::harvest_dependency_selection::{
    EntitySelection, HarvestDependencySelection, choose, missing,
};
use super::harvest_provider::record_link;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::crafting_warning::CraftingWarning;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, export_evidence_lock};

type RecordLink = SourcedValue<Box<CatalogRecordReference>>;

/// The actual dependency reads used to resolve the element.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SpawnSelection {
    /// Selected preset class name.
    pub preset_class_name: SourcedValue<String>,
    /// The consulted preset entity selector when fallback was used.
    pub preset_entity_class: Option<RecordLink>,
    /// Which selector determined the chosen entity.
    pub entity_selection: SourcedValue<EntitySelection>,
    /// The selected entity and its authored mining parameters.
    pub entity: SourcedValue<HarvestEntity>,
}

/// One chosen entity Record and its mining component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestEntity {
    /// Physical Record index selected by the actual reference.
    pub record_index: SourcedValue<u32>,
    /// GUID of the selected `EntityClass` Record.
    #[serde(serialize_with = "crate::canonical_record_header::serialize_guid")]
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub guid: SourcedValue<Guid>,
    /// First authored `MineableParams` component.
    pub mining_params: SourcedValue<HarvestMiningParams>,
}

/// The selected entity's original mining component selectors.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestMiningParams {
    /// Physical position in Components.
    pub component_index: SourcedValue<u32>,
    /// Original composition reference.
    pub composition: RecordLink,
    /// Original mining global parameters reference.
    pub global_params: RecordLink,
}

/// Intrinsic entity bodies admitted within a single projection input.
#[derive(Default)]
pub(crate) struct ProviderDependencies {
    mineables: Mutex<HashMap<u32, Arc<SourcedValue<HarvestMiningParams>>>>,
    unreadable: Mutex<BTreeSet<String>>,
}

impl ProviderDependencies {
    pub(crate) fn unreadable_count(&self) -> Result<usize, CatalogError> {
        Ok(export_evidence_lock::lock(&self.unreadable)?.len())
    }
    pub(crate) fn warning(&self) -> Result<Option<CraftingWarning>, CatalogError> {
        let unreadable = export_evidence_lock::lock(&self.unreadable)?;
        Ok(
            (!unreadable.is_empty()).then(|| CraftingWarning::UnreadableHarvestables {
                count: unreadable.len(),
                details: unreadable.iter().cloned().collect(),
            }),
        )
    }

    fn unreadable(&self, source: String) -> Result<(), CatalogError> {
        export_evidence_lock::lock(&self.unreadable)?.insert(source);
        Ok(())
    }
    pub(super) fn select<'source>(
        &self,
        element: Observed<'_, '_, '_, 'source>,
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(RecordLink, RecordLink, SpawnSelection), CatalogError> {
        let HarvestDependencySelection {
            authored_preset,
            authored_override,
            fallback_entity,
            preset_record_index,
            entity_record_index,
            entity_selection,
        } = choose(element, forge, input, cancel, notes)?;
        let preset_class_name = preset_name(forge, input, &preset_record_index)?;
        if entity_record_index.state() == SourceValueState::Unreadable
            && let Some(&index) = preset_record_index.value()
        {
            self.unreadable(format!(
                "HarvestablePreset Record {index} cannot resolve its entity"
            ))?;
        }
        let entity = self.entity(forge, input, &entity_record_index, cancel)?;
        Ok((
            authored_preset,
            authored_override,
            SpawnSelection {
                preset_class_name,
                preset_entity_class: fallback_entity,
                entity_selection,
                entity,
            },
        ))
    }

    fn entity<'source>(
        &self,
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        selected: &SourcedValue<u32>,
        cancel: Option<&CancelToken>,
    ) -> Result<SourcedValue<HarvestEntity>, CatalogError> {
        let source = &input.origins().uses;
        let Some(&index) = selected.value() else {
            return Ok(source.value_with_origins(missing(selected.state())?, &[selected.origin()])?);
        };
        let record = forge
            .record(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let context = input.record_context(&record)?;
        let guid =
            context.header_value(RecordHeaderField::Guid, SourceValue::Present(record.guid()))?;
        let route = [selected.origin()];
        let params = self.mineable(forge, input, index, cancel)?;
        let component_origins = [selected.origin(), params.origin()];
        let state = match params.value() {
            Some(params) => SourceValue::Present(HarvestMiningParams {
                component_index: bind(source, &params.component_index, &component_origins)?,
                composition: bind_link(source, &params.composition, &component_origins)?,
                global_params: bind_link(source, &params.global_params, &component_origins)?,
            }),
            None => missing(params.state())?,
        };
        let mining_params = source.value_with_origins(state, &component_origins)?;
        Ok(source.value_with_origins(
            SourceValue::Present(HarvestEntity {
                record_index: selected.clone(),
                guid: bind(source, &guid, &route)?,
                mining_params,
            }),
            &route,
        )?)
    }

    fn mineable<'source>(
        &self,
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<Arc<SourcedValue<HarvestMiningParams>>, CatalogError> {
        let cached = export_evidence_lock::lock(&self.mineables)?
            .get(&index)
            .cloned();
        if let Some(found) = cached {
            return Ok(found);
        }
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let record = forge
            .record(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let found = if let Ok(tree) = TreeBuilder::new().build(&record) {
            let admission = RecordObservation::capture(input, &record)?;
            let (observed, details) = PreparedDefinition::read_original(
                &tree,
                input,
                &admission,
                cancel,
                |_, fields, notes| {
                    fields.harvest_mineable(&tree, |mineable| {
                        Ok(HarvestMiningParams {
                            component_index: mineable.array_ordinal()?,
                            composition: record_link(mineable, "composition", notes)?,
                            global_params: record_link(mineable, "globalParams", notes)?,
                        })
                    })
                },
            )?;
            if !details.is_empty() {
                self.unreadable(format!(
                    "entity Record {index} has unreadable mining parameter fields"
                ))?;
            }
            observed.into_definition()
        } else {
            self.unreadable(format!("entity Record {index} cannot be read"))?;
            input
                .origins()
                .uses
                .value(SourceValue::Unreadable, &[], false)
        };
        let mut cache = export_evidence_lock::lock(&self.mineables)?;
        Ok(Arc::clone(
            cache.entry(index).or_insert_with(|| Arc::new(found)),
        ))
    }
}

fn preset_name<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    selected: &SourcedValue<u32>,
) -> Result<SourcedValue<String>, CatalogError> {
    let source = &input.origins().uses;
    let Some(&index) = selected.value() else {
        return Ok(source.value_with_origins(missing(selected.state())?, &[selected.origin()])?);
    };
    let record = forge
        .record(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let context = input.record_context(&record)?;
    let short = record.name().and_then(|name| {
        record
            .type_name()
            .map(|source_type| crate::record_fields::unqualified_class_name(name, source_type))
    });
    let complete = short.is_ok();
    let (state, name_role) = short.map_or(
        (SourceValue::Unreadable, SourceUseRole::Attempted),
        |short| {
            (
                SourceValue::Present(short.to_owned()),
                SourceUseRole::Conversion,
            )
        },
    );
    let fields = [
        context.observe_header(RecordHeaderField::RecordName, name_role)?,
        context.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
    ];
    let origin = source.origin(&fields, complete);
    Ok(source.value_with_origins(state, &[selected.origin(), &origin])?)
}

fn bind<T: Clone>(
    source: &SourceUses,
    value: &SourcedValue<T>,
    parents: &[&FieldOrigin],
) -> Result<SourcedValue<T>, CatalogError> {
    let state = match value.value() {
        Some(value) => SourceValue::Present(value.clone()),
        None => missing(value.state())?,
    };
    let mut origins = parents.to_vec();
    origins.push(value.origin());
    Ok(source.value_with_origins(state, &origins)?)
}

fn bind_link(
    source: &SourceUses,
    value: &RecordLink,
    parents: &[&FieldOrigin],
) -> Result<RecordLink, CatalogError> {
    let state = match value.value() {
        Some(link) => SourceValue::Present(Box::new(CatalogRecordReference {
            target_record_index: bind(source, &link.target_record_index, parents)?,
            guid: bind(source, &link.guid, parents)?,
            record_path: bind(source, &link.record_path, parents)?,
            target_type: bind(source, &link.target_type, parents)?,
            status: link.status.clone(),
        })),
        None => missing(value.state())?,
    };
    let mut origins = parents.to_vec();
    origins.push(value.origin());
    Ok(source.value_with_origins(state, &origins)?)
}
