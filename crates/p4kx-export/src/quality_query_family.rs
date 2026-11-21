// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, Record};

use super::{CatalogRecordCoverage, DatasetAvailability};
use crate::crafting_quantization::QUANTIZATIONS;
use crate::crafting_records::{DISTRIBUTIONS, OVERRIDES};
use crate::entity_selection::Selected;
use crate::selection::{self, Kind, KindRule};

const DISTRIBUTION_RULES: [KindRule; 2] = [DISTRIBUTIONS, OVERRIDES];
const QUANTIZATION_RULES: [KindRule; 1] = [QUANTIZATIONS];

#[derive(Clone, Copy)]
pub(super) enum Family {
    Distributions,
    Quantizations,
}

impl Family {
    pub(super) fn selected(
        self,
        record: &Record<'_>,
        coverage: &mut CatalogRecordCoverage,
    ) -> bool {
        match self.classify(record) {
            Ok(Some(Selected::Entity(_))) => return true,
            Ok(Some(Selected::Leftover(_))) => coverage.skipped_leftovers += 1,
            Ok(None) => {}
            Err(_) => coverage.unreadable_headers.push(record.index()),
        }
        false
    }

    pub(super) fn availability(self, forge: &DataForge<'_>, rows: usize) -> DatasetAvailability {
        if rows > 0 {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::from_source_declarations(forge, |name| {
                self.rules().iter().any(|rule| rule.type_name == name)
            })
        }
    }

    fn classify(self, record: &Record<'_>) -> Result<Option<Selected>, DataForgeError> {
        let source_type = record.type_name()?;
        if !self
            .rules()
            .iter()
            .any(|rule| rule.type_name == source_type)
        {
            return Ok(None);
        }
        let path = record.record_path()?;
        if !self
            .rules()
            .iter()
            .any(|rule| rule.selects(path, source_type))
        {
            return Ok(None);
        }
        Ok(Some(
            if selection::is_kind_leftover(self.kind(), path, record.name()?) {
                Selected::Leftover(self.kind())
            } else {
                Selected::Entity(self.kind())
            },
        ))
    }

    const fn kind(self) -> Kind {
        match self {
            Self::Distributions => Kind::QualityDistributions,
            Self::Quantizations => Kind::QualityQuantizations,
        }
    }

    const fn rules(self) -> &'static [KindRule] {
        match self {
            Self::Distributions => &DISTRIBUTION_RULES,
            Self::Quantizations => &QUANTIZATION_RULES,
        }
    }
}
