// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_definitions::{
    MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
};
use p4kx_export::catalog_queries::mission_properties::{
    MissionFlowOwnerRef, MissionPropertyOwnerFamily, MissionPropertyOwnerSnapshot,
    MissionPropertySelection, MissionPropertyVisitSummary, PropertySelection, TemplateSource,
    visit_mission_properties, visit_mission_properties_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    OriginalValueErrorFacts,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{CancelToken, DataForgeEntry, PhysicalSourceKind};
use serde_json::{Value, json};

use super::{TEMPLATE_GUID, TEMPLATE_RECORD, TestResult, fixture};

const PROPERTIES: &str = "/configurations/missionProperties";
const FLOWS: &str = "/configurations/missionFlows";
const ROOT_PROPERTY_TEMPLATE: u32 = 6;
const DISTINCT_NULL_TEMPLATE_GUID: u128 = 9;
const TEMPLATE_ALIAS: u32 = 2;

#[test]
fn mission_visit_matches_normal_tables_and_owner_arenas() -> TestResult {
    let (_folder, archive) = fixture::open_with_skipped_owners(DISTINCT_NULL_TEMPLATE_GUID)?;
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
    let mut owners = Vec::new();
    let mut properties = Vec::new();
    let mut flows = Vec::new();
    let summary = session.visit_mission_properties(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            check_owner(snapshot, &catalog)?;
            owners.push(snapshot.source.owner.physical().record_index());
            properties.extend(
                snapshot
                    .rows
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            flows.extend(
                snapshot
                    .flows
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(())
        },
    )?;
    assert_eq!(
        owners,
        [TEMPLATE_RECORD, ROOT_PROPERTY_TEMPLATE, TEMPLATE_ALIAS, 0],
        "Template-first and canonical GUID order"
    );
    check_summary(summary, &catalog, &properties, &flows);
    Ok(())
}

fn check_summary(
    summary: MissionPropertyVisitSummary,
    catalog: &Value,
    properties: &[Value],
    flows: &[Value],
) {
    assert_eq!(
        (
            summary.owner_count,
            summary.property_count,
            summary.flow_count
        ),
        (4, properties.len(), flows.len()),
        "terminal counts reflect all emitted rows and owners"
    );
    assert_eq!(
        summary.coverage.skipped_leftovers, 2,
        "both canonical leftover policies were applied"
    );
    let mut coverage = summary.coverage;
    coverage.skipped_leftovers = 0;
    assert_eq!(
        coverage,
        CatalogRecordCoverage::default(),
        "no genuine admitted owner was unreadable"
    );
    assert_eq!(
        catalog.pointer(PROPERTIES),
        Some(&json!({"availability": summary.availability, "rows": properties})),
        "exact Normal property row order and evidence"
    );
    assert_eq!(
        catalog.pointer(FLOWS),
        Some(&json!({"availability": summary.availability, "rows": flows})),
        "exact Normal flow row order and evidence"
    );
}

fn check_owner(snapshot: &MissionPropertyOwnerSnapshot<'_>, catalog: &Value) -> TestResult {
    let path = match &snapshot.source.owner {
        MissionFlowOwnerRef::ContractTemplate(_) => "/definitions/contractTemplates/rows",
        MissionFlowOwnerRef::MissionBrokerEntry(_) => "/definitions/missionBrokerEntries/rows",
        _ => return Err("unexpected mission definition family".into()),
    };
    let index = u64::from(snapshot.source.owner.physical().record_index());
    if matches!(
        &snapshot.source.owner,
        MissionFlowOwnerRef::ContractTemplate(_)
    ) && index == u64::from(ROOT_PROPERTY_TEMPLATE)
    {
        let [property] = snapshot.rows.as_slice() else {
            return Err("the Template has one original root property declaration".into());
        };
        assert!(
            matches!(property.selection(), PropertySelection::Selected),
            "the authored root property is selected"
        );
    }
    let original = catalog
        .pointer(path)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index)
            })
        })
        .ok_or("exact original definition for this owner")?;
    assert_eq!(
        original.get("source"),
        Some(&serde_json::to_value(&snapshot.source.arena)?),
        "same physical original source arena and reference IDs"
    );
    assert!(
        snapshot.fields_complete,
        "the original fields for this owner are readable"
    );
    assert!(
        snapshot.source_inventory.use_count() > 0,
        "owner-local origin references are resolvable"
    );
    assert!(
        snapshot
            .source_inventory
            .iter_sources()
            .all(|source| match source.kind() {
                PhysicalSourceKind::ArchiveEntryProof(entry) => entry.entry_index().get() == 0,
                _ => true,
            }),
        "locale projection is outside authored configuration reads"
    );
    Ok(())
}

