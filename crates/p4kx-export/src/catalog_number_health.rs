// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::ExportAccountingError;
use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_row::CatalogRows;
use crate::catalog_target_index::TargetIndex;
use crate::export_ledger::SourceObservation;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;
use crate::template_number_capture::TemplateNumbers;

/// One original nonfinite token awaiting exact actual emitted raw coverage.
#[derive(Debug, Clone)]
pub(crate) struct NumberRequirement {
    pub(crate) record_index: u32,
    pub(crate) original_ordinal: u64,
    pub(crate) field: String,
    pub(crate) source_type: String,
    pub(crate) contract: Option<ResourceNumberContract>,
    pub(crate) token: FloatToken,
    pub(crate) body: Option<ExampleBodyReceipt>,
}

impl NumberRequirement {
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        if *self != other {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        match (&mut self.body, other.body) {
            (Some(body), Some(other)) => body.merge(other),
            (None, None) => Ok(()),
            _ => Err(ExportAccountingError::UnboundHealthSubject),
        }
    }
}

// Value equality is separate from the original body-receipt merge.
impl PartialEq for NumberRequirement {
    fn eq(&self, other: &Self) -> bool {
        self.record_index == other.record_index
            && self.original_ordinal == other.original_ordinal
            && self.field == other.field
            && self.source_type == other.source_type
            && self.contract == other.contract
            && self.token == other.token
    }
}

impl Eq for NumberRequirement {}

/// Actual numeric receipts attached to one emitted canonical family.
#[derive(Debug, Clone)]
pub(crate) struct EmittedNumber {
    pub(crate) kind: Kind,
    pub(crate) value: CapturedNumber,
}

#[derive(Default)]
struct RecordNumbers<'source> {
    named: BTreeMap<&'source str, &'source CapturedNumber>,
    original: BTreeMap<u64, &'source CapturedNumber>,
}

/// Original numeric receipts indexed once by their admitted emitted physical Record.
pub(crate) struct NumberCaptures<'source> {
    records: BTreeMap<u32, RecordNumbers<'source>>,
    templates: BTreeMap<u32, Vec<&'source TemplateNumbers>>,
}

impl<'source> NumberCaptures<'source> {
    pub(crate) fn new(rows: Option<&'source CatalogRows>, targets: Option<&TargetIndex>) -> Self {
        let mut records = BTreeMap::<u32, RecordNumbers<'source>>::new();
        let mut templates = BTreeMap::<u32, Vec<&TemplateNumbers>>::new();
        if let (Some(rows), Some(targets)) = (rows, targets) {
            for number in &rows.numbers {
                let actual = &number.value;
                // Item component receipts come from the emitted component rows.
                let emitted = match actual.occurrence {
                    NumberOccurrence::ItemComponentValue { .. } => number.kind == Kind::Items,
                    _ => targets
                        .emitted
                        .contains(&(number.kind, actual.record_index)),
                };
                if !emitted {
                    continue;
                }
                if matches!(actual.occurrence, NumberOccurrence::TemplateValue { .. }) {
                    continue;
                }
                if matches!(actual.occurrence, NumberOccurrence::LootValue { .. })
                    && number.kind != Kind::LootPoolFilters
                {
                    continue;
                }
                match actual.occurrence {
                    NumberOccurrence::NamedField => {
                        records
                            .entry(actual.record_index)
                            .or_default()
                            .named
                            .entry(actual.field.as_str())
                            .or_insert(actual);
                    }
                    NumberOccurrence::TemplateValue { .. } => {}
                    NumberOccurrence::LootValue { ordinal }
                    | NumberOccurrence::BlueprintValue { ordinal }
                    | NumberOccurrence::ItemComponentValue { ordinal }
                    | NumberOccurrence::AuthoredDefinitionValue { ordinal } => {
                        if matches!(actual.occurrence, NumberOccurrence::BlueprintValue { .. })
                            && number.kind != Kind::Blueprints
                        {
                            continue;
                        }
                        records
                            .entry(actual.record_index)
                            .or_default()
                            .original
                            .entry(ordinal)
                            .or_insert(actual);
                    }
                }
            }
            for template in &rows.template_numbers {
                if let (Some(index), Some(kind)) =
                    (template.record_index(), template.emitted_kind())
                    && targets.emitted.contains(&(kind, index))
                {
                    templates.entry(index).or_default().push(template);
                }
            }
        }
        Self { records, templates }
    }

