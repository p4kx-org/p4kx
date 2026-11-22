// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_broker_context::{
    MissionBrokerContextSnapshot, with_mission_broker_context_from,
};
use p4kx_export::catalog_queries::mission_properties::{DeclarationBinding, SourcePayload};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, DataForgeEntry, FieldOrigin, SourceInventory, SourceLocation, SourceValueState,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(super) const DATASET: &str = "/configurations/missionBrokerEntries";
const BROKER: u32 = 0;
const TARGET: u32 = 1;
const ALTERNATIVE: u32 = 2;
const REWARD_GUID: u128 = 2;

#[derive(Clone, Copy)]
enum Body {
    Readable,
    Unreadable,
}

pub(super) fn read() -> TestResult<Route> {
    with_fixture(Body::Readable, |snapshot, catalog| {
        let amount = check_row(&snapshot, catalog)?;
        assert_eq!(
            amount.get(),
            2,
            "the actual Record wins over a later equal-GUID amount"
        );
        assert!(
            snapshot.reward_failures.is_empty(),
            "only the reachable physical reward was read successfully"
        );
        assert!(
            snapshot.contextual_failures.is_empty(),
            "the Broker context remains complete"
        );
        assert!(
            snapshot.coverage.incomplete_fields.is_empty(),
            "the owner body projects completely"
        );
        Route::queried_rows(
            DATASET,
            &snapshot.rows,
            "same-source synthetic canonical context and exact physical Reward origin; installed parity open",
        )
    })
}

#[test]
fn an_unreadable_reached_reward_keeps_its_unreadable_amount_and_no_nominal_fallback() -> TestResult
{
    with_fixture(Body::Unreadable, |snapshot, catalog| {
        check_context(&snapshot, catalog)?;
        let [row] = snapshot.rows.as_slice() else {
            return Err("one Broker context with a retained unreadable amount".into());
        };
        let [result] = row.reputation_results() else {
            return Err("one original reputation result".into());
        };
        let [entry] = result.amounts() else {
            return Err("one original amount".into());
        };
        let amount = entry.amount().ok_or("retained unreadable reward datum")?;
        assert_eq!(
            amount.state(),
            SourceValueState::Unreadable,
            "the exact target could not be read"
        );
        assert!(
            !amount.origin().source_set_complete(),
            "the failed physical amount is incomplete"
        );
        assert!(
            snapshot.contextual_failures.is_empty(),
            "an unavailable Reward body does not omit the Broker context"
        );
        let [failure] = snapshot.reward_failures.as_slice() else {
            return Err("one physical reward read failure".into());
        };
        assert_eq!(
            failure.record().index(),
            TARGET,
            "a readable equal-GUID peer is never substituted"
        );
        check_uses(catalog, &snapshot.source_inventory, amount.origin())
    })
}

fn with_fixture<T>(
    body: Body,
    inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>, &Value) -> TestResult<T>,
) -> TestResult<T> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("broker-context.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(body)).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let normal = build_catalog_from(&archive, &entry, &options);
    let built = match body {
        Body::Readable => normal?,
        Body::Unreadable => {
            assert!(
                matches!(normal, Err(CatalogError::IncompleteExport(_))),
                "required Reward body failure refuses normal publication"
            );
            let partial = CatalogOptions {
                publication_mode: CatalogPublicationMode::DiagnosticPartial,
                ..options
            };
            build_catalog_from(&archive, &entry, &partial)?
        }
    };
    let catalog: Value = serde_json::from_slice(built.json())?;
    with_mission_broker_context_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(BROKER),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| inspect(snapshot, &catalog),
    )?
}

fn check_context(snapshot: &MissionBrokerContextSnapshot<'_>, catalog: &Value) -> TestResult {
    let contexts = json!({"availability": snapshot.availability, "rows": &snapshot.rows});
    assert_eq!(
        Some(&contexts),
        catalog.pointer(DATASET),
        "scoped rows and all source links equal the producer"
    );
    assert_eq!(
        Some(&serde_json::to_value(&snapshot.owners)?),
        catalog.pointer("/definitions/missionBrokerEntries/rows"),
        "the canonical original Broker arena is shared, including authored references"
    );
    assert!(
        !snapshot.tag_ancestry_surveyed,
        "a reward alone does not require a Tag survey"
    );
    Ok(())
}

