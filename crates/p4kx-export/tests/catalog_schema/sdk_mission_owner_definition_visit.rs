// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_definitions::owner_visit::{
    MissionOwnerDefinitionRow, MissionOwnerDefinitionSelection, MissionOwnerDefinitionSnapshot,
    visit_mission_owner_definitions, visit_mission_owner_definitions_from,
};
use p4kx_export::catalog_queries::mission_definitions::{
    MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, OriginalValueErrorFacts, ParsedCatalogQueries,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{CancelToken, DataForgeEntry, SourceInventory};
use serde_json::{Value, json};

use super::{TestResult, fixture};

const ALL: CatalogRecordSelection = CatalogRecordSelection::All;
const DISTINCT_NULL_TEMPLATE_GUID: u128 = 9;
const BROKERS: MissionOwnerDefinitionSelection =
    MissionOwnerDefinitionSelection::MissionBrokerEntries(ALL);

#[test]
fn mission_owner_definition_visit_preserves_normal_rows_and_original_source_joins() -> TestResult {
    let (_folder, archive) = fixture::open_with_skipped_owners(DISTINCT_NULL_TEMPLATE_GUID)?;
    let entry = DataForgeEntry::read(&archive)?;
    let normal: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (family, selection, path) in [
        (
            MissionDefinitionFamily::ContractTemplates,
            MissionOwnerDefinitionSelection::ContractTemplates(ALL),
            "/definitions/contractTemplates",
        ),
        (
            MissionDefinitionFamily::MissionBrokerEntries,
            BROKERS,
            "/definitions/missionBrokerEntries",
        ),
    ] {
        let (original, coverage, inventory) = aggregate(&parsed, family)?;
        let mut rows = Vec::new();
        let summary =
            parsed.visit_mission_owner_definitions(selection, None, |snapshot| -> TestResult {
                assert!(
                    snapshot.fields_complete,
                    "the admitted original fields project completely"
                );
                let row = definition_row(snapshot)?;
                assert_owner_sources(&row, &snapshot.source_inventory, &inventory)?;
                rows.push(row);
                Ok(())
            })?;
        assert_eq!(
            original, rows,
            "rows preserve the complete typed SDK definitions"
        );
        assert_eq!(
            normal.pointer(path),
            Some(&json!({"availability": summary.availability, "rows": rows})),
            "Normal order, physical indices, source arenas and all joins"
        );
        assert_eq!(
            summary.owner_count,
            rows.len(),
            "all admitted readable owners"
        );
        assert_eq!(
            summary.coverage, coverage,
            "physical read and admission coverage"
        );
    }
    Ok(())
}

fn aggregate(
    parsed: &ParsedCatalogQueries<'_>,
    family: MissionDefinitionFamily,
) -> TestResult<(Vec<Value>, CatalogRecordCoverage, SourceInventory)> {
    parsed.with_mission_definitions(
        MissionDefinitionSelection::new(family, ALL),
        None,
        |snapshot| {
            let rows = match snapshot.rows {
                MissionDefinitionRows::ContractTemplates(rows) => rows
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<_, _>>()?,
                MissionDefinitionRows::MissionBrokerEntries(rows) => rows
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<_, _>>()?,
                _ => return Err("expected an owner definition family".into()),
            };
            Ok((rows, snapshot.coverage, snapshot.source_inventory))
        },
    )?
}

fn definition_row(snapshot: &MissionOwnerDefinitionSnapshot<'_>) -> TestResult<Value> {
    match &snapshot.row {
        MissionOwnerDefinitionRow::ContractTemplate(row) => Ok(serde_json::to_value(row)?),
        MissionOwnerDefinitionRow::MissionBrokerEntry(row) => Ok(serde_json::to_value(row)?),
        _ => Err("expected a typed original owner definition".into()),
    }
}

fn assert_owner_sources(
    row: &Value,
    owner: &SourceInventory,
    aggregate: &SourceInventory,
) -> TestResult {
    let mut needed = BTreeSet::new();
    use_refs(row, &mut needed)?;
    assert!(!needed.is_empty(), "a definition has observed source uses");
    for id in needed {
        let local = owner
            .iter_uses()
            .find(|used| used.id().as_str() == id)
            .ok_or("missing owner use")?;
        let expected = aggregate
            .iter_uses()
            .find(|used| used.id().as_str() == id)
            .ok_or("missing aggregate use")?;
        assert_eq!(
            serde_json::to_value(local)?,
            serde_json::to_value(expected)?,
            "exact observed use {id}"
        );
        assert!(
            owner
                .iter_sources()
                .any(|source| source.id() == local.source()),
            "the owner inventory includes every referenced physical source"
        );
    }
    for local in owner.iter_sources() {
        let expected = aggregate
            .iter_sources()
            .find(|source| source.id() == local.id())
            .ok_or("missing aggregate source")?;
        assert_eq!(
            serde_json::to_value(local)?,
            serde_json::to_value(expected)?,
            "exact original source and ancestry"
        );
        for ancestor in local.ancestors() {
            assert!(
                owner.iter_sources().any(|source| source.id() == ancestor),
                "the owner inventory includes physical ancestors"
            );
        }
    }
    Ok(())
}

fn use_refs<'wire>(row: &'wire Value, needed: &mut BTreeSet<&'wire str>) -> TestResult {
    if let Some(uses) = row.get("sourceUsesRefs").and_then(Value::as_array) {
        for used in uses {
            needed.insert(used.as_str().ok_or("source-use ID must be text")?);
        }
    }
    match row {
        Value::Object(fields) => {
            for value in fields.values() {
                use_refs(value, needed)?;
            }
        }
        Value::Array(items) => {
            for value in items {
                use_refs(value, needed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[test]
fn mission_owner_definition_visit_preserves_partial_owner_and_propagates_consumer_error()
-> TestResult {
    const FAULT_RECORD: u32 = 6;
    let (_folder, archive) =
        fixture::open_with_broker_fault(fixture::BrokerFieldFault::MalformedFlag)?;
    let entry = DataForgeEntry::read(&archive)?;
    let exact = MissionOwnerDefinitionSelection::MissionBrokerEntries(
        CatalogRecordSelection::RecordIndex(FAULT_RECORD),
    );
    let summary = visit_mission_owner_definitions_from(
        &archive,
        &entry,
        exact,
        None,
        |snapshot| -> TestResult {
            assert!(
                !snapshot.fields_complete,
                "the malformed original field remains incomplete"
            );
            let MissionOwnerDefinitionRow::MissionBrokerEntry(broker) = &snapshot.row else {
                return Err("expected one original Broker definition".into());
            };
            assert_eq!(
                broker.header().record_index().value(),
                Some(&FAULT_RECORD),
                "exact owner"
            );
            assert!(
                broker.source().values().iter().any(|value| matches!(
                    value.original().original_error(),
                    Some(OriginalValueErrorFacts::InvalidBool { byte, .. })
                        if byte.value() == Some(&3)
                )),
                "the owner arena retains the malformed Boolean byte"
            );
            Ok(())
        },
    )?;
    assert_eq!(summary.owner_count, 1, "a partial owner remains observable");
    assert_eq!(
        summary.coverage.incomplete_fields,
        [FAULT_RECORD],
        "the exact fault reaches terminal coverage"
    );

    let stopped = visit_mission_owner_definitions(&archive, exact, None, |_| -> TestResult {
        Err("consumer stopped".into())
    });
    assert_eq!(
        stopped.err().map(|error| error.to_string()).as_deref(),
        Some("consumer stopped"),
        "no successful terminal report after the consumer error"
    );
    Ok(())
}

#[test]
fn mission_owner_definition_visit_withholds_terminal_coverage_on_cancellation() -> TestResult {
    let (_folder, archive) = fixture::open_with_skipped_owners(DISTINCT_NULL_TEMPLATE_GUID)?;
    let selection = MissionOwnerDefinitionSelection::ContractTemplates(ALL);
    let cancel = CancelToken::new();
    let mut visited = 0;
    let result = visit_mission_owner_definitions(
        &archive,
        selection,
        Some(&cancel),
        |_| -> Result<(), CatalogError> {
            visited += 1;
            cancel.cancel();
            Ok(())
        },
    );
    assert_eq!(
        visited, 1,
        "cancellation stops before the next physical owner"
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "the cancelled visit has no terminal coverage"
    );

    let missing = MissionOwnerDefinitionSelection::ContractTemplates(
        CatalogRecordSelection::RecordIndex(u32::MAX),
    );
    let summary = visit_mission_owner_definitions(&archive, missing, None, |_| -> TestResult {
        Err("missing owner must produce no callback".into())
    })?;
    assert_eq!(summary.owner_count, 0, "no physical owner matches");
    assert!(
        matches!(summary.availability, DatasetAvailability::Available),
        "declaration availability is independent of row selection"
    );
    Ok(())
}
