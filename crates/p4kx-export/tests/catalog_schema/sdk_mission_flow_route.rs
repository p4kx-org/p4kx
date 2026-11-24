// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_properties::{
    DeclarationBinding, FlowSelection, MissionFlowConfiguration, MissionPropertyOwnerFamily,
    MissionPropertySelection, MissionPropertySnapshot, MissionPropertySource,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "sdk_mission_flow_fixture.rs"]
mod fixture;
use fixture::open;

#[path = "sdk_mission_property_visit.rs"]
mod visit;

#[path = "sdk_broker_definition_route.rs"]
mod broker_definitions;
pub(super) use broker_definitions::DATASET as BROKER_DEFINITIONS;

#[path = "sdk_contract_template_definition_route.rs"]
mod template_definitions;
pub(super) use template_definitions::DATASET as TEMPLATE_DEFINITIONS;

#[path = "sdk_contract_template_context_route.rs"]
mod template_contexts;
pub(super) use template_contexts::DATASET as TEMPLATE_CONTEXTS;

pub(super) const DATASET: &str = "/configurations/missionFlows";

const TEMPLATE: MissionPropertyOwnerFamily = MissionPropertyOwnerFamily::ContractTemplates;
const BROKER: MissionPropertyOwnerFamily = MissionPropertyOwnerFamily::MissionBrokerEntries;
const TEMPLATE_GUID: u128 = 2;
const NULL_TEMPLATE_GUID: u128 = 3;
const TEMPLATE_RECORD: u32 = 1;
const TEMPLATE_ALIAS: u32 = 2;

pub(super) fn read() -> TestResult<[Route; 4]> {
    let (_folder, archive) = open(NULL_TEMPLATE_GUID)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let mut rows = Vec::new();
    for family in [TEMPLATE, BROKER] {
        let selection = MissionPropertySelection::new(family, CatalogRecordSelection::All);
        rows.extend(parsed.with_mission_properties(selection, None, |snapshot| {
            check_owner_graphs(&catalog, family, snapshot)
        })??);
    }
    let table = json!({"availability": DatasetAvailability::Available, "rows": &rows});
    assert_eq!(
        Some(&table),
        catalog.pointer(DATASET),
        "the same declarations retain Template-first order, original arena links and origins",
    );
    let flows = Route::queried_rows(
        DATASET,
        &rows,
        "both owner families have normal synthetic producer parity; installed parity open",
    )?;
    let templates = template_definitions::read(&parsed, &catalog)?;
    let brokers = broker_definitions::read(&parsed, &catalog)?;
    let contexts = template_contexts::read(&parsed, &catalog)?;
    Ok([flows, templates, brokers, contexts])
}

fn check_owner_graphs(
    catalog: &Value,
    family: MissionPropertyOwnerFamily,
    snapshot: MissionPropertySnapshot<'_>,
) -> TestResult<Vec<MissionFlowConfiguration>> {
    let canonical = match family {
        MissionPropertyOwnerFamily::ContractTemplates => "/definitions/contractTemplates/rows",
        MissionPropertyOwnerFamily::MissionBrokerEntries => {
            "/definitions/missionBrokerEntries/rows"
        }
        _ => return Err("expected the fixture's authored mission families".into()),
    };
    let definitions = catalog
        .pointer(canonical)
        .and_then(Value::as_array)
        .ok_or("producer definitions")?;
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "both source families are declared"
    );
    assert!(
        snapshot.coverage.incomplete_fields.is_empty(),
        "original owner graphs are readable"
    );
    assert_eq!(
        snapshot.sources.len(),
        definitions.len(),
        "one source arena per physical owner"
    );
    for row in &snapshot.flows {
        let owner = row.owner().physical().record_index();
        let source = snapshot
            .sources
            .iter()
            .find(|source| source.owner.physical().record_index() == owner)
            .ok_or("flow joins its own physical source arena")?;
        let definition = definitions
            .iter()
            .find(|item| {
                item.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(owner))
            })
            .ok_or("producer owner of the same physical Record")?;
        assert_eq!(
            &serde_json::to_value(&source.arena)?,
            definition.get("source").ok_or("producer owner arena")?,
            "the source arena retains the producer coordinate and evidence closure",
        );
        check_links(row, source)?;
    }
    Ok(snapshot.flows)
}

