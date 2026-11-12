// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::ExportAccountingError;
use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_row::CatalogRows;
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDisposition, SourceObservation};
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
    configurations: BTreeMap<u64, &'source CapturedNumber>,
    ammo_definition: bool,
}

/// Original numeric receipts indexed once by their admitted emitted physical Record.
pub(crate) struct NumberCaptures<'source> {
    records: BTreeMap<u32, RecordNumbers<'source>>,
    templates: BTreeMap<u32, Vec<&'source TemplateNumbers>>,
    recovery: BTreeMap<u32, BTreeMap<&'source str, &'source CapturedNumber>>,
}

fn emitted_record_numbers<'source>(
    sources: Option<(&'source CatalogRows, &TargetIndex)>,
) -> BTreeMap<u32, RecordNumbers<'source>> {
    let mut records = BTreeMap::<u32, RecordNumbers<'source>>::new();
    if let Some((rows, targets)) = sources {
        index_configurations(&mut records, &rows.configuration_numbers);
        for number in &rows.numbers {
            let actual = &number.value;
            // Item component receipts come from the emitted component rows.
            let emitted = match actual.occurrence {
                NumberOccurrence::ItemComponentValue { .. }
                | NumberOccurrence::ItemComponentRawValue { .. } => number.kind == Kind::Items,
                _ => targets
                    .emitted
                    .contains(&(number.kind, actual.record_index)),
            };
            if !emitted {
                continue;
            }
            if number.kind == Kind::AmmoDefinitions {
                records
                    .entry(actual.record_index)
                    .or_default()
                    .ammo_definition = true;
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
                NumberOccurrence::TemplateValue { .. }
                | NumberOccurrence::ConfigurationValue { .. }
                | NumberOccurrence::ConfigurationRawValue { .. } => {}
                NumberOccurrence::LootValue { ordinal }
                | NumberOccurrence::BlueprintValue { ordinal }
                | NumberOccurrence::ItemComponentValue { ordinal }
                | NumberOccurrence::ItemComponentRawValue { ordinal }
                | NumberOccurrence::AuthoredDefinitionValue { ordinal }
                | NumberOccurrence::AuthoredDefinitionRawValue { ordinal } => {
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
    }
    records
}

fn index_configurations<'source>(
    records: &mut BTreeMap<u32, RecordNumbers<'source>>,
    numbers: &'source [CapturedNumber],
) {
    for number in numbers {
        let (NumberOccurrence::ItemComponentValue { ordinal }
        | NumberOccurrence::ItemComponentRawValue { ordinal }
        | NumberOccurrence::ConfigurationValue { ordinal }
        | NumberOccurrence::ConfigurationRawValue { ordinal }) = number.occurrence
        else {
            continue;
        };
        records
            .entry(number.record_index)
            .or_default()
            .configurations
            .insert(ordinal, number);
    }
}

impl<'source> NumberCaptures<'source> {
    pub(crate) fn new(rows: Option<&'source CatalogRows>, targets: Option<&TargetIndex>) -> Self {
        let records = emitted_record_numbers(rows.zip(targets));
        Self {
            records,
            templates: rows
                .zip(targets)
                .map_or_else(BTreeMap::new, |(rows, targets)| {
                    emitted_templates(rows, targets)
                }),
            recovery: recovery_numbers(rows),
        }
    }

    pub(crate) fn for_source(
        &self,
        source: &SourceObservation,
    ) -> Result<Coverage<'_, 'source>, ExportAccountingError> {
        let mut coverage = self.for_record(source.record_index, source.kind);
        if source.kind == Some(Kind::Items)
            && source.role == ExportSourceRole::Dependency
            && coverage
                .available
                .is_some_and(|numbers| numbers.ammo_definition)
        {
            coverage.kind = Some(Kind::AmmoDefinitions);
        }
        if source.kind.is_none()
            && source.role == ExportSourceRole::Dependency
            && source.disposition == SourceDisposition::Projected
        {
            coverage.recovery = self.recovery.get(&source.record_index);
        }
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
            recovery: None,
            consumed_named: BTreeSet::new(),
            consumed_owned: BTreeSet::new(),
        }
    }
}

