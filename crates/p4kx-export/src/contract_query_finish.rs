// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::CancelToken;

use super::read::{Projected, Retention, Transaction};
use super::{
    ContractKey, ContractQueryCoverage, ContractQueryDiagnostic, ContractSummary, ShadowedContract,
};
use crate::catalog_projection::CatalogProjection;
use crate::contract_missions::ContractWithTitles;
use crate::mission_intros::PendingContract;
use crate::{CatalogError, Kind};

fn finalize<'source>(
    roots: &BTreeSet<u32>,
    target: Option<ContractKey>,
    transaction: &mut Transaction<'source>,
    coverage: &mut ContractQueryCoverage<'source>,
) -> Result<crate::missions::FinalizedContracts<'source>, CatalogError> {
    let mut candidates = BTreeMap::new();
    for row in &transaction.pending {
        if roots.contains(&row.base.source.record_index()) && selected_key(row, target) {
            let key = (
                row.base.guid.value().map(|value| value.0),
                row.base.sub_contract_guid.value().map(|value| value.0),
            );
            candidates
                .entry(key)
                .or_insert_with(Vec::new)
                .push(row.base.source.record_index());
        }
    }
    let finished = crate::missions::finalize_contracts_scoped(
        std::mem::take(&mut transaction.pending),
        &transaction.sets,
        transaction.grants.take(),
        &mut transaction.ledger,
        |row| {
            let owner = row.base.source.record_index();
            let key = (
                row.base.guid.value().map(|value| value.0),
                row.base.sub_contract_guid.value().map(|value| value.0),
            );
            if let (Some(guid), Some(selected)) = (key.0, candidates.get(&key)) {
                for &selected in selected.iter().filter(|&&selected| selected != owner) {
                    coverage.shadowed_candidates.push(ShadowedContract {
                        contract_guid: guid,
                        sub_contract_guid: key.1,
                        selected_record_index: selected,
                        winner_record_index: owner,
                    });
                }
            }
            roots.contains(&owner) && selected_key(row, target)
        },
    )?;
    Ok(finished)
}

pub(super) fn finish<'source>(
    projection: &CatalogProjection<'_, 'source>,
    roots: &BTreeSet<u32>,
    target: Option<ContractKey>,
    mut transaction: Transaction<'source>,
    output: &mut Projected<'source>,
    coverage: &mut ContractQueryCoverage<'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let finished = finalize(roots, target, &mut transaction, coverage)?;
    match transaction.retention {
        Retention::Details => {
            for ContractWithTitles { row, titles } in finished.rows {
                output.rows.push(row);
                output.titles.push(titles);
            }
            publish_details(projection, roots, target, &mut transaction, output, cancel)?;
        }
        Retention::Summaries => {
            output.summaries = finished
                .rows
                .into_iter()
                .map(ContractSummary::from_row)
                .collect();
            output.locations = transaction.pools.locations.into_values().collect();
        }
    }
    coverage.diagnostics.extend(
        transaction
            .ledger
            .field_diagnostics
            .into_iter()
            .map(ContractQueryDiagnostic::new),
    );
    Ok(())
}

fn publish_details<'source>(
    projection: &CatalogProjection<'_, 'source>,
    roots: &BTreeSet<u32>,
    target: Option<ContractKey>,
    transaction: &mut Transaction<'source>,
    output: &mut Projected<'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    if target.is_some() {
        let linked = output
            .rows
            .iter()
            .flat_map(|row| row.base.property_set_guids.iter())
            .map(|guid| guid.0)
            .collect();
        transaction.sets.retain_guids(&linked);
    } else {
        transaction.sets.retain_owners(roots);
    }
    output.property_sets = std::mem::take(&mut transaction.sets)
        .publish(&projection.for_kind(Kind::MissionPropertySets), cancel)?;
    let pools = std::mem::take(&mut transaction.pools).publish(projection, cancel)?;
    output.ships = pools.ships;
    output.npcs = pools.npcs;
    output.entities = pools.entities;
    output.locations = pools.locations;
    Ok(())
}

fn selected_key(row: &PendingContract<'_>, target: Option<ContractKey>) -> bool {
    target.is_none_or(|target| {
        row.base.guid.value().map(|value| value.0) == Some(target.contract_guid())
            && row.base.sub_contract_guid.value().map(|value| value.0) == target.sub_contract_guid()
    })
}