fn check_links(row: &MissionFlowConfiguration, source: &MissionPropertySource<'_>) -> TestResult {
    let DeclarationBinding::Present { field, .. } = row.declaration() else {
        return Err("the fixture always declares missionFlow".into());
    };
    let declared = source
        .arena
        .field(*field)
        .ok_or("same-owner declaration field")?;
    assert_eq!(
        declared.name().value().copied(),
        Some("missionFlow"),
        "original root declaration"
    );
    let FlowSelection::Selected { value, .. } = row.selection() else {
        assert!(
            matches!(row.selection(), FlowSelection::NoStruct),
            "the null alias keeps its declaration"
        );
        assert!(
            row.triggers().is_empty() && row.referenced_fields().is_empty(),
            "a null declaration adds no links"
        );
        return Ok(());
    };
    assert!(
        source.arena.value(*value).is_some(),
        "selected original Struct link"
    );
    let [trigger] = row.triggers() else {
        return Err("expected the original trigger slot".into());
    };
    assert!(
        source.arena.value(*trigger).is_some(),
        "trigger resolves in this owner's arena"
    );
    let [target] = row.referenced_fields() else {
        return Err("expected an owner-local property selector".into());
    };
    let field = source
        .arena
        .field(*target)
        .ok_or("same-owner selected property field")?;
    assert_eq!(
        field.name().value().copied(),
        Some("missionVariableName"),
        "physical target field"
    );
    assert_eq!(
        field.name().origin().contains_encrypted_source(),
        Some(true),
        "the target links to the encrypted original DataForge",
    );
    Ok(())
}

fn check_alias(
    snapshot: &MissionPropertySnapshot<'_>,
    all: &[MissionFlowConfiguration],
) -> TestResult {
    let [row] = snapshot.flows.as_slice() else {
        return Err("the physical alias selection must return exactly one flow declaration".into());
    };
    let [first, alias] = all else {
        return Err("expected both physical Templates".into());
    };
    let physical = row.owner().physical();
    assert_eq!(
        physical.record_index(),
        TEMPLATE_ALIAS,
        "selection retains the original alias position"
    );
    assert_eq!(
        physical.record_guid(),
        first.owner().physical().record_guid(),
        "two physical Template records carry the same GUID",
    );
    assert_eq!(
        first.owner().physical().record_index(),
        TEMPLATE_RECORD,
        "first physical Template"
    );
    assert_eq!(
        serde_json::to_value(row)?,
        serde_json::to_value(alias)?,
        "the alias row keeps exact evidence"
    );
    assert_eq!(
        row.declaration_index(),
        Some(0),
        "the alias preserves its original declaration ordinal"
    );
    assert!(
        matches!(row.selection(), FlowSelection::NoStruct),
        "explicit null remains separate from absence"
    );
    Ok(())
}

#[test]
fn mission_flows_and_owner_definitions_match_the_normal_catalog() -> TestResult {
    read().map(|_| ())
}

#[test]
fn a_physical_template_selection_retains_the_explicitly_null_alias() -> TestResult {
    let (_folder, archive) = open(TEMPLATE_GUID)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let all = MissionPropertySelection::new(TEMPLATE, CatalogRecordSelection::All);
    let rows = parsed.with_mission_properties(all, None, |snapshot| snapshot.flows)?;
    let alias = MissionPropertySelection::new(
        TEMPLATE,
        CatalogRecordSelection::RecordIndex(TEMPLATE_ALIAS),
    );
    parsed.with_mission_properties(alias, None, |snapshot| check_alias(&snapshot, &rows))??;
    template_definitions::check_alias(&parsed)?;
    parsed.with_contract_template_context(
        CatalogRecordSelection::RecordIndex(TEMPLATE_ALIAS),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows.as_slice() else {
                return Err("one exact context row for the shared-GUID alias".into());
            };
            assert_eq!(
                row.template().record_index(),
                TEMPLATE_ALIAS,
                "context preserves the selected alias, despite a duplicate GUID"
            );
            assert!(
                snapshot.owner(row.template()).is_some(),
                "the query links the same physical owner"
            );
            assert!(
                !snapshot.tag_ancestry_surveyed,
                "the null alias does not visit the other Template's Tag dependency"
            );
            Ok(())
        },
    )?
}
