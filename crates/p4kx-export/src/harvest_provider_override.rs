// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, Guid, SourceUses, SourceValue, SourcedValue, Step};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::authored_link::HealthLink;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceWeakTarget;

/// The exact original target of an area modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum ModifierTarget {
    /// The authored pointer selected a Record header.
    Record {
        /// Sourced metadata of the original target Record.
        reference: Box<CatalogRecordReference>,
    },
    /// The authored pointer selected an owned occurrence.
    Instance {
        /// Sourced identity of the original target instance.
        reference: Box<ModifierInstance>,
    },
}

/// One physical owned pointer target.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ModifierInstance {
    /// Physical index of the owning Record.
    pub target_record_index: SourcedValue<u32>,
    /// Original owning Record GUID.
    #[serde(serialize_with = "crate::canonical_record_header::serialize_guid")]
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub record_guid: SourcedValue<Guid>,
    /// Original owning Record path.
    pub record_path: SourcedValue<String>,
    /// Target type at the original occurrence.
    pub source_type: SourcedValue<String>,
    /// Authored-path representation of the original pointer target.
    pub location: SourcedValue<String>,
    /// The exact same-provider group slot, when the target names one.
    pub provider_slot: SourcedValue<ProviderSlot>,
}

/// The ordered group and element slots of a same-provider modifier target.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ProviderSlot {
    /// The original group position.
    pub group_index: SourcedValue<u32>,
    /// The original element position in that group.
    pub harvestable_index: SourcedValue<u32>,
}

pub(super) fn read_modifier_target(
    fields: Observed<'_, '_, '_, '_>,
    owner: &SourcedValue<u32>,
    sources: &SourceUses,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Box<ModifierTarget>>, CatalogError> {
    fields.link("harvestableElement", notes)?.try_map(|target| {
        let target = match target {
            HealthLink::RecordReference { value } => ModifierTarget::Record {
                reference: Box::new((*value).into_catalog()),
            },
            HealthLink::WeakReference { value } => {
                let weak = *value;
                match &weak.target {
                    SourceWeakTarget::Record { .. } => {
                        let reference = weak
                            .into_record()
                            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
                        ModifierTarget::Record {
                            reference: Box::new(reference.into_catalog()),
                        }
                    }
                    SourceWeakTarget::Instance { .. } => {
                        let SourceWeakTarget::Instance {
                            source_type,
                            location,
                        } = weak.target
                        else {
                            return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
                        };
                        let slot = location
                            .value()
                            .and_then(|value| provider_slot(value.steps()));
                        let provider_slot = bind_provider_slot(
                            sources,
                            owner,
                            &weak.target_record_index,
                            location.origin(),
                            slot,
                        )?;
                        ModifierTarget::Instance {
                            reference: Box::new(ModifierInstance {
                                target_record_index: weak.target_record_index,
                                record_guid: weak.record_guid.map(|guid| guid.0),
                                record_path: weak.record_path.map(str::to_owned),
                                source_type: source_type.map(str::to_owned),
                                provider_slot,
                                location: location.map(|value| value.as_ref().to_owned()),
                            }),
                        }
                    }
                }
            }
        };
        Ok(Box::new(target))
    })
}

fn bind_provider_slot(
    sources: &SourceUses,
    owner: &SourcedValue<u32>,
    target: &SourcedValue<u32>,
    location: &FieldOrigin,
    slot: Option<(u32, u32)>,
) -> Result<SourcedValue<ProviderSlot>, CatalogError> {
    let origins = [owner.origin(), target.origin(), location];
    let slot = slot.filter(|_| owner.value().is_some() && owner.value() == target.value());
    let value = match slot {
        Some((group, element)) => SourceValue::Present(ProviderSlot {
            group_index: sources.value_with_origins(SourceValue::Present(group), &origins)?,
            harvestable_index: sources
                .value_with_origins(SourceValue::Present(element), &origins)?,
        }),
        None => SourceValue::Absent,
    };
    Ok(sources.value_with_origins(value, &origins)?)
}

fn provider_slot(steps: &[Step<'_>]) -> Option<(u32, u32)> {
    match steps {
        [
            Step::Field("harvestableGroups"),
            Step::Element(group),
            Step::Field("harvestables"),
            Step::Element(element),
        ] => Some((*group, *element)),
        _ => None,
    }
}
