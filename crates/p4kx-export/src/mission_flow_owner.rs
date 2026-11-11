// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::PhysicalSourceRef;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::OriginalFlowFields;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FlowOwnerKind {
    ContractTemplate,
    MissionBrokerEntry,
}

impl FlowOwnerKind {
    pub(crate) const fn properties(self) -> &'static str {
        match self {
            Self::ContractTemplate => "contractProperties",
            Self::MissionBrokerEntry => "properties",
        }
    }

    pub(crate) const fn catalog_kind(self) -> crate::Kind {
        match self {
            Self::ContractTemplate => crate::Kind::ContractTemplates,
            Self::MissionBrokerEntry => crate::Kind::MissionBrokerEntries,
        }
    }

    pub(crate) const fn spawn_level(self) -> crate::spawn_model::SpawnLevel {
        match self {
            Self::ContractTemplate => crate::spawn_model::SpawnLevel::Template,
            Self::MissionBrokerEntry => crate::spawn_model::SpawnLevel::Broker,
        }
    }

    pub(crate) const fn objective_hauling(self) -> crate::contract_hauling::HaulingSource {
        match self {
            Self::ContractTemplate => crate::contract_hauling::HaulingSource::TemplateObjective,
            Self::MissionBrokerEntry => crate::contract_hauling::HaulingSource::BrokerObjective,
        }
    }
}

/// Structural navigation to one actual emitted physical mission owner.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PhysicalFlowOwner {
    input_ref: PhysicalSourceRef,
    record_index: u32,
    record_guid: CatalogGuid,
}

/// A Flow's physical definition family, independent of its source payload.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum MissionFlowOwnerRef {
    ContractTemplate(PhysicalFlowOwner),
    MissionBrokerEntry(PhysicalFlowOwner),
}

impl MissionFlowOwnerRef {
    pub(crate) fn read(
        fields: &OriginalFlowFields<'_, '_, '_>,
        kind: FlowOwnerKind,
    ) -> Result<Self, CatalogError> {
        fields.check_cancel()?;
        let expected = match kind {
            FlowOwnerKind::ContractTemplate => "ContractTemplate",
            FlowOwnerKind::MissionBrokerEntry => "MissionBrokerEntry",
        };
        if fields.root().type_name() != expected {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let physical = PhysicalFlowOwner {
            input_ref: fields.input_ref().clone(),
            record_index: fields.record_index(),
            record_guid: CatalogGuid(fields.record_guid()),
        };
        Ok(match kind {
            FlowOwnerKind::ContractTemplate => Self::ContractTemplate(physical),
            FlowOwnerKind::MissionBrokerEntry => Self::MissionBrokerEntry(physical),
        })
    }
}
