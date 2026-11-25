// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting::{
    self, BlueprintPoolDefinition, BlueprintPoolEntry, BlueprintPoolSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ResourceFloat32, ResourceNumber,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, Language, PhysicalSourceKind, SourceValueState,
    SourcedValue,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value;

use crate::canonical_header::{originals, wire::Wire};
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_detail_fixture as detail;
use crate::sdk_full_proof as full_proof;

const DANGLING: u128 = 0x6800_0000_0000_4000_8000_0000_0000_0001;
const NAN_BITS: u32 = 0x7fc0_0042;

#[test]
fn physical_pool_aliases_match_the_canonical_producer_and_keep_weight_bits() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&database(), origin)?;
        let entry = DataForgeEntry::read(&archive)?;
        let catalog = canonical(&archive)?;
        full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
            .with_blueprint_pools(
                CatalogRecordSelection::Guid(common::guid(detail::POOL).parse()?),
                None,
                |snapshot| verify_selection(&snapshot, &catalog, marker),
            )??;
    }
    Ok(())
}

#[test]
fn pool_sessions_do_not_open_requested_unreadable_english() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("pools.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"locked=fixture\n").encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session = full_proof::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let verify = |snapshot: BlueprintPoolSnapshot<'_>| {
        assert_eq!(snapshot.rows().len(), 1, "one selected physical pool");
        assert_eq!(snapshot.weights().len(), 1, "one weight projection");
        assert!(snapshot.source_inventory().iter_sources().next().is_some());
        assert_eq!(
            snapshot.rows()[0].source().record_index().value(),
            snapshot.weights()[0].pool().record_index().value(),
            "the projection keeps its physical owner"
        );
        assert!(
            snapshot
                .source_inventory()
                .iter_sources()
                .all(|source| match source.kind() {
                    PhysicalSourceKind::ArchiveEntry(entry) =>
                        entry.path().value().is_some_and(|path| {
                            path.replace('\\', "/")
                                .eq_ignore_ascii_case("Data/Game2.dcb")
                        }),
                    PhysicalSourceKind::ArchiveDirectory(_) => true,
                    _ => false,
                }),
            "only original DCB sources enter the query inventory"
        );
    };
    let index = crafting::with_pools(&archive, CatalogRecordSelection::All, None, |snapshot| {
        snapshot
            .rows()
            .first()
            .and_then(|row| row.source().record_index().value().copied())
    })?
    .ok_or("decoded original pool index")?;
    let selection = CatalogRecordSelection::RecordIndex(index);
    session.with_blueprint_pools(selection, None, verify)?;
    session
        .parsed(None)?
        .with_blueprint_pools(selection, None, verify)?;
    Ok(())
}

#[test]
fn blueprint_pool_queries_reject_foreign_entries_and_propagate_cancellation() -> TestResult {
    let (_folder, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let (_other, foreign) = common::open(&database(), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let selection = CatalogRecordSelection::All;
    let result = crafting::with_pools_from(&archive, &entry, selection, None, |_| {
        panic!("foreign entry cannot invoke consumer")
    });
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "foreign data rejected"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = crafting::with_pools(&archive, selection, Some(&cancel), |_| {
        panic!("cancelled query cannot invoke consumer")
    });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "pre-cancel propagates"
    );
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let cancel = CancelToken::new();
    let result = session.with_blueprint_pools(selection, Some(&cancel), |_| cancel.cancel());
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "outer success is withheld after cancellation"
    );
    Ok(())
}

fn verify_selection(
    snapshot: &BlueprintPoolSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let [primary, alias] = snapshot.rows() else {
        return Err("both physical owners of the same pool GUID".into());
    };
    let [primary_weight, alias_weight] = snapshot.weights() else {
        return Err("one sum per physical owner".into());
    };
    verify_scope(snapshot);
    assert_eq!(
        primary.source().guid().value(),
        alias.source().guid().value(),
        "nominal pool identity is shared"
    );
    assert_ne!(
        primary.source().record_index().value(),
        alias.source().record_index().value(),
        "physical pool identity stays separate"
    );
    for (row, weight) in [(primary, primary_weight), (alias, alias_weight)] {
        let physical = row.source().record_index().value().ok_or("physical pool")?;
        assert_eq!(
            weight.pool().record_index().value(),
            Some(physical),
            "each weight retains its physical owner"
        );
        let pool_row = wire_row(
            catalog,
            "/definitions/blueprintPools/rows",
            "/recordIndex/value",
            *physical,
        )?;
        let weight_row = wire_row(
            catalog,
            "/projections/blueprintPoolWeights/rows",
            "/pool/recordIndex/value",
            *physical,
        )?;
        let wire = Wire::new(weight_row, snapshot.source_inventory());
        let pool = wire.field("pool")?;
        pool.field("inputRef")?.raw(weight.pool().input_ref())?;
        pool.field("recordIndex")?
            .leaf(weight.pool().record_index())?;
        pool.field("guid")?.leaf(weight.pool().guid())?;
        wire.field("totalWeight")?
            .cell(weight.total_weight(), |value, wire| wire.float32(value))?;
        assert_eq!(
            weight.total_weight().origin().contains_encrypted_source(),
            marker,
            "the derived sum carries the actual DCB marker"
        );
        visit_definition(row, Wire::new(pool_row, snapshot.source_inventory()))?;
    }
    assert_eq!(
        primary_weight
            .total_weight()
            .value()
            .and_then(ResourceFloat32::finite),
        Some(4.0),
        "the ordinary owner retains its sum"
    );
    verify_alias(alias, alias_weight.total_weight().value())
}

