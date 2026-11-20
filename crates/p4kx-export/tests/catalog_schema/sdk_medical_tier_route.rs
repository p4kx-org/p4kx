// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::medical::{
    MedicalItemTierSnapshot, with_medical_item_tier_configs, with_medical_item_tier_configs_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, InputScope, Language, SourceUseRole};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::super::MEDICAL_ITEM_TIER_CONFIGS;
use super::{Route, TestResult, facility_medbed_fixture as fixture};

const FAMILY: &str = "MedicalItemTierConfig";
const CONFIG_RECORD: u32 = 1;

#[derive(Clone, Copy)]
enum SourceStorage {
    Plain,
    Encrypted,
}

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open(&fixture::database(), SourceStorage::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_medical_item_tier_configs_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            verify_catalog(&snapshot, &catalog)?;
            Route::queried_rows(
                MEDICAL_ITEM_TIER_CONFIGS,
                &snapshot.rows,
                "synthetic source and Catalog parity available; installed producer parity open",
            )
        },
    )?
}

fn verify_catalog(snapshot: &MedicalItemTierSnapshot<'_>, catalog: &Value) -> TestResult {
    let table = json!({"availability": snapshot.availability, "rows": snapshot.rows});
    assert_eq!(
        Some(&table),
        catalog.pointer(MEDICAL_ITEM_TIER_CONFIGS),
        "the same producer row retains all properties and origin references"
    );
    assert_eq!(
        positions(snapshot)?,
        [CONFIG_RECORD],
        "the physical standalone definition was selected"
    );
    let [row] = snapshot.rows.as_slice() else {
        return Err("one original medical tier definition".into());
    };
    let consumption = row
        .resource_consumption()
        .value()
        .ok_or("authored resource consumption")?;
    assert_eq!(
        consumption
            .bed_resource()
            .value()
            .and_then(|reference| reference.resolved_index()),
        Some(2),
        "the original ResourceType reference retains the target Record index"
    );
    let tiers = row
        .med_bed_configs()
        .value()
        .ok_or("authored med-bed tiers")?;
    let [first, second, third] = tiers.as_slice() else {
        return Err("three authored med-bed tiers".into());
    };
    for (tier, label, range) in [
        (first, "FixtureTierTwo", 0),
        (second, "FixtureTierOne", 73_001),
        (third, "FixtureTierTwo", 0),
    ] {
        let tier = tier.value().ok_or("present tier declaration")?;
        assert_eq!(
            tier.tier().value(),
            Some(&label),
            "original tier label and array order"
        );
        assert_eq!(
            tier.default_respawn_range().value(),
            Some(&range),
            "original respawn range"
        );
    }
    verify_origin(snapshot, catalog, consumption.surgery_t1().origin())
}

fn verify_origin(
    snapshot: &MedicalItemTierSnapshot<'_>,
    catalog: &Value,
    origin: &p4kx_extraction::FieldOrigin,
) -> TestResult {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the capacity is sourced from encrypted DataForge"
    );
    assert!(
        origin.source_set_complete(),
        "the original capacity evidence is complete"
    );
    let producer = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer source use ledger")?;
    let mut payload_or_selector = false;
    for reference in origin.source_uses_refs() {
        let used = snapshot
            .source_inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("selected capacity source use")?;
        assert!(
            producer.contains(&serde_json::to_value(used)?),
            "physical SourceUse ID, role and original location are shared"
        );
        payload_or_selector |= used.scope() == InputScope::Payload
            && matches!(
                used.role(),
                SourceUseRole::Payload | SourceUseRole::Selector
            );
    }
    assert!(
        payload_or_selector,
        "one contributing use belongs to the encrypted payload"
    );
    Ok(())
}

#[test]
fn medical_sdk_preserves_physical_aliases_without_reading_locales() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(FAMILY))
        .structure(TestStruct::new("OtherMedicalFixture"))
        .record(TestRecord::new(
            7,
            FAMILY,
            "Fixture.Tier",
            "fixture/tier.xml",
        ))
        .record(TestRecord::new(
            7,
            FAMILY,
            "Fixture.Alias",
            "fixture/alias.xml",
        ))
        .record(TestRecord::new(
            7,
            "OtherMedicalFixture",
            "Fixture.Other",
            "fixture/other.xml",
        ));
    let (_folder, archive) = open(&database, SourceStorage::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge.record(0).ok_or("original fixture identity")?.guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::English]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selection, expected) in [
        (CatalogRecordSelection::Guid(guid), [0, 1].as_slice()),
        (CatalogRecordSelection::RecordIndex(1), [1].as_slice()),
        (CatalogRecordSelection::RecordIndex(2), [].as_slice()),
    ] {
        parsed.with_medical_item_tier_configs(selection, None, |snapshot| {
            assert_eq!(
                positions(&snapshot)?,
                expected,
                "selection preserves every admitted physical alias"
            );
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "declared family is available independently of the selection"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        })??;
    }
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "the encrypted locale was not attempted"
    );
    Ok(())
}

#[test]
fn medical_sdk_surfaces_incomplete_fields_and_cancellation() -> TestResult {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(FAMILY).field("medBedConfigs", TestType::Int32))
        .record(
            TestRecord::new(1, FAMILY, "Fixture.Invalid", "fixture/invalid.xml")
                .value("medBedConfigs", TestValue::Int32(12)),
        );
    let (_folder, archive) = open(&database, SourceStorage::Plain)?;
    with_medical_item_tier_configs(&archive, CatalogRecordSelection::All, None, |snapshot| {
        let [row] = snapshot.rows.as_slice() else {
            return Err("one readable but incomplete medical definition".into());
        };
        assert_eq!(
            snapshot.coverage.incomplete_fields,
            [0],
            "affected original Record is explicit"
        );
        assert!(
            row.med_bed_configs().value().is_none(),
            "a scalar cannot stand for a tier array"
        );
        assert!(
            !row.additional_fields().roots().is_empty(),
            "the incompatible source remains authored"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            with_medical_item_tier_configs(
                &archive,
                CatalogRecordSelection::All,
                Some(&cancel),
                |_| ()
            ),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled query yields no snapshot"
    );
    Ok(())
}

fn positions(snapshot: &MedicalItemTierSnapshot<'_>) -> TestResult<Vec<u32>> {
    snapshot
        .rows
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or_else(|| "physical medical Record".into())
        })
        .collect()
}

fn open(
    database: &DataForgeBuilder,
    storage: SourceStorage,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("medical-fixture.p4k");
    let dataforge = TestEntry::stored("Data/Game2.dcb", database.build());
    let (dataforge, key, english) = match storage {
        SourceStorage::Plain => (
            dataforge,
            None,
            TestEntry::stored(Language::English.global_ini_path(), b"shared=Fixture\n").encrypted(),
        ),
        SourceStorage::Encrypted => (
            dataforge.encrypted(),
            Some(p4kx_testkit::test_key()),
            TestEntry::stored(Language::English.global_ini_path(), b"shared=Fixture\n"),
        ),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(english)
        .write_to(&path)?;
    Ok((folder, Archive::open_with_key(path, key)?))
}