fn check_row(
    snapshot: &MissionBrokerContextSnapshot<'_>,
    catalog: &Value,
) -> TestResult<p4kx_export::catalog_queries::mission_broker_context::ResourceInt64> {
    check_context(snapshot, catalog)?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("one projected Broker".into());
    };
    let owner = snapshot
        .owner(row.broker())
        .ok_or("selected original Broker")?;
    let [result] = row.reputation_results() else {
        return Err("one original reputation result".into());
    };
    let [entry] = result.amounts() else {
        return Err("one original amount".into());
    };
    assert!(
        owner.source().value(entry.source()).is_some(),
        "the amount links its actual owner-local value"
    );
    let DeclarationBinding::Present { field } = entry.reward() else {
        return Err("one authored reward declaration".into());
    };
    let reference = owner.source().field(*field).ok_or("reward declaration")?;
    let original = owner
        .source()
        .value(reference.value())
        .ok_or("reward value")?;
    let Some(SourcePayload::RecordReference { value: reference }) =
        original.original().value().value()
    else {
        return Err("a typed original Record target".into());
    };
    assert_eq!(
        reference.target_record_index().value(),
        Some(&TARGET),
        "original reference selects the first physical Record"
    );
    let amount = entry
        .amount()
        .ok_or("derived amount from the exact Reward")?;
    assert!(
        amount.origin().source_set_complete(),
        "the amount keeps complete evidence"
    );
    assert_eq!(
        amount.origin().contains_encrypted_source(),
        Some(true),
        "the derived amount came from the encrypted DataForge"
    );
    check_uses(catalog, &snapshot.source_inventory, amount.origin())?;
    amount
        .value()
        .copied()
        .ok_or_else(|| "the actual amount".into())
}

fn check_uses(catalog: &Value, inventory: &SourceInventory, origin: &FieldOrigin) -> TestResult {
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer source uses")?;
    for reference in origin.source_uses_refs() {
        let actual = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("retained reward source use")?;
        assert!(
            uses.contains(&serde_json::to_value(actual)?),
            "each amount contributor is the same original producer observation"
        );
    }
    assert!(
        !inventory.iter_uses().any(|used| match used.location() {
            SourceLocation::DataForge { record_index, .. } => *record_index == ALTERNATIVE,
            SourceLocation::Subresource {
                namespace, indices, ..
            } if namespace == "dataforgeValue" => {
                indices.first() == Some(&u64::from(ALTERNATIVE))
            }
            _ => false,
        }),
        "the equal-GUID alternative does not contribute observations"
    );
    Ok(())
}

fn database(body: Body) -> Vec<u8> {
    let actual = match body {
        Body::Readable => reward("Selected", 2),
        Body::Unreadable => TestRecord::new(
            REWARD_GUID,
            "UnreadableReward",
            "UnreadableReward.Selected",
            reward_path("Selected"),
        ),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MissionBrokerEntry").array(
            "missionResultReputationRewards",
            TestType::Class("Result".to_owned()),
        ))
        .structure(
            TestStruct::new("Result")
                .array("reputationAmounts", TestType::Class("Amount".to_owned())),
        )
        .structure(TestStruct::new("Amount").field("reward", TestType::Reference))
        .structure(TestStruct::new("ReputationReward").field("reputationAmount", TestType::Int64))
        .structure(
            TestStruct::new("UnreadableReward")
                .field("amount", TestType::Int8)
                .declared_size(3),
        )
        .record(broker())
        .record(actual)
        .record(reward("Alternative", 9))
        .build()
}

fn broker() -> TestRecord {
    let amount = TestInstance::new("Amount").value("reward", TestValue::Reference(REWARD_GUID));
    let result = TestInstance::new("Result").value(
        "reputationAmounts",
        TestValue::Array(vec![TestValue::Struct(amount)]),
    );
    TestRecord::new(
        1,
        "MissionBrokerEntry",
        "MissionBrokerEntry.ScopedReward",
        "libs/foundry/records/missionbroker/scoped_reward.xml",
    )
    .value(
        "missionResultReputationRewards",
        TestValue::Array(vec![TestValue::Struct(result)]),
    )
}

fn reward(name: &str, amount: i64) -> TestRecord {
    TestRecord::new(
        REWARD_GUID,
        "ReputationReward",
        format!("ReputationReward.{name}"),
        reward_path(name),
    )
    .value("reputationAmount", TestValue::Int64(amount))
}

fn reward_path(name: &str) -> String {
    format!("libs/foundry/records/reputation/rewards/missionrewards_reputation/{name}.xml")
}