fn verify_scope(snapshot: &BlueprintPoolSnapshot<'_>) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the pool declaration is available"
    );
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty()
            && coverage.unreadable_bodies.is_empty()
            && coverage.incomplete_fields.is_empty(),
        "the original readable scope is complete"
    );
}

fn verify_alias(
    alias: &BlueprintPoolDefinition<'_>,
    total: Option<&ResourceFloat32>,
) -> TestResult {
    let slots = alias
        .blueprint_rewards()
        .value()
        .ok_or("original pool slots")?;
    let [first, repeated, null, missing] = slots.as_slice() else {
        return Err("four ordered source occurrences".into());
    };
    let link = |cell: &SourcedValue<BlueprintPoolEntry<'_>>| {
        cell.value()
            .and_then(|row| row.blueprint_record().value())
            .and_then(|link| link.guid().value().copied())
    };
    let expected = common::guid(detail::TWO_TIERS).parse()?;
    assert_eq!(link(first), Some(expected), "first authored Reference");
    assert_eq!(
        link(repeated),
        Some(expected),
        "repeated authored Reference"
    );
    assert_eq!(
        null.value()
            .ok_or("null target slot")?
            .blueprint_record()
            .state(),
        SourceValueState::Null,
        "the authored null is retained"
    );
    assert_eq!(
        link(missing)
            .ok_or("dangling reference GUID must survive")?
            .to_string(),
        common::guid(DANGLING),
        "a dangling Reference retains its original identity"
    );
    let total = total.ok_or("an overflowed result still carries its bits")?;
    assert_eq!(
        total.raw_bits(),
        f32::INFINITY.to_bits(),
        "finite sum overflows"
    );
    assert_eq!(
        total.finite(),
        None,
        "nonfinite sum has no decimal projection"
    );
    let weight = missing
        .value()
        .ok_or("missing target's original entry")?
        .weight();
    let Some(ResourceNumber::Single(weight)) = weight.value() else {
        return Err("retained exact original Single weight".into());
    };
    assert_eq!(weight.raw_bits(), NAN_BITS, "the authored NaN stays exact");
    Ok(())
}

fn visit_definition(row: &BlueprintPoolDefinition<'_>, wire: Wire<'_>) -> TestResult {
    resource_consumer_walk::visit_header(row.source(), wire)?;
    wire.field("blueprintRewards")?
        .cell(row.blueprint_rewards(), |slots, wire| {
            wire.array(slots, |slot, wire| {
                wire.field("sourceType")?.leaf(slot.source_type())?;
                wire.field("blueprintRecord")?
                    .cell(slot.blueprint_record(), |value, wire| {
                        originals::visit_record(value, wire)
                    })?;
                wire.field("weight")?
                    .cell(slot.weight(), |value, wire| wire.number(value))?;
                originals::visit_arena(slot.additional_fields(), wire.field("additionalFields")?)
            })
        })?;
    originals::visit_arena(row.additional_fields(), wire.field("additionalFields")?)
}

fn wire_row<'catalog>(
    catalog: &'catalog Value,
    table: &str,
    index: &str,
    physical: u32,
) -> TestResult<&'catalog Value> {
    common::at(catalog, table)?
        .as_array()
        .ok_or("canonical pool table rows")?
        .iter()
        .find(|row| row.pointer(index).and_then(Value::as_u64) == Some(u64::from(physical)))
        .ok_or_else(|| "physical pool row in the producer".into())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(std::time::UNIX_EPOCH)
    };
    Ok(serde_json::from_slice(
        build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}

fn database() -> DataForgeBuilder {
    let slot = |guid, weight| {
        TestValue::Struct(
            TestInstance::new("BlueprintReward")
                .value("blueprintRecord", TestValue::Reference(guid))
                .value("weight", TestValue::Single(weight)),
        )
    };
    detail::database().record(
        TestRecord::new(
            detail::POOL,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.DerivedAlias",
            "libs/foundry/records/crafting/blueprintrewards/derived_alias.xml",
        )
        .value(
            "blueprintRewards",
            TestValue::Array(vec![
                slot(detail::TWO_TIERS, f32::MAX),
                slot(detail::TWO_TIERS, f32::MAX),
                slot(0, -1.0),
                slot(DANGLING, f32::from_bits(NAN_BITS)),
            ]),
        ),
    )
}
