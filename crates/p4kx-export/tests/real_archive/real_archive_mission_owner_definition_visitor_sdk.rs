// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::slice;

use p4kx_export::catalog_queries::mission_definitions::owner_visit::{
    MissionOwnerDefinitionRow, MissionOwnerDefinitionSelection, MissionOwnerDefinitionSnapshot,
};
use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, SourceInventory};
use serde::Serialize;

use super::super::DefinitionOrder;
use super::{CENSUS, DATASET, FAMILIES, Family, MissionDisposition};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_mission_configuration_sdk::parse_dispositions;
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "installed Archive: configure the Build, sealed owner census and one dataset"]
fn installed_mission_owner_definition_visitor_cold_query() -> TestResult {
    let path = env::var(DATASET)?;
    let expected = parse_dispositions(&env::var(CENSUS)?)?;
    let (family, disposition) = FAMILIES
        .into_iter()
        .zip(expected)
        .find(|(family, _)| family.path() == path)
        .ok_or("configure one exact Template or Broker definition path")?;
    equipment_input::with_archive(|archive| cold_visit(archive, family, disposition))
}

fn cold_visit(archive: &Archive, family: Family, expected: MissionDisposition) -> TestResult {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let records = CatalogRecordSelection::All;
    let selection = match family {
        Family::Templates => MissionOwnerDefinitionSelection::ContractTemplates(records),
        Family::Brokers => MissionOwnerDefinitionSelection::MissionBrokerEntries(records),
    };
    let mut observed = OwnerObservations::default();
    let summary = session.visit_mission_owner_definitions(selection, None, |snapshot| {
        observed.record(family, snapshot)
    })?;
    let path = family.path();
    if summary.build.is_none() || summary.build.as_ref() != archive.build() {
        return Err(format!("{path}: the visitor returned a different Build").into());
    }
    component_family_parity::require_complete(
        path,
        summary.availability,
        summary.owner_count,
        &summary.coverage,
    )?;
    let disposition = MissionDisposition {
        owners: summary.owner_count,
        skipped: summary.coverage.skipped_leftovers,
    };
    if disposition != expected || observed.owner_count != summary.owner_count {
        return Err(format!("{path}: expected {expected:?}, observed {disposition:?}").into());
    }
    let availability = summary.availability;
    let MissionDisposition { owners, skipped } = disposition;
    let OwnerObservations {
        use_observations,
        largest_owner_inventory,
        largest_owner_contributing_sources,
        ..
    } = observed;
    print_line(&format!(
        "Mission owner definition visitor SDK: dataset={path} availability={availability:?} owners={owners} ownerUseObservations={use_observations} maximumOwnerInventoryUses={largest_owner_inventory} maximumOwnerContributingSources={largest_owner_contributing_sources} skippedLeftovers={skipped}",
    ));
    Ok(())
}

#[derive(Default)]
struct OwnerObservations {
    order: DefinitionOrder,
    owner_count: usize,
    use_observations: usize,
    largest_owner_inventory: usize,
    largest_owner_contributing_sources: usize,
}

impl OwnerObservations {
    fn record(
        &mut self,
        family: Family,
        snapshot: &MissionOwnerDefinitionSnapshot<'_>,
    ) -> TestResult {
        let inventory = &snapshot.source_inventory;
        match (family, &snapshot.row) {
            (Family::Templates, MissionOwnerDefinitionRow::ContractTemplate(row)) => {
                self.record_definition(row, row.header(), inventory)
            }
            (Family::Brokers, MissionOwnerDefinitionRow::MissionBrokerEntry(row)) => {
                self.record_definition(row, row.header(), inventory)
            }
            _ => Err("the selected visitor emitted a different definition family".into()),
        }
    }

    fn record_definition(
        &mut self,
        definition: &impl Serialize,
        header: &CanonicalRecordHeader<'_>,
        inventory: &SourceInventory,
    ) -> TestResult {
        self.order.observe(header)?;
        let row = wire_row(definition)?;
        let owner_source = row
            .get("inputRef")
            .and_then(serde_json::Value::as_str)
            .ok_or("the physical source of the owner Record is missing")?;
        if !inventory
            .iter_sources()
            .any(|source| source.id().as_str() == owner_source)
        {
            return Err("the visitor inventory cannot resolve its owner's inputRef".into());
        }
        let mut origins = Origins::default();
        origins.extend(inventory, slice::from_ref(&row))?;
        let (uses, sources) = origins.contributing_counts();
        self.use_observations = self
            .use_observations
            .checked_add(uses)
            .ok_or("source observation count overflow")?;
        self.largest_owner_inventory = self.largest_owner_inventory.max(inventory.use_count());
        self.largest_owner_contributing_sources =
            self.largest_owner_contributing_sources.max(sources);
        self.owner_count += 1;
        Ok(())
    }
}
