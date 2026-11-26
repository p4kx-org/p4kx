// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::damage_resistance_macros;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::worn_gear::SuitArmor;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const PATH: &str = "configurations.suitArmor";

pub(super) struct SuitArmorTables {
    pub(super) suits: StagedCatalogTable,
    pub(super) macros: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<SuitArmorTables, ReducedError> {
    let (macros, targets) = damage_resistance_macros::collect(parsed, scratch, cancel)?;
    let suits = collect_suits(parsed, scratch, cancel, &targets)?;
    Ok(SuitArmorTables { suits, macros })
}

pub(super) fn collect_suits(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    targets: &RecordTargets,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_suit_armor(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        for original in snapshot.rows() {
            rows.append(&Suit::read(original, targets)?)?;
        }
        let table = rows.finish(snapshot.availability())?;
        completeness::rows(PATH, snapshot.rows().len(), table.len())?;
        Ok(table)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Suit<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    damage_resistance: Cell<RecordLink<'read>>,
}

impl<'read> Suit<'read> {
    fn read(original: &'read SuitArmor<'_>, targets: &RecordTargets) -> Result<Self, ReducedError> {
        ComponentKey::read(original.owner(), PATH)?;
        Ok(Self {
            owner: original.owner().into(),
            damage_resistance: completeness::cell(PATH, original.damage_resistance())?
                .try_map(|target| RecordLink::read_source(PATH, target, targets, |_| true))?,
        })
    }
}