    pub(crate) fn for_source(
        &self,
        source: &SourceObservation,
    ) -> Result<Coverage<'_, 'source>, ExportAccountingError> {
        let mut coverage = self.for_record(source.record_index, source.kind);
        for template in self
            .templates
            .get(&source.record_index)
            .into_iter()
            .flatten()
        {
            if let Some(values) = template.for_source(source)?
                && coverage.template.replace(values).is_some()
            {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
        }
        Ok(coverage)
    }
    pub(crate) fn for_record(
        &self,
        record_index: u32,
        kind: Option<Kind>,
    ) -> Coverage<'_, 'source> {
        Coverage {
            record_index,
            kind,
            available: self.records.get(&record_index),
            template: None,
            consumed_named: BTreeSet::new(),
            consumed_owned: BTreeSet::new(),
        }
    }
}

/// Fresh consumption state for one actual source role and physical Record.
pub(crate) struct Coverage<'index, 'source> {
    record_index: u32,
    kind: Option<Kind>,
    available: Option<&'index RecordNumbers<'source>>,
    template: Option<&'source BTreeMap<u64, CapturedNumber>>,
    consumed_named: BTreeSet<&'source str>,
    consumed_owned: BTreeSet<u64>,
}

impl Coverage<'_, '_> {
    pub(crate) fn take(&mut self, required: &NumberRequirement) -> bool {
        if required.record_index != self.record_index {
            return false;
        }
        if let Some(values) = self.template {
            let actual = values.get(&required.original_ordinal);
            return actual.is_some_and(|actual| {
                scalar_matches(required, actual)
                    && actual.occurrence
                        == NumberOccurrence::TemplateValue {
                            ordinal: required.original_ordinal,
                        }
                    && actual.contract == ResourceNumberContract::Number
                    && self.consumed_owned.insert(required.original_ordinal)
            });
        }
        if matches!(
            self.kind,
            Some(
                Kind::LootPoolFilters | Kind::Blueprints | Kind::Items | Kind::ExplosionDefinitions
            )
        ) {
            return self.owned(required);
        }
        let Some(contract) = required.contract else {
            return false;
        };
        let Some((field, actual)) = self
            .available
            .and_then(|values| values.named.get_key_value(required.field.as_str()))
        else {
            return false;
        };
        scalar_matches(required, actual)
            && actual.contract == contract
            && self.consumed_named.insert(*field)
    }

    fn owned(&mut self, required: &NumberRequirement) -> bool {
        let Some(actual) = self
            .available
            .and_then(|values| values.original.get(&required.original_ordinal))
        else {
            return false;
        };
        let ordinal = required.original_ordinal;
        let (occurrence, contract) = match self.kind {
            Some(Kind::LootPoolFilters) => (
                NumberOccurrence::LootValue { ordinal },
                ResourceNumberContract::Number,
            ),
            Some(Kind::Blueprints) => (
                NumberOccurrence::BlueprintValue { ordinal },
                ResourceNumberContract::Number,
            ),
            Some(Kind::Items) => (
                NumberOccurrence::ItemComponentValue { ordinal },
                ResourceNumberContract::Single,
            ),
            Some(Kind::ExplosionDefinitions) => (
                NumberOccurrence::AuthoredDefinitionValue { ordinal },
                ResourceNumberContract::Single,
            ),
            _ => return false,
        };
        scalar_matches(required, actual)
            && actual.occurrence == occurrence
            && actual.contract == contract
            && self.consumed_owned.insert(required.original_ordinal)
    }
}

fn scalar_matches(required: &NumberRequirement, actual: &CapturedNumber) -> bool {
    actual.record_index == required.record_index
        && actual.token == required.token
        && actual.source_type == required.source_type
        && actual.field == required.field
}
