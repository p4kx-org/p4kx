// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ReputationRewardAmountSnapshot,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, Language, SourceUseRole};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::super::REPUTATION_REWARD_AMOUNTS;
use super::{Route, TestResult};

const REWARD_TYPE: &str = "SReputationRewardAmount";
const DUPLICATE: u128 = 1;
const OTHER: u128 = 2;
const UNRELATED: u128 = 3;
#[derive(Clone, Copy)]
enum Input {
    Plain,
    Encrypted,
}

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = fixture(reward_aliases(), Input::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let normal = CatalogOptions::new(UNIX_EPOCH);
    assert!(
        matches!(
            build_catalog_from(&archive, &entry, normal.with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication refuses an ambiguous reward GUID"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..normal
    };
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_reputation_reward_amounts(CatalogRecordSelection::All, None, |snapshot| {
            check_catalog_parity(&snapshot, &catalog)?;
            Route::queried_rows(REPUTATION_REWARD_AMOUNTS, &snapshot.rows, None)
        })?
}

fn check_catalog_parity(
    snapshot: &ReputationRewardAmountSnapshot<'_>,
    catalog: &Value,
) -> TestResult<()> {
    let expected = catalog
        .pointer(REPUTATION_REWARD_AMOUNTS)
        .ok_or("canonical reward amount table")?;
    let table = json!({
        "availability": snapshot.availability,
        "rows": serde_json::to_value(&snapshot.rows)?,
    });
    assert_eq!(
        &table, expected,
        "SDK and Catalog have identical reward rows and origins"
    );
    assert_eq!(
        amounts(snapshot)?,
        [-32, 17, -1024],
        "canonical GUID order and physical ties"
    );
    assert!(
        snapshot.coverage.unreadable_headers.is_empty()
            && snapshot.coverage.unreadable_bodies.is_empty()
            && snapshot.coverage.incomplete_fields.is_empty(),
        "the admitted reward scope is readable"
    );
    for row in &snapshot.rows {
        let origin = row.reputation_amount().origin();
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the original DCB is encrypted"
        );
        assert!(
            origin.source_set_complete(),
            "the exact amount has complete source evidence"
        );
        assert!(
            snapshot.source_inventory.iter_uses().any(|used| {
                origin.source_uses_refs().contains(used.id())
                    && used.scope() == InputScope::Payload
                    && used.role() == SourceUseRole::Payload
            }),
            "the signed amount names a physical payload use"
        );
        assert!(
            !row.authored_fields().roots().is_empty(),
            "additional original fields are available through the SDK"
        );
    }
    Ok(())
}

#[test]
fn reward_amount_queries_preserve_physical_aliases_and_scoped_availability() -> TestResult<()> {
    let (_directory, archive) = fixture(reward_aliases(), Input::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let guid = "00000000-0000-0000-0000-000000000001".parse()?;
    let second_index = parsed.with_reputation_reward_amounts(
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| -> TestResult<u32> {
            assert_eq!(
                amounts(&snapshot)?,
                [-32, 17],
                "both original records share the GUID"
            );
            let [first, second] = snapshot.rows.as_slice() else {
                return Err("two physical reward owners".into());
            };
            assert_eq!(
                first.editor_name().value(),
                Some(&"lower"),
                "the first physical alias stays first"
            );
            assert_eq!(
                second.editor_name().value(),
                Some(&"upper"),
                "the second physical alias stays second"
            );
            second
                .source()
                .record_index()
                .value()
                .copied()
                .ok_or_else(|| "physical reward index".into())
        },
    )??;
    parsed.with_reputation_reward_amounts(
        CatalogRecordSelection::RecordIndex(second_index),
        None,
        |snapshot| -> TestResult<()> {
            assert_eq!(amounts(&snapshot)?, [17], "one exact original Record");
            Ok(())
        },
    )??;
    let unrelated = forge
        .records()
        .find(|record| record.type_name().ok() == Some("Unrelated"))
        .ok_or("unrelated physical Record")?;
    check_other_record(&parsed, unrelated.index())?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "no locale payload was opened"
    );
    Ok(())
}

fn check_other_record(parsed: &ParsedCatalogQueries<'_>, physical_index: u32) -> TestResult {
    parsed.with_reputation_reward_amounts(
        CatalogRecordSelection::RecordIndex(physical_index),
        None,
        |snapshot| {
            assert!(
                snapshot.rows.is_empty(),
                "an out-of-family record does not create a row"
            );
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "the unrelated Record does not hide the declared reward family"
            );
        },
    )?;
    Ok(())
}

#[test]
fn reward_amount_queries_preserve_incompatible_source_width_and_health() -> TestResult<()> {
    let database = reward_schema(TestType::Int64)
        .record(reward(DUPLICATE, "wide", TestValue::Int64(i64::MAX)))
        .build();
    let (_directory, archive) = fixture(database, Input::Plain)?;
    full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_reputation_reward_amounts(
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            let [row] = snapshot.rows.as_slice() else {
                return Err("the retained incomplete reward row".into());
            };
            let index = row
                .source()
                .record_index()
                .value()
                .copied()
                .ok_or("owner index")?;
            assert!(
                row.reputation_amount().value().is_none(),
                "no narrowing to Int32"
            );
            assert_eq!(
                snapshot.coverage.incomplete_fields,
                [index],
                "exact incomplete owner"
            );
            assert!(
                row.authored_fields().roots().len() >= 2,
                "the incompatible signed storage and the authored extra remain observable"
            );
            Ok(())
        },
    )?
}

fn amounts(snapshot: &ReputationRewardAmountSnapshot<'_>) -> TestResult<Vec<i32>> {
    snapshot
        .rows
        .iter()
        .map(|row| {
            row.reputation_amount()
                .value()
                .copied()
                .ok_or_else(|| "original signed amount".into())
        })
        .collect()
}

fn reward_aliases() -> Vec<u8> {
    reward_schema(TestType::Int32)
        .structure(TestStruct::new("Unrelated"))
        .record(TestRecord::new(
            UNRELATED,
            "Unrelated",
            "Unrelated.Fixture",
            "fixture/unrelated.xml",
        ))
        .record(reward(OTHER, "other", TestValue::Int32(-1024)))
        .record(reward(DUPLICATE, "lower", TestValue::Int32(-32)))
        .record(reward(DUPLICATE, "upper", TestValue::Int32(17)))
        .build()
}

fn reward_schema(amount_type: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new().version(8).structure(
        TestStruct::new(REWARD_TYPE)
            .field("editorName", TestType::String)
            .field("reputationAmount", amount_type)
            .field("fixtureExtra", TestType::Bool),
    )
}

fn reward(id: u128, label: &str, amount: TestValue) -> TestRecord {
    TestRecord::new(
        id,
        REWARD_TYPE,
        format!("Reward.{label}"),
        format!("fixture/reputation/{label}.xml"),
    )
    .value("editorName", TestValue::text(label))
    .value("reputationAmount", amount)
    .value("fixtureExtra", TestValue::Bool(true))
}

fn fixture(database: Vec<u8>, input: Input) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("reputation_rewards.p4k");
    let dataforge = TestEntry::stored("Data/Game2.dcb", database);
    let (dataforge, key) = match input {
        Input::Plain => (dataforge, None),
        Input::Encrypted => (dataforge.encrypted(), Some(p4kx_testkit::test_key())),
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"fixture=synthetic\n")
                .encrypted(),
        )
        .write_to(&path)?;
    Ok((directory, Archive::open_with_key(path, key)?))
}
