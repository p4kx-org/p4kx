// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_read::original::RecordSubject;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceObservation;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, ExportAccountingError, Kind};

/// Exact emitted original occurrences together with their actual physical Record admission.
#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateNumbers {
    emission: Option<Arc<Emission>>,
}

#[derive(Debug)]
struct Emission {
    subject: RecordSubject,
    record_index: u32,
    kind: Kind,
    values: BTreeMap<u64, CapturedNumber>,
}

impl TemplateNumbers {
    pub(crate) fn read(
        members: &TemplateMembers<'_, '_, '_, '_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        Self::read_family(members, input, Kind::ContractTemplates)
    }

    pub(crate) fn broker(
        members: &TemplateMembers<'_, '_, '_, '_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        Self::read_family(members, input, Kind::MissionBrokerEntries)
    }

    fn read_family(
        members: &TemplateMembers<'_, '_, '_, '_>,
        input: &OriginalCatalogInput<'_, '_>,
        kind: Kind,
    ) -> Result<Self, CatalogError> {
        let fields = members.fields();
        if fields.root().type_name() != kind.records().type_name {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut values = BTreeMap::new();
        for fact in members.values() {
            fields.check_cancel()?;
            let Some(token) = FloatToken::read(fact.value()) else {
                continue;
            };
            fact.check_input(input)?;
            members.value_ref(fact)?;
            if values
                .insert(
                    fact.ordinal(),
                    CapturedNumber {
                        record_index: fields.record_index(),
                        occurrence: NumberOccurrence::TemplateValue {
                            ordinal: fact.ordinal(),
                        },
                        field: fields.logical_path(fact)?,
                        token,
                        source_type: fact.owner_type().to_owned(),
                        contract: ResourceNumberContract::Number,
                    },
                )
                .is_some()
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        }
        if values.is_empty() {
            return Ok(Self::default());
        }
        Ok(Self {
            emission: Some(Arc::new(Emission {
                subject: fields.record_subject(input)?,
                record_index: fields.record_index(),
                kind,
                values,
            })),
        })
    }

    pub(crate) fn record_index(&self) -> Option<u32> {
        self.emission.as_ref().map(|emission| emission.record_index)
    }

    pub(crate) fn emitted_kind(&self) -> Option<Kind> {
        self.emission.as_ref().map(|emission| emission.kind)
    }

    pub(crate) fn for_source(
        &self,
        source: &SourceObservation,
    ) -> Result<Option<&BTreeMap<u64, CapturedNumber>>, ExportAccountingError> {
        let Some(emission) = &self.emission else {
            return Ok(None);
        };
        if !eligible(source, emission.kind) {
            return Ok(None);
        }
        source.key()?;
        if emission.record_index != source.record_index {
            return Ok(None);
        }
        let mut owner = emission.subject.clone();
        match owner.merge(source.subject().clone()) {
            Ok(()) => Ok(Some(&emission.values)),
            Err(ExportAccountingError::UnboundHealthSubject) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

const fn eligible(source: &SourceObservation, family: Kind) -> bool {
    matches!(
        (family, source.kind, source.role),
        (
            Kind::ContractTemplates,
            Some(Kind::ContractTemplates),
            ExportSourceRole::Selected
        ) | (
            Kind::MissionBrokerEntries,
            Some(Kind::MissionBrokerEntries),
            ExportSourceRole::Selected
        ) | (_, Some(Kind::Contracts), ExportSourceRole::Dependency)
            | (
                _,
                Some(
                    Kind::ContractShipSpawns
                        | Kind::ContractNpcSpawns
                        | Kind::ContractEntitySpawns
                        | Kind::ContractLocationQueries
                ),
                ExportSourceRole::DerivedInput
            )
    )
}