fn emitted_templates<'source>(
    rows: &'source CatalogRows,
    targets: &TargetIndex,
) -> BTreeMap<u32, Vec<&'source TemplateNumbers>> {
    let mut templates = BTreeMap::<u32, Vec<&TemplateNumbers>>::new();
    for template in &rows.template_numbers {
        if let (Some(index), Some(kind)) = (template.record_index(), template.emitted_kind())
            && targets.emitted.contains(&(kind, index))
        {
            templates.entry(index).or_default().push(template);
        }
    }
    templates
}

fn recovery_numbers(rows: Option<&CatalogRows>) -> BTreeMap<u32, BTreeMap<&str, &CapturedNumber>> {
    let mut records = BTreeMap::<u32, BTreeMap<&str, &CapturedNumber>>::new();
    for number in rows.into_iter().flat_map(|rows| &rows.recovery_numbers) {
        records
            .entry(number.record_index)
            .or_default()
            .insert(&number.field, number);
    }
    records
}

/// Fresh consumption state for one actual source role and physical Record.
pub(crate) struct Coverage<'index, 'source> {
    record_index: u32,
    kind: Option<Kind>,
    available: Option<&'index RecordNumbers<'source>>,
    template: Option<&'source BTreeMap<u64, CapturedNumber>>,
    recovery: Option<&'index BTreeMap<&'source str, &'source CapturedNumber>>,
    consumed_named: BTreeSet<&'source str>,
    consumed_owned: BTreeSet<u64>,
}

impl Coverage<'_, '_> {
    pub(crate) fn take(&mut self, required: &NumberRequirement) -> bool {
        if required.record_index != self.record_index {
            return false;
        }
        if let Some(values) = self.recovery {
            return values
                .get_key_value(required.field.as_str())
                .is_some_and(|(field, actual)| {
                    scalar_matches(required, actual)
                        && matches!(actual.occurrence, NumberOccurrence::NamedField)
                        && self.consumed_named.insert(*field)
                });
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
        if let Some(matched) = self.configuration(required) {
            return matched;
        }
        if matches!(
            self.kind,
            Some(
                Kind::LootPoolFilters
                    | Kind::Blueprints
                    | Kind::Items
                    | Kind::ExplosionDefinitions
                    | Kind::AmmoDefinitions
                    | Kind::Factions
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

    fn configuration(&mut self, required: &NumberRequirement) -> Option<bool> {
        let actual = self
            .available?
            .configurations
            .get(&required.original_ordinal)?;
        let contract = match actual.occurrence {
            NumberOccurrence::ItemComponentValue { .. }
            | NumberOccurrence::ConfigurationValue { .. } => ResourceNumberContract::Single,
            NumberOccurrence::ItemComponentRawValue { .. }
            | NumberOccurrence::ConfigurationRawValue { .. } => ResourceNumberContract::Number,
            _ => return None,
        };
        Some(
            scalar_matches(required, actual)
                && actual.contract == contract
                && self.consumed_owned.insert(required.original_ordinal),
        )
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
            Some(Kind::Items)
                if matches!(
                    actual.occurrence,
                    NumberOccurrence::ItemComponentRawValue { .. }
                ) =>
            {
                (
                    NumberOccurrence::ItemComponentRawValue { ordinal },
                    ResourceNumberContract::Number,
                )
            }
            Some(Kind::Items) => (
                NumberOccurrence::ItemComponentValue { ordinal },
                ResourceNumberContract::Single,
            ),
            Some(Kind::Factions) => (
                NumberOccurrence::AuthoredDefinitionRawValue { ordinal },
                ResourceNumberContract::Number,
            ),
            Some(Kind::ExplosionDefinitions | Kind::AmmoDefinitions) => (
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
