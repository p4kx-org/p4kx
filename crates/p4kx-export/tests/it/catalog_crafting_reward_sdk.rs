// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting::{
    self, BlueprintRewardSelection, BlueprintRewardSnapshot, BlueprintRewardSource,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ResourceNumber,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, SourceValueState};
use serde_json::Value;

use crate::canonical_header::{originals, wire::Wire};
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[path = "../../src/blueprint_reward_query_fixture.rs"]
mod fixture;

#[test]
fn reward_source_getters_match_canonical_physical_occurrences_for_all_origins() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&fixture::database(false), origin)?;
        let catalog = canonical(&archive)?;
        let source = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::Guid(
            common::guid(fixture::OWNER).parse()?,
        ));
        let inverse = BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(
            fixture::ALIAS,
        ));
        let aliases = BlueprintRewardSelection::TargetPools(CatalogRecordSelection::Guid(
            common::guid(fixture::POOL).parse()?,
        ));
        for selection in [source, inverse, aliases] {
            crafting::with_reward_sources(&archive, selection, None, |snapshot| {
                verify(&snapshot, &catalog, marker)
            })??;
        }
    }
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(archive, &options)?.json(),
    )?)
}

fn verify(
    snapshot: &BlueprintRewardSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let rows = common::at(catalog, "/declarations/blueprintRewardSources/rows")?
        .as_array()
        .ok_or("canonical reward table")?;
    let expected: Vec<_> = rows
        .iter()
        .filter(|row| selected_wire_row(row, snapshot.selection()))
        .collect();
    assert_eq!(
        snapshot.rows().len(),
        expected.len(),
        "output selection preserves full canonical meaning"
    );
    assert!(
        !expected.is_empty(),
        "parity walks real canonical occurrences"
    );
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        visit(row, Wire::new(expected, snapshot.source_inventory()))?;
        verify_markers(row, marker);
    }
    Ok(())
}

fn visit(row: &BlueprintRewardSource<'_>, wire: Wire<'_>) -> TestResult {
    resource_consumer_walk::visit_header(row.source(), wire)?;
    wire.field("sourceOrdinal")?.leaf(row.source_ordinal())?;
    wire.field("pool")?.cell(row.pool(), |value, wire| {
        originals::visit_record(value, wire)
    })?;
    wire.field("poolStorage")?.leaf(row.pool_storage())?;
    wire.field("contractId")?.leaf(row.contract_id())?;
    wire.field("contractName")?.leaf(row.contract_name())?;
    wire.field("chance")?
        .cell(row.chance(), |value, wire| wire.number(value))
}

#[test]
fn normal_readable_reward_survey_and_all_public_wrappers_keep_same_meaning() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
        fixture::SOURCE,
    ));
    let check = |snapshot: BlueprintRewardSnapshot<'_>| verify_normal(&snapshot, &catalog);
    crafting::with_reward_sources_from(&archive, &entry, selection, None, check)??;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    session.with_blueprint_reward_sources(selection, None, check)??;
    session
        .parsed(None)?
        .with_blueprint_reward_sources(selection, None, check)??;
    Ok(())
}

#[test]
fn reward_queries_reject_foreign_entries_and_cancel_before_callback() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let (_other, foreign) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let selection = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
        fixture::SOURCE,
    ));
    let result = crafting::with_reward_sources_from(&archive, &entry, selection, None, |_| {
        panic!("foreign entry callback")
    });
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "foreign original input is rejected"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = crafting::with_reward_sources(&archive, selection, Some(&cancel), |_| {
        panic!("cancelled callback")
    });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "pre-cancelled query never invokes consumer"
    );
    Ok(())
}

fn selected_wire_row(row: &Value, selection: BlueprintRewardSelection) -> bool {
    match selection {
        BlueprintRewardSelection::SourceOwners(_) => {
            row.pointer("/guid/value").and_then(Value::as_str)
                == Some(common::guid(fixture::OWNER).as_str())
                && row
                    .pointer("/recordIndex/value")
                    .and_then(Value::as_u64)
                    .is_some_and(|index| {
                        index == u64::from(fixture::SOURCE)
                            || index == u64::from(fixture::SOURCE_ALIAS)
                    })
        }
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(index)) => {
            row.pointer("/pool/value/targetRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(index))
        }
        BlueprintRewardSelection::TargetPools(_) => {
            row.pointer("/pool/value/guid/value")
                .and_then(Value::as_str)
                == Some(common::guid(fixture::POOL).as_str())
        }
        _ => false,
    }
}

fn verify_markers(row: &BlueprintRewardSource<'_>, marker: Option<bool>) {
    assert_eq!(
        row.source_ordinal().origin().contains_encrypted_source(),
        marker,
        "occurrence has immediate original provenance"
    );
    assert_eq!(
        row.pool().origin().contains_encrypted_source(),
        marker,
        "link has immediate original provenance"
    );
    assert_eq!(
        row.contract_id().origin().contains_encrypted_source(),
        marker,
        "selected identity has immediate provenance"
    );
    assert_eq!(
        row.chance().state(),
        SourceValueState::Present,
        "authored chance remains available"
    );
    if let Some(ResourceNumber::UInt64 { value }) = row.chance().value() {
        assert_eq!(
            value.get(),
            fixture::WIDE_CHANCE,
            "wide authored chance is never narrowed or clamped"
        );
    }
}

fn verify_normal(snapshot: &BlueprintRewardSnapshot<'_>, catalog: &Value) -> TestResult {
    assert_eq!(
        snapshot.rows().len(),
        4,
        "selected owner has four actual occurrences"
    );
    assert!(
        snapshot.source_coverage().unreadable_bodies.is_empty()
            && snapshot.source_coverage().incomplete_fields.is_empty(),
        "clean original source coverage"
    );
    assert_eq!(
        snapshot.source_records_read(),
        1,
        "selected-owner scope stays narrow"
    );
    assert_eq!(
        snapshot.pool_records_read(),
        2,
        "only demanded pool aliases"
    );
    for row in snapshot.rows() {
        let source = row
            .source()
            .record_index()
            .value()
            .ok_or("physical source")?;
        let ordinal = row
            .source_ordinal()
            .value()
            .ok_or("original ordinal")?
            .get()
            .to_string();
        let expected = common::at(catalog, "/declarations/blueprintRewardSources/rows")?
            .as_array()
            .ok_or("canonical table")?
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64)
                    == Some(u64::from(*source))
                    && row.pointer("/sourceOrdinal/value").and_then(Value::as_str)
                        == Some(ordinal.as_str())
            })
            .ok_or("physical occurrence")?;
        visit(row, Wire::new(expected, snapshot.source_inventory()))?;
    }
    Ok(())
}
