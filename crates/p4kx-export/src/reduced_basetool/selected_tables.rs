// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, btree_map::Entry};

use serde::Serialize;

use super::ReducedError;
use super::plan;
use crate::catalog_rows::spool::RowSpoolError;
use crate::catalog_rows::staged::{StagedCatalogSequence, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;

const METADATA: &[&str] = &[
    "completeness",
    "exportHealth",
    "schemaVersion",
    "shipStorePledges",
    "source",
    "texts",
];

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
enum Section {
    Configurations,
    Declarations,
    Definitions,
    Placements,
    Projections,
}

impl Section {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "configurations" => Self::Configurations,
            "declarations" => Self::Declarations,
            "definitions" => Self::Definitions,
            "placements" => Self::Placements,
            "projections" => Self::Projections,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy)]
struct Position {
    section: Option<Section>,
    key: &'static str,
}

impl Position {
    fn for_leaf(path: &'static str) -> Option<Self> {
        if METADATA.contains(&path) || Section::parse(path).is_some() {
            return None;
        }
        Some(match path.split_once('.') {
            Some((group, key)) => Self {
                section: Some(Section::parse(group)?),
                key,
            },
            None => Self {
                section: None,
                key: path,
            },
        })
    }

    fn selected(path: &'static str) -> Result<Self, ReducedError> {
        if plan::SELECTION.contains(&path) {
            return Self::for_leaf(path).ok_or(ReducedError::IncompleteScope(path));
        }
        Err(ReducedError::IncompleteScope(path))
    }
}

pub(super) fn is_staged_dataset(path: &'static str) -> bool {
    Position::for_leaf(path).is_some()
}

#[derive(Default)]
pub(super) struct SelectedTables {
    tables: Tables,
}

#[derive(Default, Serialize)]
struct Tables {
    #[serde(flatten)]
    sequences: BTreeMap<&'static str, StagedCatalogSequence>,
    #[serde(flatten)]
    sections: BTreeMap<Section, SectionTables>,
}

type SectionTables = BTreeMap<&'static str, StagedCatalogTable>;

impl Tables {
    fn count(&self, position: Position) -> Option<usize> {
        match position.section {
            Some(section) => self
                .sections
                .get(&section)?
                .get(position.key)
                .map(StagedCatalogTable::len),
            None => self
                .sequences
                .get(position.key)
                .map(StagedCatalogSequence::len),
        }
    }

    fn take_failure(&self) -> Option<RowSpoolError> {
        self.sequences
            .values()
            .find_map(StagedCatalogSequence::take_failure)
            .or_else(|| {
                self.sections
                    .values()
                    .flat_map(BTreeMap::values)
                    .find_map(StagedCatalogTable::take_failure)
            })
    }
}

impl SelectedTables {
    pub(super) fn table(
        &mut self,
        path: &'static str,
        value: StagedCatalogTable,
    ) -> Result<(), ReducedError> {
        let position = Position::selected(path)?;
        let section = position
            .section
            .ok_or(ReducedError::IncompleteScope(path))?;
        if matches!(value.availability(), DatasetAvailability::Unavailable) {
            return Err(ReducedError::IncompleteScope(path));
        }
        insert(
            self.tables.sections.entry(section).or_default(),
            position.key,
            path,
            value,
        )
    }

    pub(super) fn sequence(
        &mut self,
        path: &'static str,
        value: StagedCatalogSequence,
    ) -> Result<(), ReducedError> {
        let position = Position::selected(path)?;
        if position.section.is_some() {
            return Err(ReducedError::IncompleteScope(path));
        }
        insert(&mut self.tables.sequences, position.key, path, value)
    }

    pub(super) fn finish(self) -> Result<CompleteTables, ReducedError> {
        let mut completeness = Vec::new();
        for path in plan::SELECTION {
            let Some(position) = Position::for_leaf(path) else {
                continue;
            };
            let rows = self
                .tables
                .count(position)
                .ok_or(ReducedError::IncompleteScope(path))?;
            completeness.push(Coverage {
                path,
                rows,
                selection: Selection::for_path(path),
            });
        }
        Ok(CompleteTables {
            completeness,
            tables: self.tables,
        })
    }
}

fn insert<T>(
    output: &mut BTreeMap<&'static str, T>,
    key: &'static str,
    path: &'static str,
    value: T,
) -> Result<(), ReducedError> {
    let Entry::Vacant(slot) = output.entry(key) else {
        return Err(ReducedError::IncompleteScope(path));
    };
    slot.insert(value);
    Ok(())
}

#[derive(Serialize)]
pub(super) struct CompleteTables {
    completeness: Vec<Coverage>,
    #[serde(flatten)]
    tables: Tables,
}

impl CompleteTables {
    pub(super) fn take_spool_failure(&self) -> Option<RowSpoolError> {
        self.tables.take_failure()
    }
}

#[derive(Serialize)]
struct Coverage {
    path: &'static str,
    rows: usize,
    selection: Selection,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Selection {
    All,
    ShipAndVehicleRoots,
    BoundStarmapObjects,
}

impl Selection {
    pub(super) fn for_path(path: &'static str) -> Self {
        match path {
            "loadouts" => Self::ShipAndVehicleRoots,
            "placements.starmapPlacements" => Self::BoundStarmapObjects,
            _ => Self::All,
        }
    }
}
