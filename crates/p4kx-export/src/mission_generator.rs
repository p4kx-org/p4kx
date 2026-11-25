// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DecimalU64, PhysicalSourceRef, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::guid_text::CatalogGuid;
use crate::location_hauling_targets::HaulingTargetBinding;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::{SourceArenaOutput, SourceValueRef};
use crate::{CatalogError, ExportAccountingError};

#[path = "mission_generator_entries.rs"]
pub(crate) mod entries;

#[path = "contract_completion_tag_result.rs"]
pub(crate) mod completion_tag_results;

#[path = "contract_item_reward_result.rs"]
pub(crate) mod item_reward_results;

#[path = "generator_mission_item_property.rs"]
pub(crate) mod item_properties;

/// The complete owned source of one admitted `ContractGenerator` Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct GeneratorDefinition<'source> {
    pub(crate) input_ref: PhysicalSourceRef,
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) source: SourceArenaOutput<'source>,
    pub(crate) completion_tag_results: Vec<completion_tag_results::ContractCompletionTagResult>,
    pub(crate) item_reward_results: Vec<item_reward_results::ContractItemRewardResult<'source>>,
    pub(crate) mission_item_properties: Vec<item_properties::GeneratorMissionItemProperty>,
    pub(crate) hauling_target_bindings: Vec<HaulingTargetBinding>,
}

impl<'source> GeneratorDefinition<'source> {
    /// The result-local physical input identity.
    ///
    /// Resolve it through the source inventory in Full provenance.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// The canonical source identity and Record header.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// The complete original ordered fields and linked values.
    #[must_use]
    pub const fn source(&self) -> &SourceArenaOutput<'source> {
        &self.source
    }

    /// The original completion-tag result declarations.
    #[must_use]
    pub fn completion_tag_results(&self) -> &[completion_tag_results::ContractCompletionTagResult] {
        &self.completion_tag_results
    }

    /// The original physical-item reward declarations.
    #[must_use]
    pub fn item_reward_results(&self) -> &[item_reward_results::ContractItemRewardResult<'source>] {
        &self.item_reward_results
    }

    /// The original mission-item property declarations.
    #[must_use]
    pub fn mission_item_properties(&self) -> &[item_properties::GeneratorMissionItemProperty] {
        &self.mission_item_properties
    }

    /// The ordered pickup and drop-off bindings of physical hauling declarations.
    #[must_use]
    pub fn hauling_target_bindings(&self) -> &[HaulingTargetBinding] {
        &self.hauling_target_bindings
    }
}

/// The actual owner-local sites of one projected contract or sub contract.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ContractOccurrence {
    input_ref: PhysicalSourceRef,
    record_index: u32,
    handler: SourceValueRef,
    contract: SourceValueRef,
    sub_contract: Option<SourceValueRef>,
}

impl ContractOccurrence {
    /// The result-local physical input identity.
    ///
    /// Resolve it through the source inventory in Full provenance.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// The actual source owner position in this DataForge.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// The original handler occurrence in the owner's source arena.
    #[must_use]
    pub const fn handler(&self) -> SourceValueRef {
        self.handler
    }

    /// The original parent contract occurrence in the owner's source arena.
    #[must_use]
    pub const fn contract(&self) -> SourceValueRef {
        self.contract
    }

    /// The original sub-contract occurrence, absent on a parent row.
    #[must_use]
    pub const fn sub_contract(&self) -> Option<SourceValueRef> {
        self.sub_contract
    }
}

/// The physical row sites and their immediate original identifier cells.
pub(crate) struct ContractSelection {
    pub(crate) source: ContractOccurrence,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) sub_contract_guid: SourcedValue<CatalogGuid>,
}

/// Borrows the actual admitted owner and contract until its row occurrences are captured.
pub(crate) struct ContractOwner<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    handler: SourceValueRef,
    contract: &'fields OriginalValueFact<'scope, 'read, 'source>,
    guid: SourcedValue<CatalogGuid>,
}

impl<'fields, 'scope, 'read, 'source> ContractOwner<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        handler: &StructValue<'source>,
        container: &str,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        let handler = child(fields, None, "generators", handler)?;
        let contract = child(fields, Some(handler), container, contract)?;
        let guid = identity(fields, contract)?;
        Ok(Self {
            fields,
            handler: value_ref(handler),
            contract,
            guid,
        })
    }

    pub(crate) const fn fields(&self) -> &'fields OriginalFlowFields<'scope, 'read, 'source> {
        self.fields
    }

    pub(crate) fn selection(
        &self,
        sub: Option<&StructValue<'source>>,
    ) -> Result<ContractSelection, CatalogError> {
        let sub = sub
            .map(|sub| child(self.fields, Some(self.contract), "subContracts", sub))
            .transpose()?;
        let sub_contract_guid = if let Some(sub) = sub {
            identity(self.fields, sub)?
        } else {
            let selector = self
                .contract
                .observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
            self.fields
                .sources()
                .value(SourceValue::Absent, &[selector], true)
        };
        Ok(ContractSelection {
            source: ContractOccurrence {
                input_ref: self.fields.input_ref().clone(),
                record_index: self.fields.record_index(),
                handler: self.handler,
                contract: value_ref(self.contract),
                sub_contract: sub.map(value_ref),
            },
            guid: self.guid.clone(),
            sub_contract_guid,
        })
    }
}

fn child<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    parent: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    field: &str,
    structure: &StructValue<'source>,
) -> Result<&'fields OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
    let candidate = fields
        .owner_of(structure)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let (_, declaration) = fields
        .field(parent, field)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let selected = match declaration.value() {
        Value::Struct(_) => std::ptr::eq(declaration, candidate),
        Value::Array(_) => {
            candidate.parent_ordinal() == Some(declaration.ordinal())
                && matches!(candidate.edge(), OriginalValueEdge::Element { .. })
        }
        _ => false,
    };
    if !selected {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(candidate)
}

fn identity<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
    let (_, fact) = fields
        .field(Some(owner), "id")?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let Value::Guid(guid) = fact.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let guid = crate::record_fields::nonnil_guid(*guid)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(fact
        .bind(
            OriginalValuePart::Value,
            SourceValue::Present(CatalogGuid(guid)),
            SourceUseRole::Payload,
            true,
        )?
        .into_cell(fields.sources()))
}

pub(crate) const fn value_ref(fact: &OriginalValueFact<'_, '_, '_>) -> SourceValueRef {
    SourceValueRef {
        value_index: DecimalU64::new(fact.ordinal()),
    }
}