#[test]
fn mission_queries_distinguish_missing_targets_from_malformed_values() -> TestResult {
    const BROKER_LINK_RECORD: u32 = 6;
    let broker_record = BROKER_LINK_RECORD;
    let selection = CatalogRecordSelection::RecordIndex(broker_record);
    let properties =
        MissionPropertySelection::new(MissionPropertyOwnerFamily::MissionBrokerEntries, selection);
    let definitions =
        MissionDefinitionSelection::new(MissionDefinitionFamily::MissionBrokerEntries, selection);
    for (fault, expected) in [
        (fixture::BrokerFieldFault::MissingTarget, true),
        (fixture::BrokerFieldFault::MalformedFlag, false),
    ] {
        let (_folder, archive) = fixture::open_with_broker_fault(fault)?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = CatalogQuerySession::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let expected_incomplete = if expected { &[][..] } else { &[broker_record] };
        let catalog = if expected {
            Some(serde_json::from_slice(
                build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
            )?)
        } else {
            None
        };
        let summary =
            session.visit_mission_properties(selection, None, |snapshot| -> TestResult {
                assert_eq!(
                    snapshot.fields_complete, expected,
                    "original projection state"
                );
                check_retained_error(&snapshot.source.arena, expected);
                if let Some(catalog) = &catalog {
                    check_owner(snapshot, catalog)?;
                }
                Ok(())
            })?;
        assert_eq!(summary.owner_count, 1, "the physical owner is retained");
        assert_eq!(summary.coverage.incomplete_fields, expected_incomplete);
        let parsed = session.parsed(None)?;
        parsed.with_mission_properties(properties, None, |snapshot| {
            assert_eq!(snapshot.coverage.incomplete_fields, expected_incomplete);
        })?;
        parsed.with_mission_definitions(definitions, None, |snapshot| -> TestResult {
            let MissionDefinitionRows::MissionBrokerEntries(rows) = &snapshot.rows else {
                return Err("expected the selected Broker definition".into());
            };
            let [broker] = rows.as_slice() else {
                return Err("expected one physical Broker".into());
            };
            assert_eq!(snapshot.coverage.incomplete_fields, expected_incomplete);
            check_retained_error(broker.source(), expected);
            Ok(())
        })??;
    }
    Ok(())
}

fn check_retained_error(arena: &TemplateSource<'_>, complete: bool) {
    let mut errors = arena
        .values()
        .iter()
        .filter_map(|value| value.original().original_error());
    let error = errors.next();
    let expected = if complete {
        matches!(
            error,
            Some(OriginalValueErrorFacts::DanglingReference { .. })
        )
    } else {
        matches!(error, Some(OriginalValueErrorFacts::InvalidBool { .. }))
    };
    assert!(
        expected && errors.next().is_none(),
        "exact original diagnostic state"
    );
}

#[test]
fn mission_visit_selects_duplicate_guids_and_exact_physical_records() -> TestResult {
    let (_folder, archive) = fixture::open_with_skipped_owners(TEMPLATE_GUID)?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let selection = CatalogRecordSelection::RecordIndex(TEMPLATE_ALIAS);
    let owner =
        MissionPropertySelection::new(MissionPropertyOwnerFamily::ContractTemplates, selection);
    let inventory = session.with_mission_properties(owner, None, |snapshot| {
        serde_json::to_value(snapshot.source_inventory)
    })??;
    let selected = visit_mission_properties_from(
        &archive,
        &entry,
        selection,
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.source.owner.physical().record_index(),
                TEMPLATE_ALIAS,
                "selected physical occurrence"
            );
            assert_eq!(
                serde_json::to_value(&snapshot.source_inventory)?,
                inventory,
                "owner closure retains the existing selected query's exact sources"
            );
            Ok(())
        },
    )?;
    assert_eq!(selected.owner_count, 1, "one physical occurrence");

    let mut duplicates = Vec::new();
    let parsed = session.parsed(None)?;
    let guid = "00000000-0000-0000-0000-000000000002".parse()?;
    let selected = parsed.visit_mission_properties(
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| -> TestResult {
            duplicates.push(snapshot.source.owner.physical().record_index());
            Ok(())
        },
    )?;
    assert_eq!(
        duplicates,
        [1, TEMPLATE_ALIAS],
        "duplicate GUID keeps physical order"
    );
    assert_eq!(
        selected.owner_count,
        duplicates.len(),
        "all admitted GUID occurrences"
    );
    Ok(())
}

#[test]
fn mission_visit_has_no_terminal_summary_after_callback_error_or_cancellation() -> TestResult {
    let (_folder, archive) = fixture::open_with_skipped_owners(DISTINCT_NULL_TEMPLATE_GUID)?;
    let mut owners = Vec::new();
    let stopped = visit_mission_properties(
        &archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            owners.push(snapshot.source.owner.physical().record_index());
            Err("consumer stopped".into())
        },
    );
    assert_eq!(
        owners,
        [TEMPLATE_RECORD],
        "only one owner before the consumer error"
    );
    assert_eq!(
        stopped.err().map(|error| error.to_string()).as_deref(),
        Some("consumer stopped"),
        "the consumer error is preserved"
    );

    owners.clear();
    let cancel = CancelToken::new();
    let cancelled = visit_mission_properties(
        &archive,
        CatalogRecordSelection::All,
        Some(&cancel),
        |snapshot| -> Result<(), CatalogError> {
            owners.push(snapshot.source.owner.physical().record_index());
            cancel.cancel();
            Ok(())
        },
    );
    assert_eq!(
        owners,
        [TEMPLATE_RECORD],
        "only one owner before cancellation"
    );
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "no terminal summary after cancellation"
    );
    Ok(())
}
