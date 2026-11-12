// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValueSite};
use crate::resource_raw_policy::ResourceNumberContract;

/// Exact scalar storage of one nonfinite source number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FloatToken {
    Single(u32),
    Double(u64),
}

impl FloatToken {
    pub(crate) const fn read(value: &Value<'_>) -> Option<Self> {
        match value {
            Value::Single(value) if !value.is_finite() => Some(Self::Single(value.to_bits())),
            Value::Double(value) if !value.is_finite() => Some(Self::Double(value.to_bits())),
            _ => None,
        }
    }
}

/// The actual emitted named field or original value occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NumberOccurrence {
    NamedField,
    LootValue { ordinal: u64 },
    BlueprintValue { ordinal: u64 },
    TemplateValue { ordinal: u64 },
    ItemComponentValue { ordinal: u64 },
    ItemComponentRawValue { ordinal: u64 },
    ConfigurationValue { ordinal: u64 },
    ConfigurationRawValue { ordinal: u64 },
    AuthoredDefinitionValue { ordinal: u64 },
}

/// The emitted owner of an exact source scalar.
#[derive(Clone, Copy)]
pub(crate) enum AuthoredNumberOwner {
    ItemComponent,
    Configuration,
    Definition,
}

/// A lossless numeric token retained by the actual raw-field reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedNumber {
    pub(crate) record_index: u32,
    pub(crate) occurrence: NumberOccurrence,
    pub(crate) field: String,
    pub(crate) token: FloatToken,
    pub(crate) source_type: String,
    pub(crate) contract: ResourceNumberContract,
}

impl CapturedNumber {
    /// Retains the exact original Single emitted by a selected component or definition.
    pub(crate) fn authored_single<'scope, 'read, 'source>(
        originals: &OriginalFlowFields<'scope, 'read, 'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        owner: AuthoredNumberOwner,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(token @ FloatToken::Single(_)) = FloatToken::read(fact.value()) else {
            return Ok(None);
        };
        let ordinal = fact.ordinal();
        let occurrence = match owner {
            AuthoredNumberOwner::ItemComponent => NumberOccurrence::ItemComponentValue { ordinal },
            AuthoredNumberOwner::Configuration => NumberOccurrence::ConfigurationValue { ordinal },
            AuthoredNumberOwner::Definition => {
                NumberOccurrence::AuthoredDefinitionValue { ordinal }
            }
        };
        Self::authored(
            originals,
            fact,
            occurrence,
            token,
            ResourceNumberContract::Single,
        )
        .map(Some)
    }

    /// Retains an exact nonfinite scalar emitted inside an authored value subtree.
    pub(crate) fn authored_raw<'scope, 'read, 'source>(
        originals: &OriginalFlowFields<'scope, 'read, 'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        owner: AuthoredNumberOwner,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(token) = FloatToken::read(fact.value()) else {
            return Ok(None);
        };
        let ordinal = fact.ordinal();
        let occurrence = match owner {
            AuthoredNumberOwner::Configuration => {
                NumberOccurrence::ConfigurationRawValue { ordinal }
            }
            AuthoredNumberOwner::ItemComponent | AuthoredNumberOwner::Definition => {
                NumberOccurrence::ItemComponentRawValue { ordinal }
            }
        };
        Self::authored(
            originals,
            fact,
            occurrence,
            token,
            ResourceNumberContract::Number,
        )
        .map(Some)
    }

    fn authored<'scope, 'read, 'source>(
        originals: &OriginalFlowFields<'scope, 'read, 'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        occurrence: NumberOccurrence,
        token: FloatToken,
        contract: ResourceNumberContract,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            record_index: originals.record_index(),
            occurrence,
            field: originals.logical_path(fact)?,
            token,
            source_type: fact.owner_type().to_owned(),
            contract,
        })
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CapturedPosition {
    record_index: u32,
    occurrence: NumberOccurrence,
    field: String,
}

/// Local evidence committed only with its successfully returned canonical definition.
#[derive(Default)]
pub(crate) struct NumberCapture {
    values: RefCell<BTreeMap<CapturedPosition, CapturedScalar>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedScalar {
    source_type: String,
    contract: ResourceNumberContract,
    token: FloatToken,
}

impl NumberCapture {
    pub(crate) fn retain(
        &self,
        record_index: u32,
        field: impl FnOnce() -> String,
        source_type: &str,
        contract: ResourceNumberContract,
        value: &Value<'_>,
    ) {
        self.retain_at(
            record_index,
            NumberOccurrence::NamedField,
            field,
            source_type,
            contract,
            value,
        );
    }

    /// Retains a nonfinite token only from this actual original Loot value visit.
    pub(crate) fn retain_loot(&self, site: &OriginalValueSite<'_, '_, '_, '_>) {
        self.retain_original(
            site,
            NumberOccurrence::LootValue {
                ordinal: site.ordinal(),
            },
        );
    }

    /// Retains a nonfinite token from this actual original Blueprint value visit.
    pub(crate) fn retain_blueprint(&self, site: &OriginalValueSite<'_, '_, '_, '_>) {
        self.retain_original(
            site,
            NumberOccurrence::BlueprintValue {
                ordinal: site.ordinal(),
            },
        );
    }

    fn retain_original(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
        occurrence: NumberOccurrence,
    ) {
        self.retain_at(
            site.record_index(),
            occurrence,
            || site.logical_path().to_owned(),
            site.owner_type(),
            ResourceNumberContract::Number,
            site.value(),
        );
    }

    fn retain_at(
        &self,
        record_index: u32,
        occurrence: NumberOccurrence,
        field: impl FnOnce() -> String,
        source_type: &str,
        contract: ResourceNumberContract,
        value: &Value<'_>,
    ) {
        if let Some(token) = FloatToken::read(value) {
            self.values
                .borrow_mut()
                .entry(CapturedPosition {
                    record_index,
                    occurrence,
                    field: field(),
                })
                .or_insert_with(|| CapturedScalar {
                    source_type: source_type.to_owned(),
                    contract,
                    token,
                });
        }
    }

    pub(crate) fn finish(self) -> Vec<CapturedNumber> {
        self.values
            .into_inner()
            .into_iter()
            .map(|(position, scalar)| CapturedNumber {
                record_index: position.record_index,
                occurrence: position.occurrence,
                field: position.field,
                source_type: scalar.source_type,
                contract: scalar.contract,
                token: scalar.token,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "catalog_number_capture_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "catalog_number_occurrence_tests.rs"]
mod occurrence_tests;
