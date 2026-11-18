// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record};

use crate::Kind;
use crate::catalog_queries::{CatalogRecordSelection, DatasetAvailability};
use crate::crafting_records;
use crate::selection::{self, KindRule};

use super::TestResult;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Globals,
    Refining,
    Distributions,
    Quantizations,
    Groups,
    Parents,
}

pub(super) const DIRECT_FAMILIES: [Family; 5] = [
    Family::Globals,
    Family::Refining,
    Family::Distributions,
    Family::Quantizations,
    Family::Groups,
];

pub(super) const QUERIES: [Family; 6] = [
    Family::Globals,
    Family::Refining,
    Family::Distributions,
    Family::Quantizations,
    Family::Groups,
    Family::Parents,
];

#[derive(Default)]
pub(super) struct Census {
    pub(super) declarations: usize,
    pub(super) unreadable_declarations: usize,
    pub(super) foreign_types: usize,
    pub(super) matching_types: usize,
    pub(super) outside_rule: usize,
    pub(super) leftovers: usize,
    pub(super) possible_unreadable: usize,
    pub(super) owners: Vec<u32>,
}

impl Family {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Globals => "miningGlobals",
            Self::Refining => "refiningProcesses",
            Self::Distributions => "qualityDistributions",
            Self::Quantizations => "qualityQuantizations",
            Self::Groups => "resourceGroups",
            Self::Parents => "resourceGroupParents",
        }
    }

    pub(super) const fn census_index(self) -> usize {
        match self {
            Self::Globals => 0,
            Self::Refining => 1,
            Self::Distributions => 2,
            Self::Quantizations => 3,
            Self::Groups | Self::Parents => 4,
        }
    }

    pub(super) const fn kind(self) -> Kind {
        match self {
            Self::Globals => Kind::MiningGlobalParams,
            Self::Refining => Kind::RefiningProcesses,
            Self::Distributions => Kind::QualityDistributions,
            Self::Quantizations => Kind::QualityQuantizations,
            Self::Groups | Self::Parents => Kind::ResourceGroups,
        }
    }

    fn rules(self) -> impl Iterator<Item = KindRule> {
        std::iter::once(self.kind().records())
            .chain((self == Self::Distributions).then_some(crafting_records::OVERRIDES))
    }

    pub(super) fn census(self, forge: &DataForge<'_>) -> TestResult<Census> {
        let mut census = Census::default();
        for structure in forge.structs() {
            match structure.name() {
                Ok(name) if self.rules().any(|rule| rule.type_name == name) => {
                    census.declarations += 1;
                }
                Ok(_) => {}
                Err(_) => census.unreadable_declarations += 1,
            }
        }
        for record in forge.records() {
            self.consider(&record, &mut census);
        }
        census.owners.sort_unstable();
        if census.owners.is_empty()
            && (census.unreadable_declarations > 0 || census.possible_unreadable > 0)
        {
            return Err("uncertain original metadata cannot establish family absence".into());
        }
        Ok(census)
    }

    fn consider(self, record: &Record<'_>, census: &mut Census) {
        let Ok(source_type) = record.type_name() else {
            census.possible_unreadable += 1;
            return;
        };
        if !self.rules().any(|rule| rule.type_name == source_type) {
            census.foreign_types += 1;
            return;
        }
        census.matching_types += 1;
        let Ok(path) = record.record_path() else {
            census.possible_unreadable += 1;
            return;
        };
        if !self.rules().any(|rule| rule.selects(path, source_type)) {
            census.outside_rule += 1;
            return;
        }
        let Ok(name) = record.name() else {
            census.possible_unreadable += 1;
            return;
        };
        if selection::is_kind_leftover(self.kind(), path, name) {
            census.leftovers += 1;
        } else {
            census.owners.push(record.index());
        }
    }
}

impl Census {
    pub(super) fn selection(&self) -> CatalogRecordSelection {
        self.owners.first().copied().map_or(
            CatalogRecordSelection::All,
            CatalogRecordSelection::RecordIndex,
        )
    }

    pub(super) const fn availability(&self) -> DatasetAvailability {
        if self.declarations > 0 || !self.owners.is_empty() {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::Absent
        }
    }

    pub(super) const fn absence(&self) -> &'static str {
        if !self.owners.is_empty() {
            "selectedOwner"
        } else if self.declarations > 0 {
            "declaredWithoutAdmissibleOwner"
        } else {
            "sourceTypeAbsent"
        }
    }
}
